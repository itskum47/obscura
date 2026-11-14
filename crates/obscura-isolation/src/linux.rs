//! Linux kernel-level sandboxing: namespaces, Landlock LSM, Seccomp-BPF, and Cgroups v2.

use crate::IsolationError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[allow(non_upper_case_globals)]
pub mod sys_constants {
    #[cfg(target_os = "linux")]
    pub use libc::{
        CLONE_NEWNET, CLONE_NEWPID, CLONE_NEWUSER, O_PATH, SYS_delete_module, SYS_finit_module,
        SYS_init_module, SYS_kexec_load, SYS_process_vm_readv, SYS_process_vm_writev, SYS_ptrace,
        SYS_reboot,
    };

    #[cfg(not(target_os = "linux"))]
    pub const CLONE_NEWUSER: libc::c_int = 0x10000000;
    #[cfg(not(target_os = "linux"))]
    pub const CLONE_NEWPID: libc::c_int = 0x20000000;
    #[cfg(not(target_os = "linux"))]
    pub const CLONE_NEWNET: libc::c_int = 0x40000000;
    #[cfg(not(target_os = "linux"))]
    pub const O_PATH: libc::c_int = 0o10000000;

    #[cfg(not(target_os = "linux"))]
    pub const SYS_ptrace: libc::c_long = 117;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_process_vm_readv: libc::c_long = 270;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_process_vm_writev: libc::c_long = 271;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_kexec_load: libc::c_long = 104;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_reboot: libc::c_long = 142;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_init_module: libc::c_long = 105;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_finit_module: libc::c_long = 273;
    #[cfg(not(target_os = "linux"))]
    pub const SYS_delete_module: libc::c_long = 106;
}

use sys_constants::*;

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
    /// When true, seccomp MUST synchronize across all threads via SECCOMP_FILTER_FLAG_TSYNC.
    /// If thread synchronization fails, single-thread fallback is strictly rejected.
    pub require_tsync: bool,
    pub cgroup_limit_memory_mb: Option<usize>,
    pub cgroup_limit_pids: Option<usize>,
    /// When true, failure to enforce resource limits (rlimits or cgroups) is fatal.
    pub strict_resource_limits: bool,
}

impl LinuxSandboxConfig {
    pub fn default_allowed_paths() -> Vec<LandlockRule> {
        vec![
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
        ]
    }
}

