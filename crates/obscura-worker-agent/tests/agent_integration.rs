use obscura_control_plane::{ControlPlane, EventEnvelope, NatsClient, NatsConsumer};
use obscura_isolation::{IsolationPolicy, IsolationProfile};
use obscura_worker::WorkerRequest;
use std::time::Duration;
use uuid::Uuid;

fn test_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:obscura@127.0.0.1:5432/obscura".to_string())
}

fn test_nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

const BOOTSTRAP_SECRET: &str = "phase5-11-integration-secret";

fn ensure_env() {
    std::env::set_var("OBSCURA_SERVICE_BOOTSTRAP_SECRET", BOOTSTRAP_SECRET);
}

#[tokio::test]
async fn test_agent_invalid_requests() {
    // 1. Serialization/deserialization of invalid command payloads
    let invalid_json = r#"{"version": "1.0", "command": {"UnknownCommand": {}}}"#;
    let res = serde_json::from_str::<WorkerRequest>(invalid_json);
    assert!(res.is_err(), "unknown worker command must fail deserialization");

    // 2. Untrusted profile policy check
    let untrusted_policy = IsolationPolicy::for_profile(IsolationProfile::Untrusted);
    let caps = obscura_isolation::PlatformCapabilities::probe();
    if !caps.is_production_safe_for_untrusted {
        let apply_res = untrusted_policy.apply_process_sandbox();
        assert!(
            apply_res.is_err(),
            "untrusted isolation must fail closed on platforms without full kernel namespaces"
        );
    }
}

#[tokio::test]
async fn test_agent_missing_and_invalid_identity() {
    ensure_env();
    let cp = ControlPlane::connect(&test_db_url(), 5).await.expect("cp connect");
    cp.migrate().await.expect("migrate");

    // 1. Service principal with wrong secret must be rejected
    let bad_principal = cp.service_principal("worker-agent", "wrong-secret-value");
    assert!(
        bad_principal.is_err(),
        "authentication with invalid secret must be rejected"
    );

    // 2. Service principal with valid secret succeeds
    let good_principal = cp
        .service_principal("worker-agent", BOOTSTRAP_SECRET)
        .expect("valid principal");
    assert!(good_principal.is_admin());

    // 3. Operation with non-existent tenant must be rejected
    let fake_tenant_id = Uuid::new_v4();
    let fake_job_id = Uuid::new_v4();
    let res = cp.get_job_internal(fake_job_id, fake_tenant_id).await;
    assert!(res.is_err(), "non-existent tenant must be rejected");
}

