//! Standalone executable for the isolated runtime worker.

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // On Linux, ensure this worker dies if its parent (agent/supervisor) dies.
    #[cfg(target_os = "linux")]
    unsafe {
        libc::prctl(1, 9); // PR_SET_PDEATHSIG = 1, SIGKILL = 9
    }

    // Apply OS-level process sandboxing based on environment profile before signaling readiness
    let profile = std::env::var("OBSCURA_ISOLATION_PROFILE")
        .ok()
        .and_then(|p| match p.to_lowercase().as_str() {
            "untrusted" => Some(obscura_isolation::IsolationProfile::Untrusted),
            "hardened" => Some(obscura_isolation::IsolationProfile::Hardened),
            "standard" => Some(obscura_isolation::IsolationProfile::Standard),
            _ => None,
        })
        .unwrap_or(obscura_isolation::IsolationProfile::Standard);

    let policy = obscura_isolation::IsolationPolicy::for_profile(profile);
    if let Err(e) = policy.apply_process_sandbox() {
        eprintln!("Failed to initialize worker sandbox: {e}");
        std::process::exit(1);
    }

    // Signal readiness to the supervisor before entering the request loop.
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{{\"ready\":true}}");
    let _ = std::io::stdout().flush();

    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_writer(std::io::stderr)
        .init();

    obscura_worker::run_worker_loop().await;
}