impl Default for LinuxSandboxConfig {
    fn default() -> Self {
        Self {
            unshare_user: true,
            unshare_pid: true,
            unshare_net: true,
            restrict_filesystem: true,
            allowed_paths: Self::default_allowed_paths(),
            enable_seccomp: true,
            require_tsync: true,
            cgroup_limit_memory_mb: Some(256),
            cgroup_limit_pids: Some(32),
            strict_resource_limits: false,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SockFilter {
    pub code: u16,
    pub jt: u8,
    pub jf: u8,
    pub k: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SockFprog {
    pub len: u16,
    pub filter: *const SockFilter,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LandlockRulesetAttr {
    pub handled_access_fs: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LandlockPathBeneathAttr {
    pub allowed_access: u64,
    pub parent_fd: libc::c_int,
}

pub const ACCESS_FS_READ: u64 = 0x0001 | 0x0002 | 0x0004 | 0x0008; // execute, write_file, read_file, read_dir
pub const ACCESS_FS_WRITE: u64 =
    0x0010 | 0x0020 | 0x0040 | 0x0080 | 0x0100 | 0x0200; // remove_dir, remove_file, make_char, make_dir, make_reg, make_sock

/// Injectable abstraction seam for Linux kernel syscalls.
pub trait LinuxSyscallDriver: Send + Sync {
    fn unshare(&self, flags: libc::c_int) -> Result<(), std::io::Error>;
    fn prctl_set_no_new_privs(&self) -> Result<(), std::io::Error>;
    fn seccomp_tsync(&self, prog: &SockFprog) -> Result<(), std::io::Error>;
    fn prctl_set_seccomp(&self, prog: &SockFprog) -> Result<(), std::io::Error>;
    fn landlock_create_ruleset(
        &self,
        attr: &LandlockRulesetAttr,
    ) -> Result<libc::c_int, std::io::Error>;
    fn landlock_add_rule(
        &self,
        ruleset_fd: libc::c_int,
        rule_type: u32,
        attr: &LandlockPathBeneathAttr,
    ) -> Result<(), std::io::Error>;
    fn landlock_restrict_self(&self, ruleset_fd: libc::c_int) -> Result<(), std::io::Error>;
    fn open_path(&self, path: &Path, flags: libc::c_int) -> Result<libc::c_int, std::io::Error>;
    fn close_fd(&self, fd: libc::c_int) -> Result<(), std::io::Error>;
    fn write_cgroup(&self, path: &Path, content: &str) -> Result<(), std::io::Error>;
    fn setrlimit(&self, resource: libc::c_int, rlim: &libc::rlimit) -> Result<(), std::io::Error>;
}

/// Standard production implementation that invokes real host kernel syscalls.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultLinuxSyscallDriver;

impl LinuxSyscallDriver for DefaultLinuxSyscallDriver {
    #[cfg(target_os = "linux")]
    fn unshare(&self, flags: libc::c_int) -> Result<(), std::io::Error> {
        let res = unsafe { libc::unshare(flags) };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn unshare(&self, _flags: libc::c_int) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unshare is only supported on Linux",
        ))
    }

    #[cfg(target_os = "linux")]
    fn prctl_set_no_new_privs(&self) -> Result<(), std::io::Error> {
        let res = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn prctl_set_no_new_privs(&self) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "prctl is only supported on Linux",
        ))
    }

    #[cfg(target_os = "linux")]
    fn seccomp_tsync(&self, prog: &SockFprog) -> Result<(), std::io::Error> {
        const SECCOMP_SET_MODE_FILTER: libc::c_uint = 1;
        const SECCOMP_FILTER_FLAG_TSYNC: libc::c_uint = 1;
        let res = unsafe {
            libc::syscall(
                libc::SYS_seccomp,
                SECCOMP_SET_MODE_FILTER,
                SECCOMP_FILTER_FLAG_TSYNC,
                prog as *const SockFprog as *const libc::c_void,
            )
        };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn seccomp_tsync(&self, _prog: &SockFprog) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "SYS_seccomp is only supported on Linux",
        ))
    }

