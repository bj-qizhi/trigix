// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowComment {
    pub id: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub author: String,
    pub body: String,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCommentRequest {
    pub tenant_id: String,
    pub workflow_id: String,
    pub author: String,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditCommentRequest {
    pub tenant_id: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentError {
    NotFound,
    StoreUnavailable,
    EmptyBody,
}

fn next_id() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Memory store ─────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct MemoryCommentStore {
    comments: Arc<RwLock<HashMap<String, WorkflowComment>>>,
}

impl MemoryCommentStore {
    pub fn create(&self, req: CreateCommentRequest) -> Result<WorkflowComment, CommentError> {
        if req.body.trim().is_empty() {
            return Err(CommentError::EmptyBody);
        }
        let comment = WorkflowComment {
            id: next_id(),
            tenant_id: req.tenant_id,
            workflow_id: req.workflow_id,
            author: req.author,
            body: req.body.trim().to_string(),
            created_at: unix_now(),
            edited_at: None,
        };
        self.comments
            .write()
            .unwrap()
            .insert(comment.id.clone(), comment.clone());
        Ok(comment)
    }

    pub fn list(&self, tenant_id: &str, workflow_id: &str) -> Vec<WorkflowComment> {
        let map = self.comments.read().unwrap();
        let mut out: Vec<WorkflowComment> = map
            .values()
            .filter(|c| c.tenant_id == tenant_id && c.workflow_id == workflow_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        out
    }

    pub fn edit(
        &self,
        tenant_id: &str,
        comment_id: &str,
        req: EditCommentRequest,
    ) -> Result<WorkflowComment, CommentError> {
        if req.body.trim().is_empty() {
            return Err(CommentError::EmptyBody);
        }
        let mut map = self.comments.write().unwrap();
        let comment = map.get_mut(comment_id).ok_or(CommentError::NotFound)?;
        if comment.tenant_id != tenant_id {
            return Err(CommentError::NotFound);
        }
        comment.body = req.body.trim().to_string();
        comment.edited_at = Some(unix_now());
        Ok(comment.clone())
    }

    pub fn delete(&self, tenant_id: &str, comment_id: &str) -> Result<(), CommentError> {
        let mut map = self.comments.write().unwrap();
        let comment = map.get(comment_id).ok_or(CommentError::NotFound)?;
        if comment.tenant_id != tenant_id {
            return Err(CommentError::NotFound);
        }
        map.remove(comment_id);
        Ok(())
    }
}

// ── Postgres store ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PostgresCommentStore {
    pool: PgPool,
}

impl PostgresCommentStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, req: CreateCommentRequest) -> Result<WorkflowComment, CommentError> {
        if req.body.trim().is_empty() {
            return Err(CommentError::EmptyBody);
        }
        let id = next_id();
        let now = unix_now() as i64;
        let body = req.body.trim().to_string();
        sqlx::query(
            "INSERT INTO af_workflow_comments (id, tenant_id, workflow_id, author, body, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&id).bind(&req.tenant_id).bind(&req.workflow_id)
        .bind(&req.author).bind(&body).bind(now)
        .execute(&self.pool).await.map_err(|_| CommentError::StoreUnavailable)?;
        Ok(WorkflowComment {
            id,
            tenant_id: req.tenant_id,
            workflow_id: req.workflow_id,
            author: req.author,
            body,
            created_at: now as u64,
            edited_at: None,
        })
    }

    pub async fn list(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<WorkflowComment>, CommentError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            tenant_id: String,
            workflow_id: String,
            author: String,
            body: String,
            created_at: i64,
            #[sqlx(default)]
            edited_at: Option<i64>,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, tenant_id, workflow_id, author, body, created_at, edited_at
             FROM af_workflow_comments
             WHERE tenant_id = $1 AND workflow_id = $2
             ORDER BY created_at ASC, id ASC",
        )
        .bind(tenant_id)
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommentError::StoreUnavailable)?;
        Ok(rows
            .into_iter()
            .map(|r| WorkflowComment {
                id: r.id,
                tenant_id: r.tenant_id,
                workflow_id: r.workflow_id,
                author: r.author,
                body: r.body,
                created_at: r.created_at as u64,
                edited_at: r.edited_at.map(|t| t as u64),
            })
            .collect())
    }

    pub async fn edit(
        &self,
        tenant_id: &str,
        comment_id: &str,
        req: EditCommentRequest,
    ) -> Result<WorkflowComment, CommentError> {
        if req.body.trim().is_empty() {
            return Err(CommentError::EmptyBody);
        }
        let body = req.body.trim().to_string();
        let now = unix_now() as i64;
        let result = sqlx::query(
            "UPDATE af_workflow_comments SET body = $1, edited_at = $2
             WHERE id = $3 AND tenant_id = $4",
        )
        .bind(&body)
        .bind(now)
        .bind(comment_id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommentError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            return Err(CommentError::NotFound);
        }

        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            tenant_id: String,
            workflow_id: String,
            author: String,
            body: String,
            created_at: i64,
            #[sqlx(default)]
            edited_at: Option<i64>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT id, tenant_id, workflow_id, author, body, created_at, edited_at
             FROM af_workflow_comments WHERE id = $1",
        )
        .bind(comment_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| CommentError::StoreUnavailable)?;
        Ok(WorkflowComment {
            id: row.id,
            tenant_id: row.tenant_id,
            workflow_id: row.workflow_id,
            author: row.author,
            body: row.body,
            created_at: row.created_at as u64,
            edited_at: row.edited_at.map(|t| t as u64),
        })
    }

    pub async fn delete(&self, tenant_id: &str, comment_id: &str) -> Result<(), CommentError> {
        let result =
            sqlx::query("DELETE FROM af_workflow_comments WHERE id = $1 AND tenant_id = $2")
                .bind(comment_id)
                .bind(tenant_id)
                .execute(&self.pool)
                .await
                .map_err(|_| CommentError::StoreUnavailable)?;
        if result.rows_affected() == 0 {
            Err(CommentError::NotFound)
        } else {
            Ok(())
        }
    }
}

