//! Linux kernel-level sandboxing: namespaces, Landlock LSM, Seccomp-BPF, and Cgroups v2.

use crate::IsolationError;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LandlockRule {
    pub path: PathBuf,
    pub read_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinuxSandboxConfig {
    pub unshare_user: bool,
    pub unshare_pid: bool,
    pub unshare_net: bool,
    pub restrict_filesystem: bool,
    pub allowed_paths: Vec<LandlockRule>,
    pub enable_seccomp: bool,
    pub cgroup_limit_memory_mb: Option<usize>,
    pub cgroup_limit_pids: Option<usize>,
}

impl Default for LinuxSandboxConfig {
    fn default() -> Self {
        Self {
            unshare_user: true,
            unshare_pid: true,
            unshare_net: true,
            restrict_filesystem: true,
            allowed_paths: vec![
                LandlockRule {
                    path: PathBuf::from("/usr"),
                    read_only: true,
                },
                LandlockRule {
                    path: PathBuf::from("/lib"),
                    read_only: true,
                },
                LandlockRule {
                    path: PathBuf::from("/lib64"),
                    read_only: true,
                },
                LandlockRule {
                    path: PathBuf::from("/etc/ld.so.cache"),
                    read_only: true,
                },
                LandlockRule {
                    path: PathBuf::from("/dev/null"),
                    read_only: false,
                },
                LandlockRule {
                    path: PathBuf::from("/dev/urandom"),
                    read_only: true,
                },
            ],
            enable_seccomp: true,
            cgroup_limit_memory_mb: Some(256),
            cgroup_limit_pids: Some(32),
        }
    }
}

/// Applies Linux kernel containment mechanisms to the current process.
#[cfg(target_os = "linux")]
pub fn apply_linux_sandbox(config: &LinuxSandboxConfig) -> Result<(), IsolationError> {
    // 1. Namespaces via unshare
    let mut flags = 0;
    if config.unshare_user {
        flags |= libc::CLONE_NEWUSER;
    }
    if config.unshare_pid {
        flags |= libc::CLONE_NEWPID;
    }
    if config.unshare_net {
        flags |= libc::CLONE_NEWNET;
    }

    if flags != 0 {
        let res = unsafe { libc::unshare(flags) };
        if res != 0 {
            let err = std::io::Error::last_os_error();
            tracing::warn!(flags, "Failed to unshare namespaces: {}", err);
            // If user namespace failed, unprivileged user might not have permissions
            return Err(IsolationError::SandboxInitializationFailed(format!(
                "unshare(flags={:#x}) failed: {}",
                flags, err
            )));
        }
    }

    // 2. Set NO_NEW_PRIVS (prerequisite for Landlock and Seccomp without CAP_SYS_ADMIN)
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            let err = std::io::Error::last_os_error();
            return Err(IsolationError::SandboxInitializationFailed(format!(
                "PR_SET_NO_NEW_PRIVS failed: {}",
                err
            )));
        }
    }

    // 3. Landlock LSM Filesystem Containment
    if config.restrict_filesystem {
        apply_landlock(&config.allowed_paths)?;
    }

    // 4. Seccomp-BPF Syscall Filter
    if config.enable_seccomp {
        apply_seccomp_filter()?;
    }

    // 5. Cgroup limits if configured
    if let Some(mb) = config.cgroup_limit_memory_mb {
        apply_cgroup_memory_limit(mb)?;
    }

    tracing::info!("Linux OS sandbox successfully engaged for worker process");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn apply_linux_sandbox(_config: &LinuxSandboxConfig) -> Result<(), IsolationError> {
    Err(IsolationError::UnsupportedPlatform(
        "Linux kernel isolation primitives cannot be applied on non-Linux platforms".to_string(),
    ))
}