    #[cfg(target_os = "linux")]
    fn prctl_set_seccomp(&self, prog: &SockFprog) -> Result<(), std::io::Error> {
        let res = unsafe {
            libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER,
                prog as *const SockFprog,
                0,
                0,
            )
        };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn prctl_set_seccomp(&self, _prog: &SockFprog) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "prctl(PR_SET_SECCOMP) is only supported on Linux",
        ))
    }

    #[cfg(target_os = "linux")]
    fn landlock_create_ruleset(
        &self,
        attr: &LandlockRulesetAttr,
    ) -> Result<libc::c_int, std::io::Error> {
        let fd = unsafe {
            libc::syscall(
                444, // SYS_landlock_create_ruleset
                attr as *const _ as *const libc::c_void,
                std::mem::size_of::<LandlockRulesetAttr>(),
                0u32,
            )
        };
        if fd >= 0 {
            Ok(fd as libc::c_int)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn landlock_create_ruleset(
        &self,
        _attr: &LandlockRulesetAttr,
    ) -> Result<libc::c_int, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Landlock is only supported on Linux",
        ))
    }

    #[cfg(target_os = "linux")]
    fn landlock_add_rule(
        &self,
        ruleset_fd: libc::c_int,
        rule_type: u32,
        attr: &LandlockPathBeneathAttr,
    ) -> Result<(), std::io::Error> {
        let res = unsafe {
            libc::syscall(
                445, // SYS_landlock_add_rule
                ruleset_fd,
                rule_type,
                attr as *const _ as *const libc::c_void,
                0u32,
            )
        };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn landlock_add_rule(
        &self,
        _ruleset_fd: libc::c_int,
        _rule_type: u32,
        _attr: &LandlockPathBeneathAttr,
    ) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Landlock is only supported on Linux",
        ))
    }

    #[cfg(target_os = "linux")]
    fn landlock_restrict_self(&self, ruleset_fd: libc::c_int) -> Result<(), std::io::Error> {
        let res = unsafe { libc::syscall(446, ruleset_fd, 0u32) };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn landlock_restrict_self(&self, _ruleset_fd: libc::c_int) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Landlock is only supported on Linux",
        ))
    }

    #[cfg(unix)]
    fn open_path(&self, path: &Path, flags: libc::c_int) -> Result<libc::c_int, std::io::Error> {
        use std::os::unix::ffi::OsStrExt;
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let fd = unsafe { libc::open(c_path.as_ptr(), flags) };
        if fd >= 0 {
            Ok(fd)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(unix))]
    fn open_path(&self, _path: &Path, _flags: libc::c_int) -> Result<libc::c_int, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Unix only",
        ))
    }

    #[cfg(unix)]
    fn close_fd(&self, fd: libc::c_int) -> Result<(), std::io::Error> {
        let res = unsafe { libc::close(fd) };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(unix))]
    fn close_fd(&self, _fd: libc::c_int) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Unix only",
        ))
    }

    fn write_cgroup(&self, path: &Path, content: &str) -> Result<(), std::io::Error> {
        std::fs::write(path, content)
    }

    #[cfg(unix)]
    fn setrlimit(&self, resource: libc::c_int, rlim: &libc::rlimit) -> Result<(), std::io::Error> {
        let res = unsafe { libc::setrlimit(resource, rlim) };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(unix))]
    fn setrlimit(
        &self,
        _resource: libc::c_int,
        _rlim: &libc::rlimit,
    ) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Unix only",
        ))
    }
}

/// Applies Linux kernel containment mechanisms using the provided syscall driver.
/// Enforces strict fail-closed semantics for all mandatory isolation features.
pub fn apply_linux_sandbox_with_driver(
    config: &LinuxSandboxConfig,
    driver: &dyn LinuxSyscallDriver,
) -> Result<(), IsolationError> {
    // 1. Namespaces via unshare
    let mut flags = 0;
    if config.unshare_user {
        flags |= CLONE_NEWUSER;
    }
    if config.unshare_pid {
        flags |= CLONE_NEWPID;
    }
    if config.unshare_net {
        flags |= CLONE_NEWNET;
    }

    if flags != 0 {
        if let Err(err) = driver.unshare(flags) {
            let os_err = err.raw_os_error();
            tracing::error!(
                flags,
                "Failed to unshare namespaces: {} (os error {:?})",
                err,
                os_err
            );
            return Err(IsolationError::SandboxInitializationFailed(format!(
                "Namespace isolation failed: unshare(flags={:#x}) returned {} (os error {:?})",
                flags, err, os_err
            )));
        }
    }

    // 2. Set NO_NEW_PRIVS (prerequisite for Landlock and Seccomp without CAP_SYS_ADMIN)
    if let Err(err) = driver.prctl_set_no_new_privs() {
        let os_err = err.raw_os_error();
        tracing::error!(
            "PR_SET_NO_NEW_PRIVS failed: {} (os error {:?})",
            err,
            os_err
        );
        return Err(IsolationError::SandboxInitializationFailed(format!(
            "PR_SET_NO_NEW_PRIVS failed: {} (os error {:?})",
            err, os_err
        )));
    }

    // 3. Landlock LSM Filesystem Containment
    if config.restrict_filesystem {
        apply_landlock_with_driver(&config.allowed_paths, driver)?;
    }

    // 4. Seccomp-BPF Syscall Filter
    if config.enable_seccomp {
        apply_seccomp_filter_with_driver(config.require_tsync, driver)?;
    }

    // 5. Resource limits (POSIX rlimit and Cgroups v2)
    apply_resource_limits_with_driver(config, driver)?;

    tracing::info!("Linux OS sandbox successfully engaged for worker process");
    Ok(())
}

