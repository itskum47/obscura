//! Standalone executable alias for the isolated runtime worker.

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_writer(std::io::stderr)
        .init();

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

    obscura_worker::run_worker_loop().await;
}
