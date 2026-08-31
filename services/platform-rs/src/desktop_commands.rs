// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use desktop_protocol::{
    CommandOutcome, DesktopCommand, DesktopCommandAcknowledgement, DesktopCommandApproval,
    DesktopCommandResult, RiskLevel,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopCommandRecord {
    pub command: DesktopCommand,
    pub device_id: String,
    pub workflow_id: String,
    pub status: String,
    pub result: Option<DesktopCommandResult>,
    pub created_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopCommandError {
    NotFound,
    Conflict,
    Expired,
    Store(String),
}

impl std::fmt::Display for DesktopCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("desktop command not found"),
            Self::Conflict => f.write_str("desktop command state conflict"),
            Self::Expired => f.write_str("desktop command lease expired"),
            Self::Store(message) => write!(f, "desktop command store error: {message}"),
        }
    }
}

#[derive(Clone, Default)]
pub struct MemoryDesktopCommandStore {
    commands: Arc<Mutex<HashMap<String, DesktopCommandRecord>>>,
}

impl MemoryDesktopCommandStore {
    pub async fn create(
        &self,
        command: DesktopCommand,
        device_id: String,
        workflow_id: String,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        command
            .validate(command.issued_at_unix_ms)
            .map_err(|error| DesktopCommandError::Store(error.to_string()))?;
        let mut commands = self.commands.lock().expect("desktop commands lock");
        if commands.contains_key(&command.command_id) {
            return Err(DesktopCommandError::Conflict);
        }
        let status = initial_status(&command).to_string();
        let record = DesktopCommandRecord {
            created_at_unix_ms: command.issued_at_unix_ms,
            command,
            device_id,
            workflow_id,
            status,
            result: None,
        };
        commands.insert(record.command.command_id.clone(), record.clone());
        Ok(record)
    }

    pub async fn get(
        &self,
        tenant_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        self.commands
            .lock()
            .expect("desktop commands lock")
            .get(command_id)
            .filter(|record| record.command.tenant_id == tenant_id)
            .cloned()
            .ok_or(DesktopCommandError::NotFound)
    }