fn apply_landlock_with_driver(
    rules: &[LandlockRule],
    driver: &dyn LinuxSyscallDriver,
) -> Result<(), IsolationError> {
    let attr = LandlockRulesetAttr {
        handled_access_fs: ACCESS_FS_READ | ACCESS_FS_WRITE,
    };

    let ruleset_fd = match driver.landlock_create_ruleset(&attr) {
        Ok(fd) => fd,
        Err(err) => {
            let os_err = err.raw_os_error();
            tracing::error!(
                "Landlock create_ruleset failed: {} (os error {:?})",
                err,
                os_err
            );
            return Err(IsolationError::SandboxInitializationFailed(format!(
                "Landlock filesystem restriction failed: landlock_create_ruleset (syscall 444) failed: {} (os error {:?})",
                err, os_err
            )));
        }
    };

    for rule in rules {
        if !rule.path.exists() {
            continue;
        }

        let path_fd = match driver.open_path(&rule.path, O_PATH | libc::O_CLOEXEC) {
            Ok(fd) => fd,
            Err(_) => continue,
        };

        let path_attr = LandlockPathBeneathAttr {
            allowed_access: if rule.read_only {
                ACCESS_FS_READ
            } else {
                ACCESS_FS_READ | ACCESS_FS_WRITE
            },
            parent_fd: path_fd,
        };

        // SYS_landlock_add_rule = 445 (LANDLOCK_RULE_PATH_BENEATH = 1)
        let add_res = driver.landlock_add_rule(ruleset_fd, 1u32, &path_attr);
        let _ = driver.close_fd(path_fd);

        if let Err(err) = add_res {
            let os_err = err.raw_os_error();
            let _ = driver.close_fd(ruleset_fd);
            tracing::error!(
                "Landlock add_rule failed for path {:?}: {} (os error {:?})",
                rule.path,
                err,
                os_err
            );
            return Err(IsolationError::SandboxInitializationFailed(format!(
                "Landlock add_rule failed for path {:?}: {} (os error {:?})",
                rule.path, err, os_err
            )));
        }
    }

    if let Err(err) = driver.landlock_restrict_self(ruleset_fd) {
        let os_err = err.raw_os_error();
        let _ = driver.close_fd(ruleset_fd);
        tracing::error!(
            "landlock_restrict_self failed: {} (os error {:?})",
            err,
            os_err
        );
        return Err(IsolationError::SandboxInitializationFailed(format!(
            "landlock_restrict_self (syscall 446) failed: {} (os error {:?})",
            err, os_err
        )));
    }

    let _ = driver.close_fd(ruleset_fd);
    Ok(())
}

fn apply_seccomp_filter_with_driver(
    require_tsync: bool,
    driver: &dyn LinuxSyscallDriver,
) -> Result<(), IsolationError> {
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
        SYS_ptrace,
        SYS_process_vm_readv,
        SYS_process_vm_writev,
        SYS_kexec_load,
        SYS_reboot,
        SYS_init_module,
        SYS_finit_module,
        SYS_delete_module,
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

    let tsync_res = driver.seccomp_tsync(&prog);
    match tsync_res {
        Ok(()) => {
            tracing::info!(
                "Seccomp-BPF filter installed with SECCOMP_FILTER_FLAG_TSYNC across all threads"
            );
        }
        Err(tsync_err) => {
            let tsync_os = tsync_err.raw_os_error();
            if require_tsync {
                tracing::error!(
                    "SYS_seccomp with TSYNC failed: {} (os error {:?}); single-thread fallback rejected for strict profile",
                    tsync_err,
                    tsync_os
                );
                return Err(IsolationError::SandboxInitializationFailed(format!(
                    "Seccomp TSYNC installation failed via SYS_seccomp: {} (os error {:?}); single-thread prctl fallback rejected because profile requires thread-wide synchronization",
                    tsync_err, tsync_os
                )));
            }

            // Fallback to prctl only when thread-wide synchronization was not strictly required
            tracing::warn!(
                "SYS_seccomp failed: {} (os error {:?}), attempting prctl fallback",
                tsync_err,
                tsync_os
            );
            match driver.prctl_set_seccomp(&prog) {
                Ok(()) => {
                    tracing::info!(
                        "Seccomp-BPF filter installed via prctl fallback (single-thread confinement)"
                    );
                }
                Err(prctl_err) => {
                    let prctl_os = prctl_err.raw_os_error();
                    tracing::error!(
                        "Seccomp prctl fallback failed: {} (os error {:?})",
                        prctl_err,
                        prctl_os
                    );
                    return Err(IsolationError::SandboxInitializationFailed(format!(
                        "Seccomp filter installation failed: SYS_seccomp failed ({} [os error {:?}]) and prctl fallback failed ({} [os error {:?}])",
                        tsync_err, tsync_os, prctl_err, prctl_os
                    )));
                }
            }
        }
    }

    Ok(())
}

