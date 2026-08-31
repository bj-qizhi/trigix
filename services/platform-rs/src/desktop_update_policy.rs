use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MINUTES_PER_DAY: u16 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopUpdateMode {
    Disabled,
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopReleaseChannel {
    Internal,
    ClosedBeta,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopMaintenanceWindow {
    pub start_minute_utc: u16,
    pub duration_minutes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopUpdatePolicy {
    pub tenant_id: String,
    pub revision: u64,
    pub mode: DesktopUpdateMode,
    pub channel: DesktopReleaseChannel,
    pub required_version: String,
    pub pinned_version: Option<String>,
    pub maintenance_window: Option<DesktopMaintenanceWindow>,
    pub allow_offline_import: bool,
    pub allow_emergency_rollback: bool,
    pub updated_by: String,
    pub updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateDesktopPolicyRequest {
    pub observed_revision: u64,
    pub mode: DesktopUpdateMode,
    pub channel: DesktopReleaseChannel,
    pub required_version: String,
    pub pinned_version: Option<String>,
    pub maintenance_window: Option<DesktopMaintenanceWindow>,
    pub allow_offline_import: bool,
    pub allow_emergency_rollback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopUpdatePolicyError {
    InvalidRequest,
    Conflict,
    StoreUnavailable,
}

impl DesktopUpdatePolicy {
    pub fn safe_default(tenant_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_owned(),
            revision: 0,
            mode: DesktopUpdateMode::Disabled,
            channel: DesktopReleaseChannel::Stable,
            required_version: env!("CARGO_PKG_VERSION").to_owned(),
            pinned_version: None,
            maintenance_window: None,
            allow_offline_import: false,
            allow_emergency_rollback: false,
            updated_by: "system-default".to_owned(),
            updated_at_unix_seconds: 0,
        }
    }
}

impl UpdateDesktopPolicyRequest {
    fn validate(&self) -> Result<(), DesktopUpdatePolicyError> {
        if semver::Version::parse(&self.required_version).is_err()
            || self
                .pinned_version
                .as_ref()
                .is_some_and(|value| semver::Version::parse(value).is_err())
            || self.maintenance_window.as_ref().is_some_and(|window| {
                window.start_minute_utc >= MINUTES_PER_DAY
                    || window.duration_minutes == 0
                    || window.duration_minutes > MINUTES_PER_DAY
            })
            || (self.mode == DesktopUpdateMode::Automatic && self.maintenance_window.is_none())
            || self
                .pinned_version
                .as_ref()
                .is_some_and(|value| value != &self.required_version)
        {
            return Err(DesktopUpdatePolicyError::InvalidRequest);
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct MemoryDesktopUpdatePolicyStore {
    policies: Arc<Mutex<HashMap<String, DesktopUpdatePolicy>>>,
}

impl MemoryDesktopUpdatePolicyStore {
    fn get(&self, tenant_id: &str) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
        validate_identifier(tenant_id)?;
        Ok(self
            .policies
            .lock()
            .map_err(|_| DesktopUpdatePolicyError::StoreUnavailable)?
            .get(tenant_id)
            .cloned()
            .unwrap_or_else(|| DesktopUpdatePolicy::safe_default(tenant_id)))
    }

    fn update(
        &self,
        tenant_id: &str,
        actor_id: &str,
        request: UpdateDesktopPolicyRequest,
    ) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
        validate_identifier(tenant_id)?;
        validate_identifier(actor_id)?;
        request.validate()?;
        let mut policies = self
            .policies
            .lock()
            .map_err(|_| DesktopUpdatePolicyError::StoreUnavailable)?;
        let current_revision = policies.get(tenant_id).map_or(0, |policy| policy.revision);
        if request.observed_revision != current_revision {
            return Err(DesktopUpdatePolicyError::Conflict);
        }
        let policy = policy_from_request(tenant_id, actor_id, current_revision + 1, request);
        policies.insert(tenant_id.to_owned(), policy.clone());
        Ok(policy)
    }
}

#[derive(Clone)]
pub struct PostgresDesktopUpdatePolicyStore {
    pool: PgPool,
}

impl PostgresDesktopUpdatePolicyStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn get(&self, tenant_id: &str) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
        validate_identifier(tenant_id)?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        let row = sqlx::query(
            "SELECT tenant_id, revision, mode, channel, required_version, pinned_version, maintenance_start_minute_utc, maintenance_duration_minutes, allow_offline_import, allow_emergency_rollback, updated_by, EXTRACT(EPOCH FROM updated_at)::bigint AS updated_at_unix_seconds FROM af_desktop_update_policies WHERE tenant_id=$1",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        row.as_ref()
            .map(row_to_policy)
            .transpose()?
            .map_or_else(|| Ok(DesktopUpdatePolicy::safe_default(tenant_id)), Ok)
    }

    async fn update(
        &self,
        tenant_id: &str,
        actor_id: &str,
        request: UpdateDesktopPolicyRequest,
    ) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
        validate_identifier(tenant_id)?;
        validate_identifier(actor_id)?;
        request.validate()?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        let next_revision = request
            .observed_revision
            .checked_add(1)
            .ok_or(DesktopUpdatePolicyError::InvalidRequest)?;
        let start = request
            .maintenance_window
            .as_ref()
            .map(|window| i16::try_from(window.start_minute_utc))
            .transpose()
            .map_err(|_| DesktopUpdatePolicyError::InvalidRequest)?;
        let duration = request
            .maintenance_window
            .as_ref()
            .map(|window| i16::try_from(window.duration_minutes))
            .transpose()
            .map_err(|_| DesktopUpdatePolicyError::InvalidRequest)?;
        let row = sqlx::query(
            r#"INSERT INTO af_desktop_update_policies
               (tenant_id, revision, mode, channel, required_version, pinned_version,
                maintenance_start_minute_utc, maintenance_duration_minutes,
                allow_offline_import, allow_emergency_rollback, updated_by, updated_at)
               SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,now()
               WHERE $12 = 0
               ON CONFLICT (tenant_id) DO UPDATE SET
                 revision=EXCLUDED.revision, mode=EXCLUDED.mode, channel=EXCLUDED.channel,
                 required_version=EXCLUDED.required_version, pinned_version=EXCLUDED.pinned_version,
                 maintenance_start_minute_utc=EXCLUDED.maintenance_start_minute_utc,
                 maintenance_duration_minutes=EXCLUDED.maintenance_duration_minutes,
                 allow_offline_import=EXCLUDED.allow_offline_import,
                 allow_emergency_rollback=EXCLUDED.allow_emergency_rollback,
                 updated_by=EXCLUDED.updated_by, updated_at=now()
               WHERE af_desktop_update_policies.revision = $12
               RETURNING tenant_id, revision, mode, channel, required_version, pinned_version,
                 maintenance_start_minute_utc, maintenance_duration_minutes,
                 allow_offline_import, allow_emergency_rollback, updated_by,
                 EXTRACT(EPOCH FROM updated_at)::bigint AS updated_at_unix_seconds"#,
        )
        .bind(tenant_id)
        .bind(i64::try_from(next_revision).map_err(|_| DesktopUpdatePolicyError::InvalidRequest)?)
        .bind(mode_name(request.mode))
        .bind(channel_name(request.channel))
        .bind(&request.required_version)
        .bind(&request.pinned_version)
        .bind(start)
        .bind(duration)
        .bind(request.allow_offline_import)
        .bind(request.allow_emergency_rollback)
        .bind(actor_id)
        .bind(
            i64::try_from(request.observed_revision)
                .map_err(|_| DesktopUpdatePolicyError::InvalidRequest)?,
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Err(DesktopUpdatePolicyError::Conflict);
        };
        let policy = row_to_policy(&row)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(policy)
    }
}

#[derive(Clone)]
pub enum PlatformDesktopUpdatePolicyStore {
    Memory(MemoryDesktopUpdatePolicyStore),
    Postgres(PostgresDesktopUpdatePolicyStore),
}

impl Default for PlatformDesktopUpdatePolicyStore {
    fn default() -> Self {
        Self::Memory(MemoryDesktopUpdatePolicyStore::default())
    }
}

impl PlatformDesktopUpdatePolicyStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresDesktopUpdatePolicyStore::new(pool))
    }

    pub async fn get(
        &self,
        tenant_id: &str,
    ) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
        match self {
            Self::Memory(store) => store.get(tenant_id),
            Self::Postgres(store) => store.get(tenant_id).await,
        }
    }

    pub async fn update(
        &self,
        tenant_id: &str,
        actor_id: &str,
        request: UpdateDesktopPolicyRequest,
    ) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
        match self {
            Self::Memory(store) => store.update(tenant_id, actor_id, request),
            Self::Postgres(store) => store.update(tenant_id, actor_id, request).await,
        }
    }
}