    pub async fn pending_for_device(&self, device_id: &str) -> Vec<DesktopCommandRecord> {
        let mut records = self
            .commands
            .lock()
            .expect("desktop commands lock")
            .values()
            .filter(|record| {
                record.device_id == device_id
                    && matches!(record.status.as_str(), "queued" | "delivered")
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.created_at_unix_ms);
        records
    }

    pub async fn pending_approvals(
        &self,
        tenant_id: &str,
        limit: usize,
        offset: usize,
    ) -> Vec<DesktopCommandRecord> {
        let mut records = self
            .commands
            .lock()
            .expect("desktop commands lock")
            .values()
            .filter(|record| {
                record.command.tenant_id == tenant_id && record.status == "waiting_approval"
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.created_at_unix_ms);
        records.into_iter().skip(offset).take(limit).collect()
    }

    pub async fn approve(
        &self,
        tenant_id: &str,
        command_id: &str,
        approved_by: &str,
        now_unix_ms: u64,
    ) -> Result<(DesktopCommandRecord, bool), DesktopCommandError> {
        let mut commands = self.commands.lock().expect("desktop commands lock");
        let record = commands
            .get_mut(command_id)
            .filter(|record| record.command.tenant_id == tenant_id)
            .ok_or(DesktopCommandError::NotFound)?;
        if record.status == "queued"
            && record
                .command
                .approval
                .as_ref()
                .is_some_and(|approval| approval.approved_by == approved_by)
        {
            return Ok((record.clone(), false));
        }
        if record.status != "waiting_approval" {
            return Err(DesktopCommandError::Conflict);
        }
        if record.command.lease.expires_at_unix_ms <= now_unix_ms {
            record.status = "timed_out".to_string();
            return Err(DesktopCommandError::Expired);
        }
        record.command.approval = Some(DesktopCommandApproval {
            approved_by: approved_by.to_owned(),
            expires_at_unix_ms: record.command.lease.expires_at_unix_ms,
        });
        record
            .command
            .validate(now_unix_ms)
            .map_err(|error| DesktopCommandError::Store(error.to_string()))?;
        record.status = "queued".to_string();
        Ok((record.clone(), true))
    }

    pub async fn reject(
        &self,
        tenant_id: &str,
        command_id: &str,
        _rejected_by: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let mut commands = self.commands.lock().expect("desktop commands lock");
        let record = commands
            .get_mut(command_id)
            .filter(|record| record.command.tenant_id == tenant_id)
            .ok_or(DesktopCommandError::NotFound)?;
        if record.status == "rejected" {
            return Ok(record.clone());
        }
        if record.status != "waiting_approval" {
            return Err(DesktopCommandError::Conflict);
        }
        record.status = "rejected".to_string();
        Ok(record.clone())
    }

    pub async fn mark_delivered(
        &self,
        command_id: &str,
        now_unix_ms: u64,
    ) -> Result<(), DesktopCommandError> {
        let mut commands = self.commands.lock().expect("desktop commands lock");
        let record = commands
            .get_mut(command_id)
            .ok_or(DesktopCommandError::NotFound)?;
        if record.command.lease.expires_at_unix_ms <= now_unix_ms {
            record.status = "timed_out".to_string();
            return Err(DesktopCommandError::Expired);
        }
        if matches!(record.status.as_str(), "queued" | "delivered") {
            record.status = "delivered".to_string();
            Ok(())
        } else {
            Err(DesktopCommandError::Conflict)
        }
    }

    pub async fn acknowledge(
        &self,
        device_id: &str,
        acknowledgement: &DesktopCommandAcknowledgement,
        now_unix_ms: u64,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let mut commands = self.commands.lock().expect("desktop commands lock");
        let record = commands
            .get_mut(&acknowledgement.command_id)
            .filter(|record| record.device_id == device_id)
            .ok_or(DesktopCommandError::NotFound)?;
        if record.command.execution_id != acknowledgement.execution_id
            || record.command.lease.lease_id != acknowledgement.lease_id
        {
            return Err(DesktopCommandError::Conflict);
        }
        if record.command.lease.expires_at_unix_ms <= now_unix_ms {
            record.status = "timed_out".to_string();
            return Err(DesktopCommandError::Expired);
        }
        if matches!(record.status.as_str(), "delivered" | "acknowledged") {
            record.status = "acknowledged".to_string();
            Ok(record.clone())
        } else {
            Err(DesktopCommandError::Conflict)
        }
    }

    pub async fn complete(
        &self,
        device_id: &str,
        result: DesktopCommandResult,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        result
            .validate()
            .map_err(|error| DesktopCommandError::Store(error.to_string()))?;
        let mut commands = self.commands.lock().expect("desktop commands lock");
        let record = commands
            .get_mut(&result.command_id)
            .filter(|record| record.device_id == device_id)
            .ok_or(DesktopCommandError::NotFound)?;
        if record.command.execution_id != result.execution_id {
            return Err(DesktopCommandError::Conflict);
        }
        if let Some(existing) = &record.result {
            return if existing == &result {
                Ok(record.clone())
            } else {
                Err(DesktopCommandError::Conflict)
            };
        }
        if record.status != "acknowledged" {
            return Err(DesktopCommandError::Conflict);
        }
        record.status = outcome_status(result.outcome).to_string();
        record.result = Some(result);
        Ok(record.clone())
    }

    pub async fn cancel(
        &self,
        tenant_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let mut commands = self.commands.lock().expect("desktop commands lock");
        let record = commands
            .get_mut(command_id)
            .filter(|record| record.command.tenant_id == tenant_id)
            .ok_or(DesktopCommandError::NotFound)?;
        if matches!(
            record.status.as_str(),
            "waiting_approval" | "queued" | "delivered" | "acknowledged"
        ) {
            record.status = "cancelled".to_string();
            Ok(record.clone())
        } else {
            Err(DesktopCommandError::Conflict)
        }
    }

    pub async fn expire(&self, now_unix_ms: u64) -> Vec<DesktopCommandRecord> {
        let mut commands = self.commands.lock().expect("desktop commands lock");
        commands
            .values_mut()
            .filter(|record| {
                matches!(
                    record.status.as_str(),
                    "waiting_approval" | "queued" | "delivered" | "acknowledged"
                ) && record.command.lease.expires_at_unix_ms <= now_unix_ms
            })
            .map(|record| {
                record.status = "timed_out".to_string();
                record.clone()
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct PostgresDesktopCommandStore {
    pool: PgPool,
}

impl PostgresDesktopCommandStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        command: DesktopCommand,
        device_id: String,
        workflow_id: String,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        command
            .validate(command.issued_at_unix_ms)
            .map_err(|error| DesktopCommandError::Store(error.to_string()))?;
        let command_json = serde_json::to_value(&command).map_err(store_error)?;
        let status = initial_status(&command);
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let result = sqlx::query(
            "INSERT INTO af_desktop_commands (id, tenant_id, project_id, workflow_id, execution_id, device_id, requested_by, lease_id, command_json, status, expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,to_timestamp($11::double precision / 1000.0))",
        )
        .bind(&command.command_id)
        .bind(&command.tenant_id)
        .bind(&command.project_id)
        .bind(&workflow_id)
        .bind(&command.execution_id)
        .bind(&device_id)
        .bind(&command.requested_by)
        .bind(&command.lease.lease_id)
        .bind(command_json)
        .bind(status)
        .bind(command.lease.expires_at_unix_ms as i64)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = result {
            return if error
                .as_database_error()
                .is_some_and(|error| error.is_unique_violation())
            {
                Err(DesktopCommandError::Conflict)
            } else {
                Err(store_error(error))
            };
        }
        insert_audit(
            &mut transaction,
            &command.tenant_id,
            &format!("desktop.command.{status}"),
            &command.command_id,
            &command.execution_id,
            &device_id,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(DesktopCommandRecord {
            command,
            device_id,
            workflow_id,
            status: status.to_string(),
            result: None,
            created_at_unix_ms: now_millis(),
        })
    }

    pub async fn get(
        &self,
        tenant_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let row = sqlx::query("SELECT command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms FROM af_desktop_commands WHERE tenant_id=$1 AND id=$2")
            .bind(tenant_id).bind(command_id).fetch_optional(&self.pool).await.map_err(store_error)?
            .ok_or(DesktopCommandError::NotFound)?;
        row_to_record(&row)
    }

    pub async fn pending_for_device(&self, device_id: &str) -> Vec<DesktopCommandRecord> {
        let rows = sqlx::query("SELECT command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms FROM af_desktop_commands WHERE device_id=$1 AND status IN ('queued','delivered') AND expires_at > now() ORDER BY created_at")
            .bind(device_id).fetch_all(&self.pool).await.unwrap_or_default();
        rows.iter()
            .filter_map(|row| row_to_record(row).ok())
            .collect()
    }

    pub async fn pending_approvals(
        &self,
        tenant_id: &str,
        limit: usize,
        offset: usize,
    ) -> Vec<DesktopCommandRecord> {
        let rows = sqlx::query("SELECT command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms FROM af_desktop_commands WHERE tenant_id=$1 AND status='waiting_approval' AND expires_at > now() ORDER BY created_at LIMIT $2 OFFSET $3")
            .bind(tenant_id)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.iter()
            .filter_map(|row| row_to_record(row).ok())
            .collect()
    }

    pub async fn approve(
        &self,
        tenant_id: &str,
        command_id: &str,
        approved_by: &str,
        now_unix_ms: u64,
    ) -> Result<(DesktopCommandRecord, bool), DesktopCommandError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query("SELECT command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms FROM af_desktop_commands WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
            .bind(tenant_id).bind(command_id).fetch_optional(&mut *transaction).await.map_err(store_error)?
            .ok_or(DesktopCommandError::NotFound)?;
        let mut record = row_to_record(&row)?;
        if record.status == "queued"
            && record
                .command
                .approval
                .as_ref()
                .is_some_and(|approval| approval.approved_by == approved_by)
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok((record, false));
        }
        if record.status != "waiting_approval" {
            return Err(DesktopCommandError::Conflict);
        }
        if record.command.lease.expires_at_unix_ms <= now_unix_ms {
            sqlx::query("UPDATE af_desktop_commands SET status='timed_out', completed_at=now(), updated_at=now() WHERE tenant_id=$1 AND id=$2")
                .bind(tenant_id).bind(command_id).execute(&mut *transaction).await.map_err(store_error)?;
            transaction.commit().await.map_err(store_error)?;
            return Err(DesktopCommandError::Expired);
        }
        record.command.approval = Some(DesktopCommandApproval {
            approved_by: approved_by.to_owned(),
            expires_at_unix_ms: record.command.lease.expires_at_unix_ms,
        });
        record.command.validate(now_unix_ms).map_err(store_error)?;
        let command_json = serde_json::to_value(&record.command).map_err(store_error)?;
        let row = sqlx::query("UPDATE af_desktop_commands SET command_json=$1, status='queued', updated_at=now() WHERE tenant_id=$2 AND id=$3 AND status='waiting_approval' RETURNING command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms")
            .bind(command_json).bind(tenant_id).bind(command_id).fetch_one(&mut *transaction).await.map_err(store_error)?;
        let record = row_to_record(&row)?;
        insert_decision_audit(
            &mut transaction,
            tenant_id,
            "desktop.command.approved",
            command_id,
            &record.command.execution_id,
            &record.device_id,
            approved_by,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok((record, true))
    }

    pub async fn reject(
        &self,
        tenant_id: &str,
        command_id: &str,
        rejected_by: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query("UPDATE af_desktop_commands SET status='rejected', completed_at=now(), updated_at=now() WHERE tenant_id=$1 AND id=$2 AND status='waiting_approval' RETURNING command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms")
            .bind(tenant_id).bind(command_id).fetch_optional(&mut *transaction).await.map_err(store_error)?
            ;
        let Some(row) = row else {
            drop(transaction);
            let existing = self.get(tenant_id, command_id).await?;
            return if existing.status == "rejected" {
                Ok(existing)
            } else {
                Err(DesktopCommandError::Conflict)
            };
        };
        let record = row_to_record(&row)?;
        insert_decision_audit(
            &mut transaction,
            tenant_id,
            "desktop.command.rejected",
            command_id,
            &record.command.execution_id,
            &record.device_id,
            rejected_by,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(record)
    }

    pub async fn mark_delivered(
        &self,
        command_id: &str,
        _now_unix_ms: u64,
    ) -> Result<(), DesktopCommandError> {
        let result = sqlx::query("UPDATE af_desktop_commands SET status='delivered', delivered_at=now(), updated_at=now() WHERE id=$1 AND status IN ('queued','delivered') AND expires_at > now()")
            .bind(command_id).execute(&self.pool).await.map_err(store_error)?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(DesktopCommandError::Conflict)
        }
    }

    pub async fn acknowledge(
        &self,
        device_id: &str,
        acknowledgement: &DesktopCommandAcknowledgement,
        _now_unix_ms: u64,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query("UPDATE af_desktop_commands SET status='acknowledged', acknowledged_at=now(), updated_at=now() WHERE id=$1 AND execution_id=$2 AND lease_id=$3 AND device_id=$4 AND status IN ('delivered','acknowledged') AND expires_at > now() RETURNING command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms, tenant_id")
            .bind(&acknowledgement.command_id).bind(&acknowledgement.execution_id).bind(&acknowledgement.lease_id).bind(device_id)
            .fetch_optional(&mut *transaction).await.map_err(store_error)?.ok_or(DesktopCommandError::Conflict)?;
        let record = row_to_record(&row)?;
        insert_audit(
            &mut transaction,
            row.get("tenant_id"),
            "desktop.command.acknowledged",
            &record.command.command_id,
            &record.command.execution_id,
            device_id,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(record)
    }

    pub async fn complete(
        &self,
        device_id: &str,
        result: DesktopCommandResult,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        result
            .validate()
            .map_err(|error| DesktopCommandError::Store(error.to_string()))?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let result_json = serde_json::to_value(&result).map_err(store_error)?;
        let status = outcome_status(result.outcome);
        let row = sqlx::query("UPDATE af_desktop_commands SET status=$1, result_json=$2, completed_at=now(), updated_at=now() WHERE id=$3 AND execution_id=$4 AND device_id=$5 AND status='acknowledged' RETURNING command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms, tenant_id")
            .bind(status).bind(result_json).bind(&result.command_id).bind(&result.execution_id).bind(device_id)
            .fetch_optional(&mut *transaction).await.map_err(store_error)?;
        let row = match row {
            Some(row) => row,
            None => {
                drop(transaction);
                let existing = self.get_by_device(device_id, &result.command_id).await?;
                return if existing.result.as_ref() == Some(&result) {
                    Ok(existing)
                } else {
                    Err(DesktopCommandError::Conflict)
                };
            }
        };
        let record = row_to_record(&row)?;
        insert_audit(
            &mut transaction,
            row.get("tenant_id"),
            &format!("desktop.command.{status}"),
            &record.command.command_id,
            &record.command.execution_id,
            device_id,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(record)
    }

    async fn get_by_device(
        &self,
        device_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let row = sqlx::query("SELECT command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms FROM af_desktop_commands WHERE device_id=$1 AND id=$2")
            .bind(device_id).bind(command_id).fetch_optional(&self.pool).await.map_err(store_error)?.ok_or(DesktopCommandError::NotFound)?;
        row_to_record(&row)
    }

    pub async fn cancel(
        &self,
        tenant_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query("UPDATE af_desktop_commands SET status='cancelled', completed_at=now(), updated_at=now() WHERE tenant_id=$1 AND id=$2 AND status IN ('waiting_approval','queued','delivered','acknowledged') RETURNING command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms")
            .bind(tenant_id).bind(command_id).fetch_optional(&mut *transaction).await.map_err(store_error)?.ok_or(DesktopCommandError::Conflict)?;
        let record = row_to_record(&row)?;
        insert_audit(
            &mut transaction,
            tenant_id,
            "desktop.command.cancelled",
            command_id,
            &record.command.execution_id,
            &record.device_id,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(record)
    }

    pub async fn expire(&self, _now_unix_ms: u64) -> Vec<DesktopCommandRecord> {
        let Ok(mut transaction) = self.pool.begin().await else {
            return Vec::new();
        };
        let Ok(rows) = sqlx::query("UPDATE af_desktop_commands SET status='timed_out', completed_at=now(), updated_at=now() WHERE status IN ('waiting_approval','queued','delivered','acknowledged') AND expires_at <= now() RETURNING command_json, device_id, workflow_id, status, result_json, (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT created_at_ms, tenant_id")
            .fetch_all(&mut *transaction).await else { return Vec::new() };
        let mut records = Vec::with_capacity(rows.len());
        for row in &rows {
            let Ok(record) = row_to_record(row) else {
                return Vec::new();
            };
            if insert_audit(
                &mut transaction,
                row.get("tenant_id"),
                "desktop.command.timed_out",
                &record.command.command_id,
                &record.command.execution_id,
                &record.device_id,
            )
            .await
            .is_err()
            {
                return Vec::new();
            }
            records.push(record);
        }
        if transaction.commit().await.is_err() {
            return Vec::new();
        }
        records
    }
}

#[derive(Clone)]
pub enum PlatformDesktopCommandStore {
    Memory(MemoryDesktopCommandStore),
    Postgres(PostgresDesktopCommandStore),
}

impl Default for PlatformDesktopCommandStore {
    fn default() -> Self {
        Self::Memory(MemoryDesktopCommandStore::default())
    }
}

macro_rules! delegate {
    ($self:expr, $method:ident($($arg:expr),*)) => {
        match $self { Self::Memory(store) => store.$method($($arg),*).await, Self::Postgres(store) => store.$method($($arg),*).await }
    };
}

impl PlatformDesktopCommandStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresDesktopCommandStore::new(pool))
    }
    pub async fn create(
        &self,
        command: DesktopCommand,
        device_id: String,
        workflow_id: String,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        delegate!(self, create(command, device_id, workflow_id))
    }
    pub async fn get(
        &self,
        tenant_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        delegate!(self, get(tenant_id, command_id))
    }
    pub async fn pending_for_device(&self, device_id: &str) -> Vec<DesktopCommandRecord> {
        delegate!(self, pending_for_device(device_id))
    }
    pub async fn pending_approvals(
        &self,
        tenant_id: &str,
        limit: usize,
        offset: usize,
    ) -> Vec<DesktopCommandRecord> {
        delegate!(self, pending_approvals(tenant_id, limit, offset))
    }
    pub async fn approve(
        &self,
        tenant_id: &str,
        command_id: &str,
        approved_by: &str,
        now_unix_ms: u64,
    ) -> Result<(DesktopCommandRecord, bool), DesktopCommandError> {
        delegate!(
            self,
            approve(tenant_id, command_id, approved_by, now_unix_ms)
        )
    }
    pub async fn reject(
        &self,
        tenant_id: &str,
        command_id: &str,
        rejected_by: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        delegate!(self, reject(tenant_id, command_id, rejected_by))
    }
    pub async fn mark_delivered(
        &self,
        command_id: &str,
        now: u64,
    ) -> Result<(), DesktopCommandError> {
        delegate!(self, mark_delivered(command_id, now))
    }
    pub async fn acknowledge(
        &self,
        device_id: &str,
        ack: &DesktopCommandAcknowledgement,
        now: u64,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        delegate!(self, acknowledge(device_id, ack, now))
    }
    pub async fn complete(
        &self,
        device_id: &str,
        result: DesktopCommandResult,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        delegate!(self, complete(device_id, result))
    }
    pub async fn cancel(
        &self,
        tenant_id: &str,
        command_id: &str,
    ) -> Result<DesktopCommandRecord, DesktopCommandError> {
        delegate!(self, cancel(tenant_id, command_id))
    }
    pub async fn expire(&self, now: u64) -> Vec<DesktopCommandRecord> {
        delegate!(self, expire(now))
    }
}

fn outcome_status(outcome: CommandOutcome) -> &'static str {
    match outcome {
        CommandOutcome::Succeeded => "succeeded",
        CommandOutcome::Failed => "failed",
        CommandOutcome::Rejected => "rejected",
        CommandOutcome::Cancelled => "cancelled",
        CommandOutcome::TimedOut => "timed_out",
        CommandOutcome::AwaitingApproval => "rejected",
    }
}

fn initial_status(command: &DesktopCommand) -> &'static str {
    if command.action.risk_level() > RiskLevel::Low && command.approval.is_none() {
        "waiting_approval"
    } else {
        "queued"
    }
}

fn row_to_record(row: &sqlx::postgres::PgRow) -> Result<DesktopCommandRecord, DesktopCommandError> {
    Ok(DesktopCommandRecord {
        command: serde_json::from_value(row.get("command_json")).map_err(store_error)?,
        device_id: row.get("device_id"),
        workflow_id: row.get("workflow_id"),
        status: row.get("status"),
        result: row
            .try_get::<Option<serde_json::Value>, _>("result_json")
            .ok()
            .flatten()
            .map(serde_json::from_value)
            .transpose()
            .map_err(store_error)?,
        created_at_unix_ms: row.get::<i64, _>("created_at_ms") as u64,
    })
}

async fn insert_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
    action: &str,
    command_id: &str,
    execution_id: &str,
    device_id: &str,
) -> Result<(), DesktopCommandError> {
    sqlx::query("INSERT INTO af_audit_log (id, tenant_id, action, resource_type, resource_id, detail_json) VALUES ($1,$2,$3,'desktop_command',$4,$5)")
        .bind(format!("audit-desktop-command-{}", uuid::Uuid::new_v4())).bind(tenant_id).bind(action).bind(command_id)
        .bind(serde_json::json!({"execution_id": execution_id, "device_id": device_id}))
        .execute(&mut **transaction).await.map_err(store_error)?;
    Ok(())
}

async fn insert_decision_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
    action: &str,
    command_id: &str,
    execution_id: &str,
    device_id: &str,
    actor_id: &str,
) -> Result<(), DesktopCommandError> {
    sqlx::query("INSERT INTO af_audit_log (id, tenant_id, action, resource_type, resource_id, detail_json) VALUES ($1,$2,$3,'desktop_command',$4,$5)")
        .bind(format!("audit-desktop-command-{}", uuid::Uuid::new_v4()))
        .bind(tenant_id)
        .bind(action)
        .bind(command_id)
        .bind(serde_json::json!({
            "execution_id": execution_id,
            "device_id": device_id,
            "actor_id": actor_id,
        }))
        .execute(&mut **transaction)
        .await
        .map_err(store_error)?;
    Ok(())
}

fn store_error(error: impl std::fmt::Display) -> DesktopCommandError {
    DesktopCommandError::Store(error.to_string())
}
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_protocol::{DesktopAction, ExecutionLease};