fn apply_resource_limits_with_driver(
    config: &LinuxSandboxConfig,
    driver: &dyn LinuxSyscallDriver,
) -> Result<(), IsolationError> {
    // 1. Apply POSIX RLIMIT_AS (virtual address space bound)
    if let Some(mb) = config.cgroup_limit_memory_mb {
        let bytes = (mb as u64) * 1024 * 1024;
        let rlim = libc::rlimit {
            rlim_cur: bytes,
            rlim_max: bytes,
        };
        if let Err(err) = driver.setrlimit(libc::RLIMIT_AS, &rlim) {
            let os_err = err.raw_os_error();
            if config.strict_resource_limits {
                tracing::error!(
                    "Strict setrlimit(RLIMIT_AS) failed: {} (os error {:?})",
                    err,
                    os_err
                );
                return Err(IsolationError::SandboxInitializationFailed(format!(
                    "setrlimit(RLIMIT_AS) failed: {} (os error {:?})",
                    err, os_err
                )));
            } else {
                tracing::warn!(
                    "setrlimit(RLIMIT_AS) failed: {} (os error {:?})",
                    err,
                    os_err
                );
            }
        }
    }

    // 2. Apply POSIX RLIMIT_NPROC (process/thread creation bound)
    if let Some(pids) = config.cgroup_limit_pids {
        let rlim = libc::rlimit {
            rlim_cur: pids as u64,
            rlim_max: pids as u64,
        };
        if let Err(err) = driver.setrlimit(libc::RLIMIT_NPROC, &rlim) {
            let os_err = err.raw_os_error();
            if config.strict_resource_limits {
                tracing::error!(
                    "Strict setrlimit(RLIMIT_NPROC) failed: {} (os error {:?})",
                    err,
                    os_err
                );
                return Err(IsolationError::SandboxInitializationFailed(format!(
                    "setrlimit(RLIMIT_NPROC) failed: {} (os error {:?})",
                    err, os_err
                )));
            } else {
                tracing::warn!(
                    "setrlimit(RLIMIT_NPROC) failed: {} (os error {:?})",
                    err,
                    os_err
                );
            }
        }
    }

    // 3. Apply Cgroups v2 if hierarchy is present and writeable
    if let Some(mb) = config.cgroup_limit_memory_mb {
        let cgroup_path = Path::new("/sys/fs/cgroup/memory.max");
        if cgroup_path.exists() {
            let bytes = (mb * 1024 * 1024).to_string();
            if let Err(err) = driver.write_cgroup(cgroup_path, &bytes) {
                let os_err = err.raw_os_error();
                if config.strict_resource_limits {
                    tracing::error!(
                        "Strict cgroup memory limit write failed: {} (os error {:?})",
                        err,
                        os_err
                    );
                    return Err(IsolationError::SandboxInitializationFailed(format!(
                        "cgroup memory limit write failed to {}: {} (os error {:?})",
                        cgroup_path.display(),
                        err,
                        os_err
                    )));
                } else {
                    tracing::warn!(
                        "cgroup memory limit write failed: {} (os error {:?})",
                        err,
                        os_err
                    );
                }
            }
        }
    }

    Ok(())
}

