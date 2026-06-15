// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;

/// Operator view of acquisition ROI: per channel, how many tenants signed up,
/// how many converted to paid, and the converted revenue. Admin-gated and global
/// (it spans tenants by design), mirroring the cross-tenant `admin_list_users`.
async fn acquisition_channels_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
) -> Result<Json<Vec<crate::attribution::ChannelStats>>, ApiError> {
    require_admin(&claims)?;
    use crate::attribution::AttributionStore;
    Ok(Json(state.attribution_store.channel_revenue().await))
}

async fn list_audit_log(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<AuditLogQuery>,
) -> Json<Vec<crate::audit::AuditEvent>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let limit = query.limit.unwrap_or(200).min(1000);
    let events = state.audit_store.list(&tenant_id, limit).await;
    let filtered = events
        .into_iter()
        .filter(|e| query.action.as_ref().map_or(true, |a| &e.action == a))
        .filter(|e| {
            query
                .resource_id
                .as_ref()
                .map_or(true, |r| &e.resource_id == r)
        })
        .collect();
    Json(filtered)
}

// ── Token Usage ───────────────────────────────────────────────────────────

async fn node_type_analytics_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TenantQuery>,
) -> Json<Vec<NodeTypeStat>> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let execs = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let ids: Vec<String> = execs.iter().map(|e| e.id.clone()).collect();

    // Track sum+count for avg_duration_ms computation
    let mut map: std::collections::HashMap<String, NodeTypeStat> = std::collections::HashMap::new();
    let mut dur_sum: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut dur_count: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for id in ids {
        if let Ok(rec) = state.execution_service.get(&tenant_id, &id).await {
            for nr in &rec.node_results {
                let st = map.entry(nr.node_type.clone()).or_insert(NodeTypeStat {
                    node_type: nr.node_type.clone(),
                    total: 0,
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    avg_duration_ms: None,
                });
                st.total += 1;
                match nr.status {
                    execution_core::NodeStatus::Succeeded => st.succeeded += 1,
                    execution_core::NodeStatus::Failed => st.failed += 1,
                    execution_core::NodeStatus::Skipped => st.skipped += 1,
                    _ => {}
                }
                if nr.duration_ms > 0 {
                    *dur_sum.entry(nr.node_type.clone()).or_insert(0) += nr.duration_ms;
                    *dur_count.entry(nr.node_type.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    for (node_type, st) in map.iter_mut() {
        if let (Some(&sum), Some(&cnt)) = (dur_sum.get(node_type), dur_count.get(node_type)) {
            if cnt > 0 {
                st.avg_duration_ms = Some(sum / cnt);
            }
        }
    }
    let mut stats: Vec<_> = map.into_values().collect();
    stats.sort_by(|a, b| b.total.cmp(&a.total));
    Json(stats)
}

// ── Workflow dependency graph ─────────────────────────────────────────────

async fn workflow_deps_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<TenantQuery>,
) -> Json<WorkflowDepsResponse> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .unwrap_or_default();
    let mut edges = Vec::new();

    for wf in &workflows {
        let Some(ref vid) = wf.latest_version_id else {
            continue;
        };
        let Ok(version) = state.workflow_service.get_version(&tenant_id, vid).await else {
            continue;
        };
        for node in &version.graph.nodes {
            let nt = format!("{:?}", node.node_type).to_lowercase();
            if nt != "subworkflow" && nt != "foreach" {
                continue;
            }
            let config = node.config.as_ref().unwrap_or(&serde_json::Value::Null);
            let Some(target_id) = config.get("workflow_id").and_then(|v| v.as_str()) else {
                continue;
            };
            if target_id != wf.id {
                edges.push(WorkflowDepEdge {
                    from_workflow_id: wf.id.clone(),
                    to_workflow_id: target_id.to_string(),
                    node_type: nt,
                });
            }
        }
    }

    Json(WorkflowDepsResponse { edges })
}

// ── Workflow Stats Analytics ───────────────────────────────────────────────

async fn workflow_stats_analytics_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkflowStatsQuery>,
) -> Json<WorkflowStatsAnalyticsResponse> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));
    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let since = now_secs.saturating_sub(query.days * 86400);

    // Aggregate per workflow_id
    let mut map: std::collections::HashMap<String, WorkflowStatRow> =
        std::collections::HashMap::new();
    let mut duration_sum: std::collections::HashMap<String, (u64, f64)> =
        std::collections::HashMap::new();

    for ex in executions.iter().filter(|e| e.started_at >= since) {
        let entry = map
            .entry(ex.workflow_id.clone())
            .or_insert_with(|| WorkflowStatRow {
                workflow_id: ex.workflow_id.clone(),
                total: 0,
                succeeded: 0,
                failed: 0,
                cancelled: 0,
                running: 0,
                avg_duration_secs: None,
                last_run_at: None,
            });
        entry.total += 1;
        match ex.status {
            ExecutionStatus::Succeeded => entry.succeeded += 1,
            ExecutionStatus::Failed => entry.failed += 1,
            ExecutionStatus::Cancelled => entry.cancelled += 1,
            ExecutionStatus::Running | ExecutionStatus::WaitingApproval => entry.running += 1,
        }
        if let Some(fin) = ex.finished_at {
            if fin > ex.started_at {
                let dur = (fin - ex.started_at) as f64;
                let d = duration_sum
                    .entry(ex.workflow_id.clone())
                    .or_insert((0, 0.0));
                d.0 += 1;
                d.1 += dur;
            }
        }
        if entry.last_run_at.map_or(true, |t| ex.started_at > t) {
            entry.last_run_at = Some(ex.started_at);
        }
    }

    for (wf_id, (count, sum)) in &duration_sum {
        if let Some(row) = map.get_mut(wf_id) {
            row.avg_duration_secs = Some(sum / *count as f64);
        }
    }

    let mut rows: Vec<WorkflowStatRow> = map.into_values().collect();
    rows.sort_by(|a, b| b.total.cmp(&a.total));

    Json(WorkflowStatsAnalyticsResponse { rows, since })
}