fn policy_from_request(
    tenant_id: &str,
    actor_id: &str,
    revision: u64,
    request: UpdateDesktopPolicyRequest,
) -> DesktopUpdatePolicy {
    DesktopUpdatePolicy {
        tenant_id: tenant_id.to_owned(),
        revision,
        mode: request.mode,
        channel: request.channel,
        required_version: request.required_version,
        pinned_version: request.pinned_version,
        maintenance_window: request.maintenance_window,
        allow_offline_import: request.allow_offline_import,
        allow_emergency_rollback: request.allow_emergency_rollback,
        updated_by: actor_id.to_owned(),
        updated_at_unix_seconds: unix_now(),
    }
}

fn row_to_policy(
    row: &sqlx::postgres::PgRow,
) -> Result<DesktopUpdatePolicy, DesktopUpdatePolicyError> {
    let start: Option<i16> = row
        .try_get("maintenance_start_minute_utc")
        .map_err(store_error)?;
    let duration: Option<i16> = row
        .try_get("maintenance_duration_minutes")
        .map_err(store_error)?;
    let maintenance_window = match (start, duration) {
        (None, None) => None,
        (Some(start), Some(duration)) => Some(DesktopMaintenanceWindow {
            start_minute_utc: u16::try_from(start)
                .map_err(|_| DesktopUpdatePolicyError::StoreUnavailable)?,
            duration_minutes: u16::try_from(duration)
                .map_err(|_| DesktopUpdatePolicyError::StoreUnavailable)?,
        }),
        _ => return Err(DesktopUpdatePolicyError::StoreUnavailable),
    };
    Ok(DesktopUpdatePolicy {
        tenant_id: row.try_get("tenant_id").map_err(store_error)?,
        revision: u64::try_from(row.try_get::<i64, _>("revision").map_err(store_error)?)
            .map_err(|_| DesktopUpdatePolicyError::StoreUnavailable)?,
        mode: parse_mode(&row.try_get::<String, _>("mode").map_err(store_error)?)?,
        channel: parse_channel(&row.try_get::<String, _>("channel").map_err(store_error)?)?,
        required_version: row.try_get("required_version").map_err(store_error)?,
        pinned_version: row.try_get("pinned_version").map_err(store_error)?,
        maintenance_window,
        allow_offline_import: row.try_get("allow_offline_import").map_err(store_error)?,
        allow_emergency_rollback: row
            .try_get("allow_emergency_rollback")
            .map_err(store_error)?,
        updated_by: row.try_get("updated_by").map_err(store_error)?,
        updated_at_unix_seconds: u64::try_from(
            row.try_get::<i64, _>("updated_at_unix_seconds")
                .map_err(store_error)?,
        )
        .map_err(|_| DesktopUpdatePolicyError::StoreUnavailable)?,
    })
}