// ── Platform enum ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum PlatformCommentStore {
    Memory(MemoryCommentStore),
    Postgres(PostgresCommentStore),
}

impl Default for PlatformCommentStore {
    fn default() -> Self {
        Self::Memory(MemoryCommentStore::default())
    }
}

impl PlatformCommentStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresCommentStore::new(pool))
    }

    pub async fn create(&self, req: CreateCommentRequest) -> Result<WorkflowComment, CommentError> {
        match self {
            Self::Memory(s) => s.create(req),
            Self::Postgres(s) => s.create(req).await,
        }
    }

    pub async fn list(
        &self,
        tenant_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<WorkflowComment>, CommentError> {
        match self {
            Self::Memory(s) => Ok(s.list(tenant_id, workflow_id)),
            Self::Postgres(s) => s.list(tenant_id, workflow_id).await,
        }
    }

    pub async fn edit(
        &self,
        tenant_id: &str,
        comment_id: &str,
        req: EditCommentRequest,
    ) -> Result<WorkflowComment, CommentError> {
        match self {
            Self::Memory(s) => s.edit(tenant_id, comment_id, req),
            Self::Postgres(s) => s.edit(tenant_id, comment_id, req).await,
        }
    }

    pub async fn delete(&self, tenant_id: &str, comment_id: &str) -> Result<(), CommentError> {
        match self {
            Self::Memory(s) => s.delete(tenant_id, comment_id),
            Self::Postgres(s) => s.delete(tenant_id, comment_id).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> MemoryCommentStore {
        MemoryCommentStore::default()
    }

    fn make_req(tenant: &str, workflow: &str, author: &str, body: &str) -> CreateCommentRequest {
        CreateCommentRequest {
            tenant_id: tenant.to_string(),
            workflow_id: workflow.to_string(),
            author: author.to_string(),
            body: body.to_string(),
        }
    }

    #[test]
    fn create_and_list_comments() {
        let s = store();
        s.create(make_req("t1", "wf-1", "alice", "First comment"))
            .unwrap();
        s.create(make_req("t1", "wf-1", "bob", "Second comment"))
            .unwrap();
        let comments = s.list("t1", "wf-1");
        assert_eq!(comments.len(), 2);
        let authors: Vec<&str> = comments.iter().map(|c| c.author.as_str()).collect();
        assert!(authors.contains(&"alice"));
        assert!(authors.contains(&"bob"));
        // Different tenant sees nothing
        assert!(s.list("t2", "wf-1").is_empty());
        // Different workflow sees nothing
        assert!(s.list("t1", "wf-2").is_empty());
    }

    #[test]
    fn edit_comment_updates_body_and_sets_edited_at() {
        let s = store();
        let comment = s
            .create(make_req("t1", "wf-1", "alice", "Original body"))
            .unwrap();
        let edited = s
            .edit(
                "t1",
                &comment.id,
                EditCommentRequest {
                    tenant_id: "t1".into(),
                    body: "Updated body".into(),
                },
            )
            .unwrap();
        assert_eq!(edited.body, "Updated body");
        assert!(edited.edited_at.is_some());
        assert!(edited.edited_at.unwrap() >= comment.created_at);
    }

    #[test]
    fn delete_comment_removes_it() {
        let s = store();
        let c = s
            .create(make_req("t1", "wf-1", "alice", "To delete"))
            .unwrap();
        assert_eq!(s.list("t1", "wf-1").len(), 1);
        s.delete("t1", &c.id).unwrap();
        assert!(s.list("t1", "wf-1").is_empty());
    }

    #[test]
    fn delete_wrong_tenant_returns_not_found() {
        let s = store();
        let c = s.create(make_req("t1", "wf-1", "alice", "body")).unwrap();
        let err = s.delete("t2", &c.id).unwrap_err();
        assert_eq!(err, CommentError::NotFound);
    }

    #[test]
    fn empty_body_returns_error() {
        let s = store();
        let err = s.create(make_req("t1", "wf-1", "alice", "  ")).unwrap_err();
        assert_eq!(err, CommentError::EmptyBody);
    }
}
