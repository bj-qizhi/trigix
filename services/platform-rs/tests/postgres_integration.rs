// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Postgres integration tests for the platform's `Postgres*Store` implementations.
//!
//! The unit-test suite exercises the in-memory stores, which never touch SQL or
//! Postgres column types — so a mismatch like binding a bigint into a `timestamptz`
//! column (which once broke every registration) passes unit tests yet fails in
//! production. These tests run the real migrations against a real Postgres and
//! drive the Postgres store paths so that drift is caught in CI.
//!
//! They are gated on `TEST_DATABASE_URL`: when it is unset (the default for a
//! plain `cargo test`) every test no-ops, so the suite stays fast and needs no
//! Docker locally. CI sets it to a `pgvector/pgvector` service.
//!
//! Multi-thread flavor is required: the Postgres stores use
//! `tokio::task::block_in_place`, which panics on the current-thread runtime.

use desktop_protocol::{
    CommandOutcome, DesktopAction, DesktopCommand, DesktopCommandAcknowledgement,
    DesktopCommandResult, DeviceCapability, DeviceDescriptor, DeviceState, ExecutionLease,
    Heartbeat,
};
use trigix_platform::affiliate::{AffiliateStore, PlatformAffiliateStore, PostgresAffiliateStore};
use trigix_platform::attribution::{
    AttributionRecord, AttributionStore, CurrencyRevenue, PlatformAttributionStore,
    PostgresAttributionStore,
};
use trigix_platform::billing::{BillingStore, PlatformBillingStore, TenantQuota};
use trigix_platform::desktop_commands::PlatformDesktopCommandStore;
use trigix_platform::desktop_evidence::{
    prepare_evidence, EvidenceKind, EvidencePolicy, EvidenceUploadRequest,
    PlatformDesktopEvidenceStore, RedactionAttestation, SelectorStrategy,
};
use trigix_platform::device_pairing::{CreatePairingSessionRequest, PlatformDevicePairingStore};
use trigix_platform::execution::{ExecutionStore, PostgresExecutionStore, StartExecutionRequest};
use trigix_platform::token_usage::{
    PlatformTokenUsageStore, PostgresTokenUsageStore, TokenUsageRecord, TokenUsageStore,
};
use trigix_platform::users::{PlatformUserStore, UserStore};
use workflow_core::{Node, NodeType, WorkflowGraph};

