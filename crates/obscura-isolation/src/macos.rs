//! macOS / Darwin isolation: Apple Seatbelt sandbox integration and resource limits.

use crate::IsolationError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DarwinSandboxConfig {
    /// Custom Seatbelt profile definition in TinyScheme syntax.
    pub seatbelt_profile: Option<String>,
    /// Address space memory limit in megabytes.
    pub rlimit_as_mb: Option<usize>,
    /// Max open file descriptors.
    pub rlimit_nofile: Option<u64>,
}

impl Default for DarwinSandboxConfig {
    fn default() -> Self {
        Self {
            seatbelt_profile: None,
            rlimit_as_mb: Some(512),
            rlimit_nofile: Some(256),
        }
    }
}

/// Applies macOS containment primitives (rlimits and Seatbelt sandbox if configured).
#[cfg(target_os = "macos")]
pub fn apply_darwin_sandbox(config: &DarwinSandboxConfig) -> Result<(), IsolationError> {
    // 1. Apply RLIMIT_AS (virtual memory limit)
    if let Some(mb) = config.rlimit_as_mb {
        let bytes = (mb as u64) * 1024 * 1024;
        let rlim = libc::rlimit {
            rlim_cur: bytes,
            rlim_max: bytes,
        };
        unsafe {
            if libc::setrlimit(libc::RLIMIT_AS, &rlim) != 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!("setrlimit(RLIMIT_AS) failed: {}", err);
            }
        }
    }

    // 2. Apply RLIMIT_NOFILE
    if let Some(nofile) = config.rlimit_nofile {
        let rlim = libc::rlimit {
            rlim_cur: nofile,
            rlim_max: nofile,
        };
        unsafe {
            if libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) != 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!("setrlimit(RLIMIT_NOFILE) failed: {}", err);
            }
        }
    }

    // 3. Apply Seatbelt profile if provided
    if let Some(profile) = &config.seatbelt_profile {
        extern "C" {
            fn sandbox_init(
                profile: *const libc::c_char,
                flags: u64,
                errorbuf: *mut *mut libc::c_char,
            ) -> libc::c_int;
            fn sandbox_free_error(errorbuf: *mut libc::c_char);
        }

        let c_profile = std::ffi::CString::new(profile.as_str()).map_err(|e| {
            IsolationError::SandboxInitializationFailed(format!("Invalid Seatbelt profile: {}", e))
        })?;

        let mut errorbuf: *mut libc::c_char = std::ptr::null_mut();
        let res = unsafe { sandbox_init(c_profile.as_ptr(), 0, &mut errorbuf) };

        if res != 0 {
            let msg = if !errorbuf.is_null() {
                let s = unsafe { std::ffi::CStr::from_ptr(errorbuf) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { sandbox_free_error(errorbuf) };
                s
            } else {
                std::io::Error::last_os_error().to_string()
            };
            return Err(IsolationError::SandboxInitializationFailed(format!(
                "sandbox_init failed: {}",
                msg
            )));
        }
    }

    tracing::info!("macOS process sandboxing applied (evaluation / dev mode)");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn apply_darwin_sandbox(_config: &DarwinSandboxConfig) -> Result<(), IsolationError> {
    Err(IsolationError::UnsupportedPlatform(
        "Darwin Seatbelt primitives cannot be applied on non-macOS platforms".to_string(),
    ))
}