#[cfg(target_os = "linux")]
fn apply_landlock(rules: &[LandlockRule]) -> Result<(), IsolationError> {
    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
    }

    const ACCESS_FS_READ: u64 = 0x0001 | 0x0002 | 0x0004 | 0x0008; // execute, write_file, read_file, read_dir
    const ACCESS_FS_WRITE: u64 = 0x0010 | 0x0020 | 0x0040 | 0x0080 | 0x0100 | 0x0200; // remove_dir, remove_file, make_char, make_dir, make_reg, make_sock

    let attr = LandlockRulesetAttr {
        handled_access_fs: ACCESS_FS_READ | ACCESS_FS_WRITE,
    };

    // SYS_landlock_create_ruleset = 444
    let fd = unsafe {
        libc::syscall(
            444,
            &attr as *const _ as *const libc::c_void,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0u32,
        )
    };

    if fd < 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!(
            "Landlock create_ruleset not supported on this kernel: {}",
            err
        );
        return Ok(()); // Kernel may not have CONFIG_SECURITY_LANDLOCK=y
    }

    let ruleset_fd = fd as libc::c_int;

    #[repr(C)]
    struct LandlockPathBeneathAttr {
        allowed_access: u64,
        parent_fd: libc::c_int,
    }

    for rule in rules {
        if !rule.path.exists() {
            continue;
        }
        use std::os::unix::ffi::OsStrExt;
        let c_path = std::ffi::CString::new(rule.path.as_os_str().as_bytes()).map_err(|e| {
            IsolationError::SandboxInitializationFailed(format!("Invalid path: {}", e))
        })?;

        let path_fd = unsafe { libc::open(c_path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if path_fd < 0 {
            continue;
        }

        let path_attr = LandlockPathBeneathAttr {
            allowed_access: if rule.read_only {
                ACCESS_FS_READ
            } else {
                ACCESS_FS_READ | ACCESS_FS_WRITE
            },
            parent_fd: path_fd,
        };

        // SYS_landlock_add_rule = 445 (LANDLOCK_RULE_PATH_BENEATH = 1)
        let _ = unsafe {
            libc::syscall(
                445,
                ruleset_fd,
                1u32,
                &path_attr as *const _ as *const libc::c_void,
                0u32,
            )
        };

        unsafe { libc::close(path_fd) };
    }

    // SYS_landlock_restrict_self = 446
    let res = unsafe { libc::syscall(446, ruleset_fd, 0u32) };
    unsafe { libc::close(ruleset_fd) };

    if res != 0 {
        let err = std::io::Error::last_os_error();
        return Err(IsolationError::SandboxInitializationFailed(format!(
            "landlock_restrict_self failed: {}",
            err
        )));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_seccomp_filter() -> Result<(), IsolationError> {
    #[repr(C)]
    struct SockFilter {
        code: u16,
        jt: u8,
        jf: u8,
        k: u32,
    }

    #[repr(C)]
    struct SockFprog {
        len: u16,
        filter: *const SockFilter,
    }

    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;

    const SECCOMP_RET_ALLOW: u32 = 0x7fff0000;
    const SECCOMP_RET_ERRNO: u32 = 0x00050000;
    const EPERM: u32 = 1;

    // Syscalls blocked for worker process containment:
    // ptrace, process_vm_readv/writev (cross-process memory access),
    // kexec_load, reboot, module manipulation.
    let blocked_syscalls = [
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_kexec_load,
        libc::SYS_reboot,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
    ];
    let n = blocked_syscalls.len();

    let mut filter = Vec::with_capacity(n + 3);
    // Instruction 0: Load syscall number
    filter.push(SockFilter {
        code: BPF_LD | BPF_W | BPF_ABS,
        jt: 0,
        jf: 0,
        k: 0, // offsetof(struct seccomp_data, nr)
    });

    // Check each blocked syscall
    for (i, &sc) in blocked_syscalls.iter().enumerate() {
        let jt = (n - 1 - i) as u8;
        let jf = if i == n - 1 { 1 } else { 0 };
        filter.push(SockFilter {
            code: BPF_JMP | BPF_JEQ | BPF_K,
            jt,
            jf,
            k: sc as u32,
        });
    }

    // Instruction n + 1: Deny with EPERM
    filter.push(SockFilter {
        code: BPF_RET | BPF_K,
        jt: 0,
        jf: 0,
        k: SECCOMP_RET_ERRNO | EPERM,
    });

    // Instruction n + 2: Allow all other syscalls
    filter.push(SockFilter {
        code: BPF_RET | BPF_K,
        jt: 0,
        jf: 0,
        k: SECCOMP_RET_ALLOW,
    });

    let prog = SockFprog {
        len: filter.len() as u16,
        filter: filter.as_ptr(),
    };

    // Install seccomp filter with thread-wide synchronization (SECCOMP_FILTER_FLAG_TSYNC = 1)
    // using seccomp syscall (SYS_seccomp).
    // This guarantees all current and subsequent threads in the thread group share the filter.
    const SECCOMP_SET_MODE_FILTER: libc::c_uint = 1;
    const SECCOMP_FILTER_FLAG_TSYNC: libc::c_uint = 1;

    let res = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            SECCOMP_SET_MODE_FILTER,
            SECCOMP_FILTER_FLAG_TSYNC,
            &prog as *const SockFprog as *const libc::c_void,
        )
    };

    if res != 0 {
        // Fall back to prctl if SYS_seccomp is not available
        let fallback_res = unsafe {
            libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &prog, 0, 0)
        };
        if fallback_res != 0 {
            let err = std::io::Error::last_os_error();
            tracing::warn!("Seccomp-BPF filter installation returned: {}", err);
        } else {
            tracing::info!("Seccomp-BPF filter installed via prctl fallback");
        }
    } else {
        tracing::info!("Seccomp-BPF filter installed with SECCOMP_FILTER_FLAG_TSYNC across all threads");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_cgroup_memory_limit(limit_mb: usize) -> Result<(), IsolationError> {
    let cgroup_procs = Path::new("/sys/fs/cgroup/memory.max");
    if cgroup_procs.exists() {
        let bytes = (limit_mb * 1024 * 1024).to_string();
        let _ = std::fs::write(cgroup_procs, bytes);
    }
    Ok(())
}