/// Connects to `TEST_DATABASE_URL` and runs all migrations, or returns `None`
/// (and prints a skip notice) when the env var is unset. `sqlx::migrate` takes a
/// Postgres advisory lock, so concurrent test setups are safe.
async fn setup() -> Option<sqlx::PgPool> {
    let url = match std::env::var("TEST_DATABASE_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            eprintln!("skipping: TEST_DATABASE_URL not set");
            return None;
        }
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect TEST_DATABASE_URL");
    sqlx::migrate!("../../infra/postgres/migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    Some(pool)
}

fn uniq(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
}

/// Polls a synchronous predicate until it holds or ~3s elapses. Several billing
/// writes are fire-and-forget (`tokio::spawn`), so reads may lag the call.
async fn eventually(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..30 {
        if check() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    check()
}

#[tokio::test(flavor = "multi_thread")]
async fn tenant_rls_policy_covers_every_tenant_table() {
    let Some(pool) = setup().await else { return };
    let tenant_tables: i64 = sqlx::query_scalar(
        r#"SELECT count(DISTINCT c.table_name)
           FROM information_schema.columns c
           JOIN information_schema.tables t
             ON t.table_schema = c.table_schema AND t.table_name = c.table_name
           WHERE c.table_schema = 'public' AND c.column_name = 'tenant_id'
             AND t.table_type = 'BASE TABLE'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("count tenant tables");
    let policies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_policies WHERE schemaname = 'public' AND policyname = 'tenant_isolation'",
    )
    .fetch_one(&pool)
    .await
    .expect("count tenant policies");
    assert_eq!(policies, tenant_tables);
}

#[tokio::test(flavor = "multi_thread")]
async fn execution_transitions_are_durable_and_terminal_state_wins() {
    let Some(pool) = setup().await else { return };
    let store = PostgresExecutionStore::new(pool.clone());
    let tenant_id = uniq("transition-tenant");
    let request = || StartExecutionRequest {
        tenant_id: tenant_id.clone(),
        workflow_id: uniq("workflow"),
        workflow_version_id: "version-1".to_string(),
        graph: WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![Node {
                id: "trigger".to_string(),
                node_type: NodeType::Trigger,
                config: None,
            }],
            edges: vec![],
            input_schema: vec![],
            output_schema: vec![],
        },
        input_json: "{}".to_string(),
        label: None,
        callback_url: None,
        trigger_type: None,
        dry_run: false,
        retried_from: None,
    };

    let completed = store.create(request()).await.expect("create execution");
    store
        .complete(
            &tenant_id,
            &completed.id,
            execution_core::ExecutionReport {
                execution_id: completed.id.clone(),
                status: execution_core::ExecutionStatus::Succeeded,
                node_results: vec![],
            },
        )
        .await
        .expect("complete execution");

    let failed = store.create(request()).await.expect("create execution");
    store
        .fail(&tenant_id, &failed.id, "worker stopped".to_string())
        .await
        .expect("fail execution");
    let late = store
        .complete(
            &tenant_id,
            &failed.id,
            execution_core::ExecutionReport {
                execution_id: failed.id.clone(),
                status: execution_core::ExecutionStatus::Succeeded,
                node_results: vec![],
            },
        )
        .await
        .expect("ignore late completion");
    assert_eq!(late.status, execution_core::ExecutionStatus::Failed);

    let transitions: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT execution_id, to_status, from_status
           FROM af_execution_state_transitions
           WHERE tenant_id = $1
           ORDER BY execution_id"#,
    )
    .bind(&tenant_id)
    .fetch_all(&pool)
    .await
    .expect("load transition history");
    assert_eq!(transitions.len(), 2);
    assert!(transitions.iter().any(|(id, to, from)| {
        id == &completed.id && to == "succeeded" && from.as_deref() == Some("running")
    }));
    assert!(transitions.iter().any(|(id, to, from)| {
        id == &failed.id && to == "failed" && from.as_deref() == Some("running")
    }));
}