/// Applies Linux containment using the real kernel syscall driver.
#[cfg(target_os = "linux")]
pub fn apply_linux_sandbox(config: &LinuxSandboxConfig) -> Result<(), IsolationError> {
    apply_linux_sandbox_with_driver(config, &DefaultLinuxSyscallDriver)
}

/// On non-Linux platforms, Linux kernel sandboxing cannot be applied directly.
#[cfg(not(target_os = "linux"))]
pub fn apply_linux_sandbox(_config: &LinuxSandboxConfig) -> Result<(), IsolationError> {
    Err(IsolationError::UnsupportedPlatform(
        "Linux kernel isolation primitives cannot be applied on non-Linux platforms".to_string(),
    ))
}

/// Mock driver for deterministic testing of syscall failures and regression scenarios.
#[derive(Debug, Default, Clone)]
pub struct MockLinuxSyscallDriver {
    pub unshare_err: Option<i32>,
    pub prctl_no_new_privs_err: Option<i32>,
    pub seccomp_tsync_err: Option<i32>,
    pub prctl_seccomp_err: Option<i32>,
    pub landlock_create_err: Option<i32>,
    pub landlock_create_fd: Option<libc::c_int>,
    pub landlock_add_rule_err: Option<i32>,
    pub landlock_restrict_err: Option<i32>,
    pub open_path_err: Option<i32>,
    pub open_path_fd: Option<libc::c_int>,
    pub close_fd_err: Option<i32>,
    pub write_cgroup_err: Option<i32>,
    pub setrlimit_err: Option<i32>,
}

impl LinuxSyscallDriver for MockLinuxSyscallDriver {
    fn unshare(&self, _flags: libc::c_int) -> Result<(), std::io::Error> {
        if let Some(err) = self.unshare_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn prctl_set_no_new_privs(&self) -> Result<(), std::io::Error> {
        if let Some(err) = self.prctl_no_new_privs_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn seccomp_tsync(&self, _prog: &SockFprog) -> Result<(), std::io::Error> {
        if let Some(err) = self.seccomp_tsync_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn prctl_set_seccomp(&self, _prog: &SockFprog) -> Result<(), std::io::Error> {
        if let Some(err) = self.prctl_seccomp_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn landlock_create_ruleset(
        &self,
        _attr: &LandlockRulesetAttr,
    ) -> Result<libc::c_int, std::io::Error> {
        if let Some(err) = self.landlock_create_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(self.landlock_create_fd.unwrap_or(10))
        }
    }

    fn landlock_add_rule(
        &self,
        _ruleset_fd: libc::c_int,
        _rule_type: u32,
        _attr: &LandlockPathBeneathAttr,
    ) -> Result<(), std::io::Error> {
        if let Some(err) = self.landlock_add_rule_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn landlock_restrict_self(&self, _ruleset_fd: libc::c_int) -> Result<(), std::io::Error> {
        if let Some(err) = self.landlock_restrict_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn open_path(&self, _path: &Path, _flags: libc::c_int) -> Result<libc::c_int, std::io::Error> {
        if let Some(err) = self.open_path_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(self.open_path_fd.unwrap_or(11))
        }
    }

    fn close_fd(&self, _fd: libc::c_int) -> Result<(), std::io::Error> {
        if let Some(err) = self.close_fd_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn write_cgroup(&self, _path: &Path, _content: &str) -> Result<(), std::io::Error> {
        if let Some(err) = self.write_cgroup_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }

    fn setrlimit(
        &self,
        _resource: libc::c_int,
        _rlim: &libc::rlimit,
    ) -> Result<(), std::io::Error> {
        if let Some(err) = self.setrlimit_err {
            Err(std::io::Error::from_raw_os_error(err))
        } else {
            Ok(())
        }
    }
}
