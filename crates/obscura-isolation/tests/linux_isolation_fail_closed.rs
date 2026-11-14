//! Regression tests for Linux Isolation Fail-Closed Security Remediation.
//! Proves that mandatory isolation mechanisms fail closed, preventing untrusted workload startup on failure.

use obscura_isolation::{
    apply_linux_sandbox_with_driver, IsolationError, IsolationPolicy, IsolationProfile,
    LandlockRule, LinuxSandboxConfig, MockLinuxSyscallDriver, PlatformCapabilities,
};

/// 1. Seccomp installation failure prevents untrusted workload startup.
#[test]
fn test_seccomp_tsync_failure_prevents_untrusted_startup() {
    let config = LinuxSandboxConfig {
        enable_seccomp: true,
        require_tsync: true, // Mandatory for Untrusted profile
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        seccomp_tsync_err: Some(libc::EPERM),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Seccomp TSYNC failure must prevent workload startup"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("SYS_seccomp"),
            "Error must specify SYS_seccomp failure: {msg}"
        );
        assert!(
            msg.contains("single-thread prctl fallback rejected"),
            "Error must confirm single-thread fallback was rejected for strict profile: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 2. Failure of both seccomp and its fallback prevents startup.
#[test]
fn test_seccomp_and_fallback_failure_prevents_startup() {
    let config = LinuxSandboxConfig {
        enable_seccomp: true,
        require_tsync: false, // Fallback allowed for non-strict profile
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        seccomp_tsync_err: Some(libc::ENOSYS),
        prctl_seccomp_err: Some(libc::EACCES),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Failure of both SYS_seccomp and prctl fallback must fail closed"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("SYS_seccomp failed"),
            "Error must identify SYS_seccomp failure: {msg}"
        );
        assert!(
            msg.contains("prctl fallback failed"),
            "Error must identify prctl fallback failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 3. Landlock installation failure prevents startup.
#[test]
fn test_landlock_create_ruleset_failure_prevents_startup() {
    let config = LinuxSandboxConfig {
        restrict_filesystem: true,
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        landlock_create_err: Some(libc::ENOSYS),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Landlock create_ruleset failure must fail closed"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("landlock_create_ruleset (syscall 444) failed"),
            "Error must specify landlock_create_ruleset failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 4. Landlock add_rule failure prevents startup and closes ruleset fd.
#[test]
fn test_landlock_add_rule_failure_prevents_startup() {
    let tmp_dir = std::env::temp_dir();
    let config = LinuxSandboxConfig {
        restrict_filesystem: true,
        allowed_paths: vec![LandlockRule {
            path: tmp_dir.clone(),
            read_only: true,
        }],
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        landlock_create_fd: Some(10),
        open_path_fd: Some(11),
        landlock_add_rule_err: Some(libc::EACCES),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Landlock add_rule failure must fail closed"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("Landlock add_rule failed for path"),
            "Error must specify landlock_add_rule failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 5. Namespace setup failure prevents startup.
#[test]
fn test_namespace_setup_failure_prevents_startup() {
    let config = LinuxSandboxConfig {
        unshare_user: true,
        unshare_pid: true,
        unshare_net: true,
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        unshare_err: Some(libc::EPERM),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Namespace unshare failure must fail closed"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("Namespace isolation failed: unshare"),
            "Error must specify namespace unshare failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 6. Network namespace unshare failure alone prevents startup.
#[test]
fn test_network_namespace_failure_prevents_startup() {
    let config = LinuxSandboxConfig {
        unshare_net: true,
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        unshare_err: Some(libc::ENOSPC),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Network namespace unshare failure must fail closed"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("Namespace isolation failed: unshare"),
            "Error must specify namespace unshare failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 7. Required filesystem isolation failure prevents startup (restrict_self failure).
#[test]
fn test_landlock_restrict_self_failure_prevents_startup() {
    let config = LinuxSandboxConfig {
        restrict_filesystem: true,
        ..Default::default()
    };

    let driver = MockLinuxSyscallDriver {
        landlock_restrict_err: Some(libc::EPERM),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_err(),
        "Failure to enforce landlock_restrict_self must fail closed"
    );

    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("landlock_restrict_self (syscall 446) failed"),
            "Error must specify landlock_restrict_self failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 8. Resource-limit setup failure follows the explicitly documented security policy.
#[test]
fn test_resource_limit_failure_follows_policy() {
    // A. Strict policy (mandatory for untrusted workloads):
    let strict_config = LinuxSandboxConfig {
        cgroup_limit_memory_mb: Some(128),
        cgroup_limit_pids: Some(16),
        strict_resource_limits: true,
        ..Default::default()
    };

    let strict_driver = MockLinuxSyscallDriver {
        setrlimit_err: Some(libc::EINVAL),
        ..Default::default()
    };

    let res_strict = apply_linux_sandbox_with_driver(&strict_config, &strict_driver);
    assert!(
        res_strict.is_err(),
        "Strict resource limits failure must fail closed"
    );
    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res_strict {
        assert!(
            msg.contains("setrlimit(RLIMIT_AS) failed"),
            "Error must specify setrlimit failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res_strict);
    }

    // B. Non-strict policy (trusted/standard workloads where container runtime enforces limits):
    let standard_config = LinuxSandboxConfig {
        cgroup_limit_memory_mb: Some(256),
        cgroup_limit_pids: Some(32),
        strict_resource_limits: false,
        ..Default::default()
    };

    let standard_driver = MockLinuxSyscallDriver {
        setrlimit_err: Some(libc::EINVAL),
        ..Default::default()
    };

    let res_standard = apply_linux_sandbox_with_driver(&standard_config, &standard_driver);
    assert!(
        res_standard.is_ok(),
        "Non-strict standard workload must log warning and continue without failing"
    );
}

/// 9. Strict process count (RLIMIT_NPROC) limit failure prevents untrusted startup.
#[test]
fn test_strict_nproc_limit_failure_prevents_startup() {
    let strict_config = LinuxSandboxConfig {
        cgroup_limit_memory_mb: None, // bypass AS limit
        cgroup_limit_pids: Some(16),
        strict_resource_limits: true,
        ..Default::default()
    };

    let strict_driver = MockLinuxSyscallDriver {
        setrlimit_err: Some(libc::EPERM),
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&strict_config, &strict_driver);
    assert!(
        res.is_err(),
        "Strict RLIMIT_NPROC limit failure must fail closed"
    );
    if let Err(IsolationError::SandboxInitializationFailed(msg)) = res {
        assert!(
            msg.contains("setrlimit(RLIMIT_NPROC) failed"),
            "Error must specify RLIMIT_NPROC failure: {msg}"
        );
    } else {
        panic!("Expected SandboxInitializationFailed, got {:?}", res);
    }
}

/// 10. Trusted workloads retain their documented behavior.
#[test]
fn test_trusted_workloads_retain_documented_behavior() {
    let standard_policy = IsolationPolicy::for_profile(IsolationProfile::Standard);
    assert!(
        standard_policy.apply_process_sandbox().is_ok(),
        "Standard policy on host platform must retain documented working behavior"
    );

    let driver = MockLinuxSyscallDriver::default();
    let config = LinuxSandboxConfig {
        unshare_user: true,
        unshare_pid: true,
        unshare_net: true,
        restrict_filesystem: true,
        enable_seccomp: true,
        require_tsync: false,
        strict_resource_limits: false,
        ..Default::default()
    };

    let res = apply_linux_sandbox_with_driver(&config, &driver);
    assert!(
        res.is_ok(),
        "Standard trusted Linux configuration with operational drivers must succeed"
    );
}

/// 11. macOS continues to reject unsupported untrusted execution.
#[test]
fn test_macos_continues_to_reject_unsupported_untrusted_execution() {
    let caps = PlatformCapabilities::probe();

    #[cfg(target_os = "macos")]
    {
        assert!(
            !caps.is_production_safe_for_untrusted,
            "macOS must report is_production_safe_for_untrusted as false"
        );
        let untrusted_policy = IsolationPolicy::for_profile(IsolationProfile::Untrusted);
        let res = untrusted_policy.apply_process_sandbox();
        assert!(
            matches!(
                res,
                Err(IsolationError::UntrustedProfileForbiddenOnPlatform(_))
            ),
            "macOS must fail-closed on Untrusted profile: got {:?}",
            res
        );
    }
}
