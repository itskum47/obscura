use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_v8_runtime_timeout_and_recovery() {
    let mut runtime = obscura_js::runtime::ObscuraJsRuntime::new();

    // 1. Run an infinite loop with a short timeout (300ms)
    let start = std::time::Instant::now();
    let res = runtime.evaluate_with_timeout("while(true){}", Duration::from_millis(300));
    let elapsed = start.elapsed();

    assert!(res.is_err(), "Expected timeout error, got: {:?}", res);
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("timed out") || err_msg.contains("execution terminated"),
        "Unexpected error message: {}",
        err_msg
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "Timeout watchdog took too long: {:?}",
        elapsed
    );

    // 2. Immediate subsequent evaluation MUST succeed on the same isolate
    let recovery = runtime.evaluate_with_timeout("20 + 22", Duration::from_millis(500));
    assert!(
        recovery.is_ok(),
        "Runtime failed to recover after watchdog timeout: {:?}",
        recovery
    );
    let val = recovery.unwrap();
    assert_eq!(val.as_f64(), Some(42.0));

    // 3. Repeated cycle: verify that repeated timeouts followed by successes
    // on the EXACT same runtime isolate maintain bounded execution and clean recovery
    for cycle in 1..=5 {
        let t_start = std::time::Instant::now();
        let loop_res = runtime.evaluate_with_timeout("while(true){}", Duration::from_millis(150));
        let t_elapsed = t_start.elapsed();
        assert!(loop_res.is_err(), "Cycle {cycle} infinite loop must fail");
        assert!(t_elapsed < Duration::from_secs(2), "Cycle {cycle} watchdog took too long: {:?}", t_elapsed);

        let ok_res = runtime.evaluate_with_timeout(&format!("{cycle} * 10"), Duration::from_millis(300));
        assert!(ok_res.is_ok(), "Cycle {cycle} recovery failed: {:?}", ok_res);
        assert_eq!(ok_res.unwrap().as_f64(), Some((cycle * 10) as f64));
    }
}

#[tokio::test]
async fn test_page_try_evaluate_with_timeout() {
    let context = Arc::new(obscura_browser::BrowserContext::with_storage_and_network(
        "test-v8-timeout".to_string(),
        None,
        false,
        None,
        None,
        true,
    ));
    let mut page = obscura_browser::Page::new("test-v8-timeout-page".to_string(), context);
    let _ = page.navigate("about:blank").await;

    // 1. Verify runaway script fails with an error rather than returning Null or success
    let res = page.try_evaluate_with_timeout("while(true){}", Duration::from_millis(300));
    assert!(
        res.is_err(),
        "Expected try_evaluate_with_timeout to fail on infinite loop, got: {:?}",
        res
    );
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("timed out") || err_msg.contains("execution terminated"),
        "Unexpected error: {}",
        err_msg
    );

    // 2. Recovery on the page's JS runtime
    let recovery = page.try_evaluate_with_timeout("100 * 2", Duration::from_millis(500));
    assert!(
        recovery.is_ok(),
        "Page JS failed to recover after watchdog timeout: {:?}",
        recovery
    );
    let val = recovery.unwrap();
    assert_eq!(val.as_f64(), Some(200.0));
}

#[tokio::test]
async fn test_cdp_evaluate_with_timeout_infinite_loop() {
    let context = Arc::new(obscura_browser::BrowserContext::with_storage_and_network(
        "test-cdp-timeout".to_string(),
        None,
        false,
        None,
        None,
        true,
    ));
    let mut page = obscura_browser::Page::new("test-cdp-timeout-page".to_string(), context);
    let _ = page.navigate("about:blank").await;

    // 1. Run runaway script through evaluate_for_cdp_with_timeout
    let res = page
        .evaluate_for_cdp_with_timeout("while(true){}", true, false, 300)
        .await;
    assert!(
        res.is_err(),
        "Expected CDP evaluation to fail on infinite loop, got: {:?}",
        res
    );

    // 2. Recovery on subsequent CDP evaluation
    let recovery = page
        .evaluate_for_cdp_with_timeout("40 + 2", true, false, 500)
        .await;
    assert!(
        recovery.is_ok(),
        "CDP evaluation failed to recover after timeout: {:?}",
        recovery
    );
    let obj = recovery.unwrap();
    assert_eq!(obj.value.and_then(|v| v.as_f64()), Some(42.0));
}

#[test]
fn test_cli_fetch_eval_timeout_fails_closed() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_obscura"));
    cmd.args(["fetch", "about:blank", "--eval", "while(true){}", "--timeout", "1"]);

    let output = cmd.output().expect("execute obscura fetch");
    assert!(
        !output.status.success(),
        "CLI fetch with infinite loop must fail (exit non-zero), but exited with success! stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("evaluation failed") || stderr.contains("timed out"),
        "Expected error message on stderr, got: {}",
        stderr
    );
}

#[test]
fn test_cli_fetch_eval_success() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_obscura"));
    cmd.args(["fetch", "about:blank", "--eval", "30 + 12", "--timeout", "5"]);

    let output = cmd.output().expect("execute obscura fetch");
    assert!(
        output.status.success(),
        "CLI fetch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: f64 = stdout.trim().parse().expect("parsed number");
    assert_eq!(parsed, 42.0);
}
