//! Platform capability detection for OS-level sandboxing and isolation.

use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::path::Path;

/// Describes the OS-level security and containment features available on the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    /// Operating system name ("linux", "macos", "windows", "other").
    pub os: String,
    /// Kernel architecture (e.g. "x86_64", "aarch64").
    pub arch: String,
    /// Linux user namespaces (`CLONE_NEWUSER`).
    pub has_user_namespaces: bool,
    /// Linux PID namespaces (`CLONE_NEWPID`).
    pub has_pid_namespaces: bool,
    /// Linux network namespaces (`CLONE_NEWNET`).
    pub has_net_namespaces: bool,
    /// Landlock LSM unprivileged filesystem access control.
    pub has_landlock: bool,
    /// Seccomp-BPF system call filtering.
    pub has_seccomp: bool,
    /// Unified Cgroups v2 hierarchy for CPU, memory, and PID limits.
    pub has_cgroups_v2: bool,
    /// macOS Seatbelt / `sandbox_init` interface.
    pub has_seatbelt: bool,
    /// Whether this environment satisfies the mandatory requirements for untrusted execution.
    pub is_production_safe_for_untrusted: bool,
}

impl PlatformCapabilities {
    /// Probe the current host system to detect available OS sandboxing primitives.
    pub fn probe() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::probe_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::probe_macos()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self {
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
                has_user_namespaces: false,
                has_pid_namespaces: false,
                has_net_namespaces: false,
                has_landlock: false,
                has_seccomp: false,
                has_cgroups_v2: false,
                has_seatbelt: false,
                is_production_safe_for_untrusted: false,
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn probe_linux() -> Self {
        let has_user_ns = Path::new("/proc/self/ns/user").exists()
            && std::fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone")
                .map(|val| val.trim() == "1")
                .unwrap_or(true);

        let has_pid_ns = Path::new("/proc/self/ns/pid").exists();
        let has_net_ns = Path::new("/proc/self/ns/net").exists();

        // Check for Landlock ABI version
        let has_landlock = unsafe {
            // SYS_landlock_create_ruleset = 444 on x86_64, aarch64
            // Passing NULL attr and LANDLOCK_CREATE_RULESET_VERSION (1 << 0)
            let res = libc::syscall(444, std::ptr::null::<()>(), 0, 1u32);
            res >= 1
        };

        // Check for Seccomp support via prctl
        let has_seccomp = unsafe {
            // PR_GET_SECCOMP = 21
            libc::prctl(21, 0, 0, 0, 0) >= 0
        };

        // Check for unified cgroups v2
        let has_cgroups_v2 = Path::new("/sys/fs/cgroup/cgroup.controllers").exists();

        let is_safe = has_user_ns && has_pid_ns && has_net_ns && (has_landlock || has_seccomp);

        Self {
            os: "linux".to_string(),
            arch: std::env::consts::ARCH.to_string(),
            has_user_namespaces: has_user_ns,
            has_pid_namespaces: has_pid_ns,
            has_net_namespaces: has_net_ns,
            has_landlock,
            has_seccomp,
            has_cgroups_v2,
            has_seatbelt: false,
            is_production_safe_for_untrusted: is_safe,
        }
    }

    #[cfg(target_os = "macos")]
    fn probe_macos() -> Self {
        // macOS provides libsystem_sandbox with sandbox_init, but lacks Linux namespaces/landlock
        Self {
            os: "macos".to_string(),
            arch: std::env::consts::ARCH.to_string(),
            has_user_namespaces: false,
            has_pid_namespaces: false,
            has_net_namespaces: false,
            has_landlock: false,
            has_seccomp: false,
            has_cgroups_v2: false,
            has_seatbelt: true,
            // macOS is development/evaluation only; not safe for untrusted hostile multi-tenant execution
            is_production_safe_for_untrusted: false,
        }
    }

    /// Formats a diagnostic human-readable summary of sandbox capability.
    pub fn summary(&self) -> String {
        format!(
            "OS: {} ({}), UserNS: {}, PidNS: {}, NetNS: {}, Landlock: {}, Seccomp: {}, CgroupsV2: {}, Seatbelt: {}, ProductionUntrustedSafe: {}",
            self.os,
            self.arch,
            self.has_user_namespaces,
            self.has_pid_namespaces,
            self.has_net_namespaces,
            self.has_landlock,
            self.has_seccomp,
            self.has_cgroups_v2,
            self.has_seatbelt,
            self.is_production_safe_for_untrusted
        )
    }
}