// ── SLA breach analytics ──────────────────────────────────────────────────

async fn sla_breaches_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkflowStatsQuery>,
) -> Json<SlaBreachesResponse> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let since = now_secs.saturating_sub(query.days * 86400);

    // fetch workflows with SLA set
    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .unwrap_or_default();
    let sla_map: std::collections::HashMap<String, (String, u64)> = workflows
        .iter()
        .filter_map(|w| w.sla_seconds.map(|s| (w.id.clone(), (w.name.clone(), s))))
        .collect();

    if sla_map.is_empty() {
        return Json(SlaBreachesResponse {
            breaches: vec![],
            total_workflows_with_sla: 0,
            compliance_rate: 100.0,
            total_completed: 0,
        });
    }

    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();

    let mut breaches = Vec::new();
    let mut total_completed = 0usize;
    let mut total_compliant = 0usize;

    for ex in executions.iter().filter(|e| e.started_at >= since) {
        if let Some((wf_name, sla)) = sla_map.get(&ex.workflow_id) {
            if ex.status == ExecutionStatus::Succeeded || ex.status == ExecutionStatus::Failed {
                if let Some(fin) = ex.finished_at {
                    total_completed += 1;
                    let elapsed = fin.saturating_sub(ex.started_at);
                    if elapsed > *sla {
                        breaches.push(SlaBreachEntry {
                            execution_id: ex.id.clone(),
                            workflow_id: ex.workflow_id.clone(),
                            workflow_name: wf_name.clone(),
                            sla_seconds: *sla,
                            elapsed_seconds: elapsed,
                            overage_seconds: elapsed - sla,
                            started_at: ex.started_at,
                            finished_at: fin,
                        });
                    } else {
                        total_compliant += 1;
                    }
                }
            }
        }
    }

    breaches.sort_by(|a, b| b.overage_seconds.cmp(&a.overage_seconds));

    let compliance_rate = if total_completed > 0 {
        (total_compliant as f64 / total_completed as f64) * 100.0
    } else {
        100.0
    };

    Json(SlaBreachesResponse {
        breaches,
        total_workflows_with_sla: sla_map.len(),
        compliance_rate,
        total_completed,
    })
}

// ── Error analysis analytics ──────────────────────────────────────────────

async fn error_analysis_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Query(query): Query<WorkflowStatsQuery>,
) -> Json<ErrorAnalysisResponse> {
    let tenant_id = effective_tenant_id(&claims, query.tenant_id.as_deref().unwrap_or(""));
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let since = now_secs.saturating_sub(query.days * 86400);

    let workflows = state
        .workflow_service
        .list_workflows(&tenant_id, None, None, None)
        .await
        .unwrap_or_default();
    let wf_name_map: HashMap<String, String> = workflows
        .iter()
        .map(|w| (w.id.clone(), w.name.clone()))
        .collect();

    let executions = state
        .execution_service
        .list(&tenant_id)
        .await
        .unwrap_or_default();
    let recent_failed: Vec<_> = executions
        .iter()
        .filter(|e| e.started_at >= since && matches!(e.status, ExecutionStatus::Failed))
        .collect();

    // key = truncated error message (first 120 chars)
    let mut error_map: HashMap<String, TopErrorEntry> = HashMap::new();
    let mut total_failed_nodes = 0usize;

    for ex in &recent_failed {
        if let Ok(full) = state.execution_service.get(&tenant_id, &ex.id).await {
            for nr in &full.node_results {
                if matches!(nr.status, execution_core::NodeStatus::Failed) {
                    total_failed_nodes += 1;
                    let raw = nr.error.as_deref().unwrap_or("unknown error");
                    let key: String = raw.chars().take(120).collect();
                    let entry = error_map.entry(key.clone()).or_insert(TopErrorEntry {
                        error_message: key,
                        count: 0,
                        node_type: nr.node_type.clone(),
                        workflow_id: ex.workflow_id.clone(),
                        workflow_name: wf_name_map
                            .get(&ex.workflow_id)
                            .cloned()
                            .unwrap_or_default(),
                        last_seen: 0,
                    });
                    entry.count += 1;
                    if ex.started_at > entry.last_seen {
                        entry.last_seen = ex.started_at;
                    }
                }
            }
        }
    }

    let distinct_error_types = error_map.len();
    let mut top_errors: Vec<_> = error_map.into_values().collect();
    top_errors.sort_by(|a, b| b.count.cmp(&a.count));
    top_errors.truncate(20);

    Json(ErrorAnalysisResponse {
        top_errors,
        total_failed_nodes,
        distinct_error_types,
    })
}

// ── Variables ─────────────────────────────────────────────────────────────

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/audit-log", get(list_audit_log))
        .route("/v1/analytics/node-types", get(node_type_analytics_handler))
        .route("/v1/analytics/workflow-deps", get(workflow_deps_handler))
        .route(
            "/v1/analytics/workflow-stats",
            get(workflow_stats_analytics_handler),
        )
        .route("/v1/analytics/sla-breaches", get(sla_breaches_handler))
        .route("/v1/analytics/errors", get(error_analysis_handler))
        .route(
            "/v1/analytics/attribution",
            get(acquisition_channels_handler),
        )
}

#[cfg(test)]
mod tests {
    use crate::http::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn node_type_analytics_returns_empty_initially() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/analytics/node-types?tenant_id=t1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let stats: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(stats.as_array().is_some());
    }
}