    fn command(id: &str, expires: u64) -> DesktopCommand {
        DesktopCommand {
            command_id: id.to_string(),
            execution_id: "execution-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            project_id: "project-1".to_string(),
            requested_by: "user-1".to_string(),
            issued_at_unix_ms: 100,
            lease: ExecutionLease {
                lease_id: format!("lease-{id}"),
                expires_at_unix_ms: expires,
            },
            approval: None,
            action: DesktopAction::ReadSystemInformation,
        }
    }

    fn approval_command(id: &str, expires: u64) -> DesktopCommand {
        let mut value = command(id, expires);
        value.action = serde_json::from_value(serde_json::json!({
            "kind": "focus_window",
            "selector": { "executable": "fixture.exe" }
        }))
        .unwrap();
        value
    }

    #[tokio::test]
    async fn risky_commands_wait_for_one_tenant_scoped_approval() {
        let store = MemoryDesktopCommandStore::default();
        let created = store
            .create(
                approval_command("approval", 1_000),
                "device-1".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(created.status, "waiting_approval");
        assert!(store.pending_for_device("device-1").await.is_empty());
        assert!(store.pending_approvals("tenant-2", 10, 0).await.is_empty());
        assert_eq!(store.pending_approvals("tenant-1", 10, 0).await.len(), 1);

        let approved = store
            .approve("tenant-1", "approval", "operator-1", 200)
            .await
            .unwrap()
            .0;
        assert_eq!(approved.status, "queued");
        assert_eq!(approved.command.approval.unwrap().approved_by, "operator-1");
        assert_eq!(store.pending_for_device("device-1").await.len(), 1);
        assert_eq!(
            store
                .approve("tenant-1", "approval", "operator-1", 201)
                .await
                .unwrap()
                .0
                .status,
            "queued"
        );
    }

    #[tokio::test]
    async fn approval_rejection_and_expiry_are_terminal() {
        let store = MemoryDesktopCommandStore::default();
        store
            .create(
                approval_command("reject-approval", 1_000),
                "device-1".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .reject("tenant-1", "reject-approval", "operator-1")
                .await
                .unwrap()
                .status,
            "rejected"
        );
        assert_eq!(
            store
                .reject("tenant-1", "reject-approval", "operator-1")
                .await
                .unwrap()
                .status,
            "rejected"
        );

        store
            .create(
                approval_command("expired-approval", 300),
                "device-1".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .approve("tenant-1", "expired-approval", "operator-1", 300)
                .await,
            Err(DesktopCommandError::Expired)
        );
        assert_eq!(
            store
                .get("tenant-1", "expired-approval")
                .await
                .unwrap()
                .status,
            "timed_out"
        );
    }

    #[tokio::test]
    async fn duplicate_completion_is_idempotent_but_cannot_repeat_a_side_effect() {
        let store = MemoryDesktopCommandStore::default();
        store
            .create(
                command("command-1", 1000),
                "device-1".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        store.mark_delivered("command-1", 200).await.unwrap();
        store
            .acknowledge(
                "device-1",
                &DesktopCommandAcknowledgement {
                    command_id: "command-1".to_string(),
                    execution_id: "execution-1".to_string(),
                    lease_id: "lease-command-1".to_string(),
                    acknowledged_at_unix_ms: 210,
                },
                210,
            )
            .await
            .unwrap();
        let result = DesktopCommandResult {
            command_id: "command-1".to_string(),
            execution_id: "execution-1".to_string(),
            outcome: CommandOutcome::Succeeded,
            completed_at_unix_ms: 220,
            output: Some(serde_json::json!({"ok": true})),
            error_code: None,
            error_message: None,
        };
        assert_eq!(
            store
                .complete("device-1", result.clone())
                .await
                .unwrap()
                .status,
            "succeeded"
        );
        assert!(store.complete("device-1", result).await.is_ok());
        assert_eq!(
            store
                .complete(
                    "device-1",
                    DesktopCommandResult {
                        command_id: "command-1".to_string(),
                        execution_id: "execution-1".to_string(),
                        outcome: CommandOutcome::Succeeded,
                        completed_at_unix_ms: 221,
                        output: None,
                        error_code: None,
                        error_message: None
                    }
                )
                .await,
            Err(DesktopCommandError::Conflict)
        );
    }

    #[tokio::test]
    async fn cancellation_and_timeout_are_terminal() {
        let store = MemoryDesktopCommandStore::default();
        store
            .create(
                command("cancel", 1000),
                "device-1".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(
            store.cancel("tenant-1", "cancel").await.unwrap().status,
            "cancelled"
        );
        store
            .create(
                command("timeout", 300),
                "device-1".to_string(),
                "workflow-1".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(store.expire(300).await[0].status, "timed_out");
    }

    #[tokio::test]
    async fn failed_and_rejected_results_are_explicit_terminal_states() {
        let store = MemoryDesktopCommandStore::default();
        for (id, outcome, expected) in [
            ("failed", CommandOutcome::Failed, "failed"),
            ("rejected", CommandOutcome::Rejected, "rejected"),
        ] {
            store
                .create(
                    command(id, 1000),
                    "device-1".to_string(),
                    "workflow-1".to_string(),
                )
                .await
                .unwrap();
            store.mark_delivered(id, 200).await.unwrap();
            store
                .acknowledge(
                    "device-1",
                    &DesktopCommandAcknowledgement {
                        command_id: id.to_string(),
                        execution_id: "execution-1".to_string(),
                        lease_id: format!("lease-{id}"),
                        acknowledged_at_unix_ms: 210,
                    },
                    210,
                )
                .await
                .unwrap();
            assert_eq!(
                store
                    .complete(
                        "device-1",
                        DesktopCommandResult {
                            command_id: id.to_string(),
                            execution_id: "execution-1".to_string(),
                            outcome,
                            completed_at_unix_ms: 220,
                            output: None,
                            error_code: Some("operation_failed".to_string()),
                            error_message: Some("redacted".to_string()),
                        },
                    )
                    .await
                    .unwrap()
                    .status,
                expected
            );
        }
    }
}
