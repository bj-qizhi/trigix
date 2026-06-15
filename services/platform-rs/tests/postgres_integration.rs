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

use trigix_platform::attribution::{
    AttributionRecord, AttributionStore, PlatformAttributionStore, PostgresAttributionStore,
};
use trigix_platform::billing::{BillingStore, PlatformBillingStore, TenantQuota};
use trigix_platform::token_usage::{
    PlatformTokenUsageStore, PostgresTokenUsageStore, TokenUsageRecord, TokenUsageStore,
};
use trigix_platform::users::{PlatformUserStore, UserStore};

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

/// First-touch attribution persists and is not overwritten by a later signup.
#[tokio::test(flavor = "multi_thread")]
async fn attribution_first_touch_roundtrip() {
    let Some(pool) = setup().await else { return };
    let store = PlatformAttributionStore::postgres(PostgresAttributionStore::new(pool));
    let tenant = uniq("tenant");

    let first = AttributionRecord {
        tenant_id: tenant.clone(),
        utm_source: Some("google".into()),
        utm_campaign: Some("launch".into()),
        referrer: Some("https://news.ycombinator.com".into()),
        created_at: 1_700_000_000,
        ..Default::default()
    };
    store.record_first_touch(first).await;

    // Second touch must NOT overwrite.
    let second = AttributionRecord {
        tenant_id: tenant.clone(),
        utm_source: Some("twitter".into()),
        created_at: 1_700_001_000,
        ..Default::default()
    };
    store.record_first_touch(second).await;

    let got = store.get(&tenant).await.expect("attribution row present");
    assert_eq!(got.utm_source.as_deref(), Some("google"));
    assert_eq!(got.utm_campaign.as_deref(), Some("launch"));
    assert_eq!(store.get(&uniq("absent")).await.map(|_| ()), None);

    // The channel breakdown (GROUP BY) buckets the row under its utm_source.
    // Global across tenants, so assert our channel is present with >= 1 signup.
    let breakdown = store.channel_breakdown().await;
    let google = breakdown.iter().find(|c| c.channel == "google");
    assert!(
        google.map(|c| c.signups >= 1).unwrap_or(false),
        "channel_breakdown should include google with >= 1 signup"
    );
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