fn mode_name(mode: DesktopUpdateMode) -> &'static str {
    match mode {
        DesktopUpdateMode::Disabled => "disabled",
        DesktopUpdateMode::Manual => "manual",
        DesktopUpdateMode::Automatic => "automatic",
    }
}

fn channel_name(channel: DesktopReleaseChannel) -> &'static str {
    match channel {
        DesktopReleaseChannel::Internal => "internal",
        DesktopReleaseChannel::ClosedBeta => "closed_beta",
        DesktopReleaseChannel::Stable => "stable",
    }
}

fn parse_mode(value: &str) -> Result<DesktopUpdateMode, DesktopUpdatePolicyError> {
    match value {
        "disabled" => Ok(DesktopUpdateMode::Disabled),
        "manual" => Ok(DesktopUpdateMode::Manual),
        "automatic" => Ok(DesktopUpdateMode::Automatic),
        _ => Err(DesktopUpdatePolicyError::StoreUnavailable),
    }
}

fn parse_channel(value: &str) -> Result<DesktopReleaseChannel, DesktopUpdatePolicyError> {
    match value {
        "internal" => Ok(DesktopReleaseChannel::Internal),
        "closed_beta" => Ok(DesktopReleaseChannel::ClosedBeta),
        "stable" => Ok(DesktopReleaseChannel::Stable),
        _ => Err(DesktopUpdatePolicyError::StoreUnavailable),
    }
}

fn validate_identifier(value: &str) -> Result<(), DesktopUpdatePolicyError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'@'))
    {
        return Err(DesktopUpdatePolicyError::InvalidRequest);
    }
    Ok(())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn store_error(_: impl std::fmt::Display) -> DesktopUpdatePolicyError {
    DesktopUpdatePolicyError::StoreUnavailable
}

async fn set_tenant_context(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
) -> Result<(), DesktopUpdatePolicyError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .map_err(store_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(revision: u64) -> UpdateDesktopPolicyRequest {
        UpdateDesktopPolicyRequest {
            observed_revision: revision,
            mode: DesktopUpdateMode::Automatic,
            channel: DesktopReleaseChannel::Stable,
            required_version: "1.5.1".to_owned(),
            pinned_version: Some("1.5.1".to_owned()),
            maintenance_window: Some(DesktopMaintenanceWindow {
                start_minute_utc: 60,
                duration_minutes: 30,
            }),
            allow_offline_import: false,
            allow_emergency_rollback: true,
        }
    }

    #[tokio::test]
    async fn defaults_are_safe_and_updates_are_tenant_scoped() {
        let store = PlatformDesktopUpdatePolicyStore::default();
        assert_eq!(
            store.get("tenant-a").await.unwrap().mode,
            DesktopUpdateMode::Disabled
        );
        let saved = store
            .update("tenant-a", "admin-1", request(0))
            .await
            .unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(store.get("tenant-b").await.unwrap().revision, 0);
    }

    #[tokio::test]
    async fn rejects_stale_and_invalid_updates() {
        let store = PlatformDesktopUpdatePolicyStore::default();
        store
            .update("tenant-a", "admin-1", request(0))
            .await
            .unwrap();
        assert_eq!(
            store
                .update("tenant-a", "admin-2", request(0))
                .await
                .unwrap_err(),
            DesktopUpdatePolicyError::Conflict
        );
        let mut invalid = request(1);
        invalid.required_version = "latest".to_owned();
        assert_eq!(
            store
                .update("tenant-a", "admin-1", invalid)
                .await
                .unwrap_err(),
            DesktopUpdatePolicyError::InvalidRequest
        );
    }
}