#[tokio::test]
async fn test_agent_stale_fencing_tokens() {
    ensure_env();
    let cp = ControlPlane::connect(&test_db_url(), 5).await.expect("cp connect");
    cp.migrate().await.expect("migrate");

    let tenant = cp
        .create_tenant(&format!("tenant-fencing-{}", Uuid::new_v4()))
        .await
        .expect("tenant");
    let principal = cp
        .service_principal("worker-agent", BOOTSTRAP_SECRET)
        .expect("principal");

    let worker1_id = cp
        .register_worker(Some(tenant.tenant_id), serde_json::json!({"name": "w1"}), 2)
        .await
        .expect("w1");
    let worker2_id = cp
        .register_worker(Some(tenant.tenant_id), serde_json::json!({"name": "w2"}), 2)
        .await
        .expect("w2");

    let job = cp
        .submit_job_internal(
            tenant.tenant_id,
            "https://example.com/test",
            "snapshot",
            serde_json::json!({}),
            None,
            3,
        )
        .await
        .expect("job");

    let claimed = cp
        .claim_pending_job_for_tenant(Some(tenant.tenant_id))
        .await
        .expect("claim")
        .expect("some claimed job");
    assert_eq!(claimed.job_id, job.job_id);

    // Worker 1 is dispatched -> gets token_a
    let attempt_a = Uuid::new_v4();
    let token_a = cp
        .mark_dispatched(job.job_id, tenant.tenant_id, worker1_id, Uuid::new_v4(), attempt_a)
        .await
        .expect("dispatch 1");

    // Worker 2 takes over ownership (re-scheduled after worker 1 crash or lease expiration)
    let attempt_b = Uuid::new_v4();
    sqlx::query("UPDATE jobs SET status='scheduled', updated_at=now() WHERE job_id=$1")
        .bind(job.job_id)
        .execute(cp.pool())
        .await
        .expect("reschedule");
    let token_b = cp
        .mark_dispatched(job.job_id, tenant.tenant_id, worker2_id, Uuid::new_v4(), attempt_b)
        .await
        .expect("dispatch 2");

    assert!(token_b > token_a, "fencing token must advance on each dispatch");

    // Worker 1 attempts to complete job using stale token_a -> MUST BE REJECTED
    let stale_complete = cp
        .complete_job(
            &principal,
            job.job_id,
            tenant.tenant_id,
            attempt_a,
            token_a,
            serde_json::json!({"from": "Worker 1 stale"}),
        )
        .await;
    assert!(
        stale_complete.is_err(),
        "completion with stale fencing token must be rejected"
    );

    // Worker 1 attempts to fail job using stale token_a -> MUST BE REJECTED
    let stale_fail = cp
        .fail_job(
            &principal,
            job.job_id,
            tenant.tenant_id,
            attempt_a,
            token_a,
            "ERR",
            "stale fail",
            false,
        )
        .await;
    assert!(
        stale_fail.is_err(),
        "failure reporting with stale fencing token must be rejected"
    );

    // Worker 2 completes job with current token_b -> MUST SUCCEED
    let valid_complete = cp
        .complete_job(
            &principal,
            job.job_id,
            tenant.tenant_id,
            attempt_b,
            token_b,
            serde_json::json!({"from": "Worker 2 active"}),
        )
        .await;
    assert!(
        valid_complete.is_ok(),
        "completion with current fencing token must succeed"
    );
}

#[tokio::test]
async fn test_agent_lease_loss_watchdog() {
    ensure_env();
    let cp = ControlPlane::connect(&test_db_url(), 5).await.expect("cp connect");
    cp.migrate().await.expect("migrate");

    let tenant = cp
        .create_tenant(&format!("tenant-leaseloss-{}", Uuid::new_v4()))
        .await
        .expect("tenant");
    let principal = cp
        .service_principal("worker-agent", BOOTSTRAP_SECRET)
        .expect("principal");

    let worker_id = cp
        .register_worker(Some(tenant.tenant_id), serde_json::json!({"name": "w-loss"}), 2)
        .await
        .expect("worker");

    let job = cp
        .submit_job_internal(
            tenant.tenant_id,
            "https://example.com/slow",
            "snapshot",
            serde_json::json!({}),
            None,
            3,
        )
        .await
        .expect("job");

    let _claimed = cp
        .claim_pending_job_for_tenant(Some(tenant.tenant_id))
        .await
        .expect("claim");

    let attempt_id = Uuid::new_v4();
    let fencing_token = cp
        .mark_dispatched(job.job_id, tenant.tenant_id, worker_id, Uuid::new_v4(), attempt_id)
        .await
        .expect("dispatched");

    let lease = cp
        .acquire_lease("job", &job.job_id.to_string(), worker_id, 1) // 1 second TTL
        .await
        .expect("lease");

    // Renewal task fails when lease is revoked/expired
    let (loss_tx, mut loss_rx) = tokio::sync::oneshot::channel::<()>();
    let cp_renew = cp.clone();
    let lease_id = lease.lease_id;

    // Simulate renewal task that detects lease expiry
    let renew_handle = tokio::spawn(async move {
        // Wait 1.5 seconds so lease expires
        tokio::time::sleep(Duration::from_millis(1500)).await;
        // Attempt renewal after expiration: fails
        let res = cp_renew.renew_lease(lease_id, 1).await;
        if res.is_err() {
            let _ = loss_tx.send(());
        }
    });

    // Agent execution select loop
    let job_failed_with_lease_loss = tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("watchdog timed out");
        }
        _ = &mut loss_rx => {
            renew_handle.abort();
            // Ownership lost: fail job
            let res = cp.fail_job(
                &principal,
                job.job_id,
                tenant.tenant_id,
                attempt_id,
                fencing_token,
                "LEASE_LOST",
                "ownership lost",
                true,
            ).await;
            res.is_ok()
        }
    };

    assert!(
        job_failed_with_lease_loss,
        "lease loss watchdog must fail the job with LEASE_LOST"
    );
}