/// The original regression: `af_users.created_at` is `timestamptz`, and a signup
/// must succeed and round-trip a sane epoch (binding a raw bigint used to fail).
#[tokio::test(flavor = "multi_thread")]
async fn users_create_verify_find_roundtrip() {
    let Some(pool) = setup().await else { return };
    let store = PlatformUserStore::postgres(pool);

    let email = uniq("user") + "@example.com";
    let tenant = uniq("tenant");
    let created = store
        .create(&email, "s3cret-pw", Some("Integration User"), &tenant)
        .expect("create user should succeed against Postgres");
    assert_eq!(created.email, email);
    assert!(
        created.created_at > 1_600_000_000,
        "created_at should be a real unix epoch, got {}",
        created.created_at
    );

    // Correct + wrong password.
    let verified = store
        .verify_password(&email, "s3cret-pw")
        .expect("verify_password with correct password");
    assert_eq!(verified.id, created.id);
    assert!(store.verify_password(&email, "wrong").is_err());

    // Lookups round-trip.
    let by_email = store.find_by_email(&email).expect("find_by_email");
    assert_eq!(by_email.id, created.id);
    assert_eq!(by_email.created_at, created.created_at);
    assert!(store.find_by_id(&created.id).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn desktop_pairing_is_atomic_tenant_scoped_and_single_use() {
    let Some(pool) = setup().await else { return };
    let store = PlatformDevicePairingStore::postgres(pool.clone());
    let device_id = uniq("desktop-device");
    let tenant_id = uniq("desktop-tenant");
    let session = store
        .create_session(CreatePairingSessionRequest {
            device: DeviceDescriptor {
                device_id: device_id.clone(),
                display_name: "Postgres Test Device".to_string(),
                operating_system: "windows".to_string(),
                agent_version: "1.0.0".to_string(),
                capabilities: vec![DeviceCapability::SystemInformation],
            },
            device_public_key: format!("public-key-{device_id}"),
        })
        .await
        .expect("create pairing session");

    let device = store
        .approve(&session.pairing_code, &tenant_id, "admin-1")
        .await
        .expect("approve pairing");
    assert_eq!(device.tenant_id, tenant_id);
    assert!(store
        .approve(&session.pairing_code, "wrong-tenant", "admin-2")
        .await
        .is_err());

    let claimed = store
        .claim(&session.session_id, &session.claim_secret)
        .await
        .expect("claim credential");
    assert_eq!(claimed.device_id, device_id);
    assert!(claimed.credential.starts_with("desktop_"));
    assert!(store
        .claim(&session.session_id, &session.claim_secret)
        .await
        .is_err());

    let row: (String, String, Option<String>) = sqlx::query_as(
        r#"SELECT d.tenant_id, d.credential_hash, s.pending_credential_ciphertext
           FROM af_desktop_devices d
           JOIN af_desktop_pairing_sessions s ON s.credential_id = d.credential_id
           WHERE d.id = $1"#,
    )
    .bind(&device_id)
    .fetch_one(&pool)
    .await
    .expect("load paired device");
    assert_eq!(row.0, tenant_id);
    assert_ne!(row.1, claimed.credential);
    assert!(
        row.2.is_none(),
        "claimed plaintext must not remain recoverable"
    );

    assert!(store
        .get_device("wrong-tenant", &device_id)
        .await
        .expect("tenant-scoped lookup")
        .is_none());
    let renamed = store
        .rename_device(&tenant_id, &device_id, "Renamed Postgres Device")
        .await
        .expect("rename device");
    assert_eq!(renamed.display_name, "Renamed Postgres Device");
    let list = store
        .list_devices(&tenant_id, Some("paired"), 1, 0)
        .await
        .expect("list devices");
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].id, device_id);

    sqlx::query(
        "UPDATE af_desktop_devices SET state = 'online', last_seen_at = now() - interval '5 minutes' WHERE id = $1",
    )
    .bind(&device_id)
    .execute(&pool)
    .await
    .expect("make device stale");
    assert!(
        store
            .get_device(&tenant_id, &device_id)
            .await
            .expect("get stale device")
            .expect("device exists")
            .stale
    );

    store
        .start_rotation(&tenant_id, &device_id)
        .await
        .expect("start credential rotation");
    let store = std::sync::Arc::new(store);
    let first_store = std::sync::Arc::clone(&store);
    let second_store = std::sync::Arc::clone(&store);
    let first_secret = claimed.credential.clone();
    let second_secret = claimed.credential.clone();
    let first_device = device_id.clone();
    let second_device = device_id.clone();
    let (first, second) = tokio::join!(
        async move {
            first_store
                .claim_rotation(&first_device, &first_secret)
                .await
        },
        async move {
            second_store
                .claim_rotation(&second_device, &second_secret)
                .await
        }
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let rotated = first.or(second).expect("one rotation claim succeeds");
    assert!(store
        .authenticate_device(&device_id, &claimed.credential)
        .await
        .is_err());
    assert!(store
        .authenticate_device(&device_id, &rotated.credential)
        .await
        .is_ok());

    store
        .connect_device(&device_id, &rotated.credential, "connection-1")
        .await
        .expect("connect device");
    store
        .connect_device(&device_id, &rotated.credential, "connection-2")
        .await
        .expect("newest connection wins");
    let heartbeat = Heartbeat {
        device_id: device_id.clone(),
        state: DeviceState::Degraded,
        active_execution_id: None,
        agent_version: "1.2.0".to_string(),
        capabilities: vec![DeviceCapability::SystemInformation],
        health_detail: Some("automation adapter unavailable".to_string()),
    };
    assert!(store
        .record_heartbeat(&device_id, &rotated.credential, "connection-1", &heartbeat,)
        .await
        .is_err());
    let heartbeat_device = store
        .record_heartbeat(&device_id, &rotated.credential, "connection-2", &heartbeat)
        .await
        .expect("record heartbeat from owner");
    assert_eq!(heartbeat_device.state, "degraded");
    assert_eq!(heartbeat_device.agent_version, "1.2.0");
    assert_eq!(
        heartbeat_device.health_detail.as_deref(),
        Some("automation adapter unavailable")
    );
    sqlx::query(
        "UPDATE af_desktop_devices SET last_seen_at = now() - interval '5 minutes' WHERE id = $1",
    )
    .bind(&device_id)
    .execute(&pool)
    .await
    .expect("age heartbeat");
    let database_now: i64 = sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM now())::BIGINT")
        .fetch_one(&pool)
        .await
        .expect("load database time");
    assert_eq!(
        store
            .expire_stale_devices(database_now)
            .await
            .expect("expire stale device"),
        vec![device_id.clone()]
    );
    assert_eq!(
        store
            .get_device(&tenant_id, &device_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        "offline"
    );

    let execution_id = uniq("desktop-execution");
    sqlx::query("INSERT INTO af_executions (id, tenant_id, workflow_id, workflow_version_id, status, started_at) VALUES ($1,$2,'workflow-1','version-1','running',1)")
        .bind(&execution_id)
        .bind(&tenant_id)
        .execute(&pool)
        .await
        .expect("insert command execution");
    let command_store = PlatformDesktopCommandStore::postgres(pool.clone());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let approval_command = DesktopCommand {
        command_id: uniq("desktop-command-approval"),
        execution_id: execution_id.clone(),
        tenant_id: tenant_id.clone(),
        project_id: "project-1".to_string(),
        requested_by: "requester-1".to_string(),
        issued_at_unix_ms: now,
        lease: ExecutionLease {
            lease_id: uniq("desktop-lease-approval"),
            expires_at_unix_ms: now + 60_000,
        },
        approval: None,
        action: serde_json::from_value(serde_json::json!({
            "kind": "focus_window",
            "selector": { "executable": "fixture.exe" }
        }))
        .unwrap(),
    };
    assert_eq!(
        command_store
            .create(
                approval_command.clone(),
                device_id.clone(),
                "workflow-1".to_string(),
            )
            .await
            .expect("persist approval command")
            .status,
        "waiting_approval"
    );
    assert!(command_store
        .pending_approvals("another-tenant", 10, 0)
        .await
        .is_empty());
    assert_eq!(
        command_store
            .pending_approvals(&tenant_id, 10, 0)
            .await
            .len(),
        1
    );
    let approved = command_store
        .approve(&tenant_id, &approval_command.command_id, "operator-1", now)
        .await
        .expect("approve command")
        .0;
    assert_eq!(approved.status, "queued");
    assert_eq!(approved.command.approval.unwrap().approved_by, "operator-1");
    let approval_actor: String = sqlx::query_scalar(
        "SELECT detail_json->>'actor_id' FROM af_audit_log WHERE tenant_id=$1 AND action='desktop.command.approved' AND resource_id=$2",
    )
    .bind(&tenant_id)
    .bind(&approval_command.command_id)
    .fetch_one(&pool)
    .await
    .expect("approval actor audit");
    assert_eq!(approval_actor, "operator-1");
    assert!(
        !command_store
            .approve(&tenant_id, &approval_command.command_id, "operator-1", now,)
            .await
            .expect("repeat approval idempotently")
            .1
    );

    let command = DesktopCommand {
        command_id: uniq("desktop-command"),
        execution_id: execution_id.clone(),
        tenant_id: tenant_id.clone(),
        project_id: "project-1".to_string(),
        requested_by: "admin-1".to_string(),
        issued_at_unix_ms: now,
        lease: ExecutionLease {
            lease_id: uniq("desktop-lease"),
            expires_at_unix_ms: now + 60_000,
        },
        approval: None,
        action: DesktopAction::ReadSystemInformation,
    };
    command_store
        .create(command.clone(), device_id.clone(), "workflow-1".to_string())
        .await
        .expect("persist command");
    command_store
        .mark_delivered(&command.command_id, now)
        .await
        .expect("deliver command");
    command_store
        .acknowledge(
            &device_id,
            &DesktopCommandAcknowledgement {
                command_id: command.command_id.clone(),
                execution_id: execution_id.clone(),
                lease_id: command.lease.lease_id.clone(),
                acknowledged_at_unix_ms: now,
            },
            now,
        )
        .await
        .expect("acknowledge command");
    let result = DesktopCommandResult {
        command_id: command.command_id.clone(),
        execution_id: execution_id.clone(),
        outcome: CommandOutcome::Succeeded,
        completed_at_unix_ms: now,
        output: Some(serde_json::json!({"hostname": "desktop"})),
        error_code: None,
        error_message: None,
    };
    assert_eq!(
        command_store
            .complete(&device_id, result.clone())
            .await
            .expect("complete command")
            .status,
        "succeeded"
    );
    assert!(command_store.complete(&device_id, result).await.is_ok());
    let audit_actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM af_audit_log WHERE tenant_id=$1 AND resource_id=$2 ORDER BY created_at",
    )
    .bind(&tenant_id)
    .bind(&command.command_id)
    .fetch_all(&pool)
    .await
    .expect("load command audit");
    assert_eq!(
        audit_actions,
        vec![
            "desktop.command.queued",
            "desktop.command.acknowledged",
            "desktop.command.succeeded"
        ]
    );
    let evidence_store = PlatformDesktopEvidenceStore::postgres(pool.clone());
    let evidence = prepare_evidence(
        &tenant_id,
        &device_id,
        EvidenceUploadRequest {
            command_id: command.command_id.clone(),
            execution_id: execution_id.clone(),
            project_id: "project-1".to_owned(),
            kind: EvidenceKind::AdapterAudit,
            selector_strategy: SelectorStrategy::NotApplicable,
            selector_fallback_depth: 0,
            selector_fallback_used: false,
            application_id: "system_information".to_owned(),
            started_at_unix_ms: now,
            completed_at_unix_ms: now,
            outcome: CommandOutcome::Succeeded,
            retention_days: 7,
            capture_opt_in: false,
            redaction: RedactionAttestation {
                policy_version: "redaction-v1".to_owned(),
                succeeded: true,
                sensitive_regions: 0,
                redacted_regions: 0,
            },
            content_type: None,
            payload_base64: None,
        },
        &EvidencePolicy::default(),
        now,
    )
    .expect("prepare desktop evidence");
    let evidence_id = evidence.record.evidence_id.clone();
    evidence_store
        .create(evidence)
        .await
        .expect("persist desktop evidence");
    assert_eq!(
        evidence_store
            .list(&tenant_id, &execution_id)
            .await
            .expect("list desktop evidence")
            .len(),
        1
    );
    assert!(evidence_store
        .list("wrong-tenant", &execution_id)
        .await
        .expect("isolate desktop evidence")
        .is_empty());
    sqlx::query("UPDATE af_desktop_evidence SET expires_at = now() WHERE id = $1")
        .bind(&evidence_id)
        .execute(&pool)
        .await
        .expect("expire desktop evidence");
    assert_eq!(
        trigix_platform::retention::run_evidence_retention_pass(&pool)
            .await
            .expect("purge desktop evidence"),
        1
    );
    store
        .connect_device(&device_id, &rotated.credential, "connection-before-suspend")
        .await
        .expect("reconnect before suspension");

    store
        .set_device_state(&tenant_id, &device_id, "suspended")
        .await
        .expect("suspend device");
    assert!(!store
        .connection_is_current(&device_id, "connection-before-suspend")
        .await
        .expect("suspended connection lookup"));
    assert!(store
        .authenticate_device(&device_id, &rotated.credential)
        .await
        .is_err());
    store
        .set_device_state(&tenant_id, &device_id, "revoked")
        .await
        .expect("revoke device");
    assert!(store.start_rotation(&tenant_id, &device_id).await.is_err());
}

/// First-touch attribution persists and is not overwritten by a later signup.
#[tokio::test(flavor = "multi_thread")]
async fn attribution_first_touch_roundtrip() {
    let Some(pool) = setup().await else { return };
    let store = PlatformAttributionStore::postgres(PostgresAttributionStore::new(pool.clone()));
    let billing = PlatformBillingStore::postgres(pool.clone());
    let tenant = uniq("tenant");
    // Unique channel name → channel_revenue assertions stay deterministic even
    // though the aggregate is global across tenants/parallel test runs.
    let channel = uniq("ch");

    store
        .record_first_touch(AttributionRecord {
            tenant_id: tenant.clone(),
            utm_source: Some(channel.clone()),
            utm_campaign: Some("launch".into()),
            created_at: 1_700_000_000,
            ..Default::default()
        })
        .await;

    // Second touch must NOT overwrite.
    store
        .record_first_touch(AttributionRecord {
            tenant_id: tenant.clone(),
            utm_source: Some("twitter".into()),
            created_at: 1_700_001_000,
            ..Default::default()
        })
        .await;

    let got = store.get(&tenant).await.expect("attribution row present");
    assert_eq!(got.utm_source.as_deref(), Some(channel.as_str()));
    assert_eq!(got.utm_campaign.as_deref(), Some("launch"));
    assert_eq!(store.get(&uniq("absent")).await.map(|_| ()), None);

    // Convert the tenant to paid. set_quota and add_revenue are both
    // fire-and-forget, and add_revenue is an UPDATE — so wait for the quota row
    // to exist before adding revenue (in production checkout creates the row
    // well before invoice.paid arrives).
    billing.set_quota(TenantQuota::pro(&tenant));
    for _ in 0..30 {
        if billing.get_quota(&tenant).tier == "pro" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    billing.add_revenue(&tenant, "eur", 4900);

    // channel_revenue joins attribution × per-currency revenue: our channel
    // should show one paid signup with the EUR revenue (not mixed into USD).
    let mut stats = None;
    for _ in 0..30 {
        if let Some(s) = store
            .channel_revenue()
            .await
            .into_iter()
            .find(|c| c.channel == channel)
        {
            if s.paid >= 1
                && s.revenue
                    .iter()
                    .any(|r| r.currency == "eur" && r.cents >= 4900)
            {
                stats = Some(s);
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let s = stats.expect("channel_revenue should show the converted channel");
    assert_eq!(s.signups, 1);
    assert_eq!(s.paid, 1);
    assert_eq!(
        s.revenue,
        vec![CurrencyRevenue {
            currency: "eur".into(),
            cents: 4900
        }]
    );
    // Revenue read-back is per-currency (settled by the poll above).
    assert_eq!(
        billing.revenue_by_currency(&tenant),
        vec![("eur".to_string(), 4900)]
    );
}

/// Referral codes, first-touch links and the signed commission ledger round-trip.
#[tokio::test(flavor = "multi_thread")]
async fn affiliate_referral_and_ledger_roundtrip() {
    let Some(pool) = setup().await else { return };
    let store = PlatformAffiliateStore::postgres(PostgresAffiliateStore::new(pool));
    let referrer = uniq("affref");
    let referee = uniq("affee");

    // A code is created, resolves back, and the referral is first-touch.
    let code = store.get_or_create_code(&referrer).await;
    assert_eq!(
        store.resolve_code(&code).await.as_deref(),
        Some(referrer.as_str())
    );
    store.record_referral(&referee, &referrer, &code).await;
    store.record_referral(&referee, &uniq("other"), &code).await; // ignored
    assert_eq!(
        store.get_referrer(&referee).await.as_deref(),
        Some(referrer.as_str())
    );
    assert_eq!(store.referral_count(&referrer).await, 1);

    // Double-entry, per currency: USD commission − clawback − payout → owed 500.
    store
        .accrue_commission(&referrer, &referee, "usd", 1000, Some("evt1"))
        .await;
    store
        .clawback_commission(&referrer, &referee, "usd", 300, Some("evt2"))
        .await;
    store.record_payout(&referrer, "usd", 200, None).await;
    // An EUR commission is tracked separately.
    store
        .accrue_commission(&referrer, &referee, "eur", 700, Some("evt3"))
        .await;
    assert_eq!(store.balance_for(&referrer, "usd").await, 500);
    assert_eq!(store.balance_for(&referrer, "eur").await, 700);
    assert_eq!(store.list_entries(&referrer, 10).await.len(), 4);

    // Payout request (USD) → operator approval books a payout, reducing balance.
    let req = store
        .request_payout(&referrer, "usdt", "TUSDTaddr", "usd", 100)
        .await;
    assert!(store
        .list_pending_payouts()
        .await
        .iter()
        .any(|r| r.id == req.id));
    let done = store
        .process_payout_request(&req.id, true, Some("sent"))
        .await
        .expect("request exists");
    assert_eq!(done.status, "paid");
    assert_eq!(done.currency, "usd");
    assert_eq!(store.balance_for(&referrer, "usd").await, 400); // 500 − 100
    assert_eq!(store.balance_for(&referrer, "eur").await, 700); // unaffected
}

/// Token-usage records persist and aggregate per model in the summary.
#[tokio::test(flavor = "multi_thread")]
async fn token_usage_record_and_summarize() {
    let Some(pool) = setup().await else { return };
    let store = PlatformTokenUsageStore::postgres(PostgresTokenUsageStore::new(pool));
    let tenant = uniq("tenant");

    for (prompt, completion) in [(10, 5), (20, 7)] {
        store
            .record(TokenUsageRecord {
                id: uuid::Uuid::new_v4().to_string(),
                tenant_id: tenant.clone(),
                execution_id: uniq("exec"),
                node_id: "n1".into(),
                model: "gpt-test".into(),
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
                created_at: 1_700_000_000,
            })
            .await;
    }

    let summary = store.summarize(&tenant, 0).await;
    assert_eq!(summary.prompt_tokens, 30);
    assert_eq!(summary.completion_tokens, 12);
    assert_eq!(summary.total_tokens, 42);
    assert_eq!(
        summary.by_model.get("gpt-test").map(|m| m.total_tokens),
        Some(42)
    );
}

/// Quota, usage counters and Stripe-id mapping round-trip through Postgres.
/// These writers are fire-and-forget, so reads are polled.
#[tokio::test(flavor = "multi_thread")]
async fn billing_quota_usage_and_stripe_ids() {
    let Some(pool) = setup().await else { return };
    let store = PlatformBillingStore::postgres(pool);
    let tenant = uniq("tenant");

    // A fresh tenant defaults to the free tier.
    assert_eq!(store.get_quota(&tenant).tier, "free");

    // Upgrade persists.
    store.set_quota(TenantQuota::pro(&tenant));
    assert!(
        eventually(|| store.get_quota(&tenant).tier == "pro").await,
        "quota should upgrade to pro"
    );

    // Usage counters increment.
    store.increment_execution(&tenant);
    store.increment_tokens(&tenant, 123);
    assert!(
        eventually(|| {
            let u = store.billing_status(&tenant).usage;
            u.executions_used >= 1 && u.tokens_used >= 123
        })
        .await,
        "execution + token usage should be recorded"
    );

    // Stripe customer/subscription mapping round-trips both ways.
    let customer = uniq("cus");
    store.set_stripe_ids(&tenant, Some(&customer), Some("sub_int"));
    assert!(
        eventually(|| store.get_stripe_ids(&tenant).0.as_deref() == Some(customer.as_str())).await,
        "stripe customer id should persist"
    );
    assert_eq!(
        store.get_tenant_by_stripe_customer(&customer).as_deref(),
        Some(tenant.as_str())
    );

    // Webhook idempotency: an event id is claimed once, then deduped.
    let event_id = uniq("evt");
    assert!(
        store.mark_stripe_event_processed(&event_id),
        "first delivery of an event is processed"
    );
    assert!(
        !store.mark_stripe_event_processed(&event_id),
        "a retried/replayed event is skipped"
    );
}