#[tokio::test]
async fn test_agent_worker_crash_handling() {
    ensure_env();
    let cp = ControlPlane::connect(&test_db_url(), 5).await.expect("cp connect");
    cp.migrate().await.expect("migrate");

    let tenant = cp
        .create_tenant(&format!("tenant-crash-{}", Uuid::new_v4()))
        .await
        .expect("tenant");
    let principal = cp
        .service_principal("worker-agent", BOOTSTRAP_SECRET)
        .expect("principal");

    let worker_id = cp
        .register_worker(Some(tenant.tenant_id), serde_json::json!({"name": "w-crash"}), 2)
        .await
        .expect("worker");

    let job = cp
        .submit_job_internal(
            tenant.tenant_id,
            "https://example.com/crash",
            "snapshot",
            serde_json::json!({}),
            None,
            3,
        )
        .await
        .expect("job");

    let _claimed = cp
        .claim_pending_job_for_tenant(Some(tenant.tenant_id))
        .await
        .expect("claim");

    let attempt_id = Uuid::new_v4();
    let fencing_token = cp
        .mark_dispatched(job.job_id, tenant.tenant_id, worker_id, Uuid::new_v4(), attempt_id)
        .await
        .expect("dispatched");

    // Simulate worker process crashing during request execution
    let simulated_error = "Worker process terminated unexpectedly (SIGKILL/EOF)";
    let fail_res = cp
        .fail_job(
            &principal,
            job.job_id,
            tenant.tenant_id,
            attempt_id,
            fencing_token,
            "COMM_ERROR",
            simulated_error,
            false,
        )
        .await;

    assert!(fail_res.is_ok(), "agent must record failure on worker crash");
    let updated = cp.get_job_internal(job.job_id, tenant.tenant_id).await.expect("job");
    assert_eq!(updated.status, "failed");
}

#[tokio::test]
async fn test_agent_nats_redelivery() {
    let nats = NatsClient::connect(&test_nats_url()).await.expect("nats connect");
    let stream_name = format!("TEST_AGENT_STREAM_{}", Uuid::new_v4().simple());
    let subject = format!("obscura.agent.{}.jobs", Uuid::new_v4().simple());

    nats.ensure_stream(&stream_name, vec![subject.clone()])
        .await
        .expect("ensure_stream");

    let env = EventEnvelope::new(
        "job.dispatch",
        "worker-agent",
        "dispatch-1",
        serde_json::json!({
            "job_id": Uuid::new_v4(),
            "task": "render"
        }),
    );
    nats.publish(&subject, &env).await.expect("publish");

    let consumer = NatsConsumer::create_with_ack_wait(
        nats.jetstream(),
        &stream_name,
        &format!("consumer-{}", Uuid::new_v4().simple()),
        Duration::from_secs(30),
    )
    .await
    .expect("consumer");

    // Fetch 1st time
    let msg1 = consumer.next_message().await.expect("next").expect("some msg");

    // Negative acknowledgement (simulating worker crash or agent restart)
    msg1.ack_with(async_nats::jetstream::AckKind::Nak(None))
        .await
        .expect("nak");

    // Fetch redelivered message
    let msg2 = consumer.next_message().await.expect("next").expect("redelivered msg");
    let info = msg2.info().expect("info");
    assert!(
        info.delivered > 1,
        "delivery count must be > 1 after nak, got {}",
        info.delivered
    );
    msg2.ack().await.expect("ack");
}
