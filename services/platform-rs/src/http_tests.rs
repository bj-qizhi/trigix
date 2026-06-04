// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use super::*;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn starts_and_gets_execution_over_http() {
    let app = router();
    // Use trigger → transform so we can verify node-output template resolution
    // without requiring an external AI runtime.
    let request_body = json!({
        "tenant_id": "tenant-1",
        "workflow_id": "workflow-1",
        "workflow_version_id": "version-1",
        "graph": {
            "workflow_version_id": "version-1",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "xform", "type": "transform",
                 "config": {"template": {"result": "{{input.lead_id}}"}}}
            ],
            "edges": [
                {"source": "trigger", "target": "xform"}
            ]
        },
        "input_json": "{\"lead_id\":\"lead-1\"}"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["tenant_id"], "tenant-1");
    assert_eq!(payload["workflow_id"], "workflow-1");
    assert_eq!(payload["workflow_version_id"], "version-1");
    assert_eq!(payload["status"], "running");

    let execution_id = payload["id"].as_str().unwrap().to_string();

    // Wait for background execution to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/executions?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload.as_array().unwrap().len(), 1);
    assert_eq!(payload[0]["id"], execution_id);
    assert_eq!(payload[0]["status"], "succeeded");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], execution_id);
    assert_eq!(payload["status"], "succeeded");
    assert_eq!(payload["node_results"][0]["node_id"], "trigger");
    assert_eq!(payload["node_results"][0]["node_type"], "trigger");
    assert_eq!(payload["node_results"][0]["status"], "succeeded");
    // Transform output should contain the resolved input value
    let xform_out: serde_json::Value = serde_json::from_str(
        payload["node_results"][1]["output_json"]
            .as_str()
            .unwrap_or("{}"),
    )
    .unwrap();
    assert_eq!(xform_out["result"], "lead-1");
}

#[tokio::test]
async fn creates_and_lists_workflows_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1",
        "workspace_id": "workspace-1",
        "project_id": "project-1",
        "name": "New Workflow"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let workflow_id = payload["id"].as_str().unwrap();

    assert_eq!(payload["name"], "New Workflow");
    assert_eq!(payload["status"], "draft");
    assert!(payload["latest_version_id"].is_null());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows?tenant_id=tenant-1&project_id=project-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(payload
        .as_array()
        .unwrap()
        .iter()
        .any(|workflow| workflow["id"] == workflow_id));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows?tenant_id=tenant-1&project_id=project-1&status=draft")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload.as_array().unwrap().len(), 1);
    assert_eq!(payload[0]["id"], workflow_id);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows?tenant_id=tenant-1&status=deleted")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn gets_workflow_over_http() {
    let app = router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows/workflow-1?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], "workflow-1");
    assert_eq!(payload["name"], "Dev Lead Workflow");
    assert_eq!(payload["status"], "published");
    assert_eq!(payload["latest_version_id"], "version-1");
}

#[tokio::test]
async fn updates_workflow_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1",
        "name": "Renamed Workflow"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/workflows/workflow-1")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], "workflow-1");
    assert_eq!(payload["name"], "Renamed Workflow");
    assert_eq!(payload["status"], "published");
    assert_eq!(payload["latest_version_id"], "version-1");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows/workflow-1?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["name"], "Renamed Workflow");
}

#[tokio::test]
async fn archives_workflow_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/archive")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], "workflow-1");
    assert_eq!(payload["status"], "archived");
    assert_eq!(payload["latest_version_id"], "version-1");

    let run_body = json!({
        "tenant_id": "tenant-1",
        "input_json": "{\"lead_id\":\"lead-from-archived-workflow\"}"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(run_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let version_run_body = json!({
        "tenant_id": "tenant-1",
        "input_json": "{\"lead_id\":\"lead-from-archived-version\"}"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-versions/version-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(version_run_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let restore_body = json!({
        "tenant_id": "tenant-1"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/restore")
                .header("content-type", "application/json")
                .body(Body::from(restore_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], "workflow-1");
    assert_eq!(payload["status"], "published");
    assert_eq!(payload["latest_version_id"], "version-1");

    let run_body = json!({
        "tenant_id": "tenant-1",
        "input_json": "{\"lead_id\":\"lead-from-restored-workflow\"}"
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(run_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn starts_execution_from_workflow_version_over_http() {
    let app = router();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflow-versions/version-1?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], "version-1");
    assert_eq!(payload["workflow_id"], "workflow-1");
    assert_eq!(payload["graph"]["nodes"].as_array().unwrap().len(), 2);

    let request_body = json!({
        "tenant_id": "tenant-1",
        "input_json": "{\"lead_id\":\"lead-from-version\"}"
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-versions/version-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["tenant_id"], "tenant-1");
    assert_eq!(payload["workflow_id"], "workflow-1");
    assert_eq!(payload["workflow_version_id"], "version-1");
    assert_eq!(payload["status"], "running");
}

#[tokio::test]
async fn draft_version_execution_is_rejected() {
    let app = router();

    // Create a new (draft) version of workflow-1
    let create_body = json!({
        "tenant_id": "tenant-1",
        "graph": {
            "workflow_version_id": "draft-version-x",
            "nodes": [{"id": "trigger", "type": "trigger"}],
            "edges": []
        }
    });
    let create_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/versions")
                .header("content-type", "application/json")
                .body(Body::from(create_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let body = to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
    let version: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let draft_id = version["id"].as_str().unwrap().to_string();
    assert_eq!(version["status"], "draft");

    // Trying to run the draft version must be rejected
    let exec_body = json!({
        "tenant_id": "tenant-1",
        "input_json": "{}"
    });
    let exec_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflow-versions/{draft_id}/executions"))
                .header("content-type", "application/json")
                .body(Body::from(exec_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exec_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_execution_removes_terminal_execution() {
    let app = router();
    // Start execution on pre-seeded workflow
    let start_body = json!({ "tenant_id": "tenant-1", "input_json": "{}" });
    let start_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-versions/version-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(start_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start_resp.status(), StatusCode::ACCEPTED);
    let bytes = to_bytes(start_resp.into_body(), usize::MAX).await.unwrap();
    let exec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let exec_id = exec["id"].as_str().unwrap().to_string();

    // Wait for the background executor task to complete
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Delete the finished execution
    let del_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/executions/{exec_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let get_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/executions/{exec_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn starts_execution_from_latest_workflow_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1",
        "input_json": "{\"lead_id\":\"lead-from-workflow\"}"
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["tenant_id"], "tenant-1");
    assert_eq!(payload["workflow_id"], "workflow-1");
    assert_eq!(payload["workflow_version_id"], "version-1");
    assert_eq!(payload["status"], "running");
}

#[tokio::test]
async fn creates_workflow_version_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1",
        "graph": {
            "workflow_version_id": "client-supplied-id",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "agent", "type": "agent"}
            ],
            "edges": [
                {"source": "trigger", "target": "agent"}
            ]
        }
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/versions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let workflow_version_id = payload["id"].as_str().unwrap();

    assert_ne!(workflow_version_id, "client-supplied-id");
    assert_eq!(payload["workflow_id"], "workflow-1");
    assert_eq!(payload["version"], 2);
    assert_eq!(payload["status"], "draft");
    assert_eq!(payload["graph"]["workflow_version_id"], workflow_version_id);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/v1/workflow-versions/{workflow_version_id}?tenant_id=tenant-1"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn lists_workflow_versions_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1",
        "graph": {
            "workflow_version_id": "client-supplied-id",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "agent", "type": "agent"}
            ],
            "edges": [
                {"source": "trigger", "target": "agent"}
            ]
        }
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/versions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows/workflow-1/versions?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload.as_array().unwrap().len(), 2);
    assert_eq!(payload[0]["version"], 2);
    assert_eq!(payload[1]["version"], 1);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows/workflow-1/versions?tenant_id=tenant-1&status=draft")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload.as_array().unwrap().len(), 1);
    assert_eq!(payload[0]["version"], 2);
    assert_eq!(payload[0]["status"], "draft");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows/workflow-1/versions?tenant_id=tenant-1&status=archived")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn publishes_workflow_version_over_http() {
    let app = router();
    let workflow_body = json!({
        "tenant_id": "tenant-1",
        "workspace_id": "workspace-1",
        "project_id": "project-1",
        "name": "New Workflow"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(workflow_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let workflow: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let workflow_id = workflow["id"].as_str().unwrap();

    let version_body = json!({
        "tenant_id": "tenant-1",
        "graph": {
            "workflow_version_id": "client-supplied-id",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "agent", "type": "agent"}
            ],
            "edges": [
                {"source": "trigger", "target": "agent"}
            ]
        }
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{workflow_id}/versions"))
                .header("content-type", "application/json")
                .body(Body::from(version_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let version: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let workflow_version_id = version["id"].as_str().unwrap();

    let publish_body = json!({
        "tenant_id": "tenant-1"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/workflow-versions/{workflow_version_id}/publish"
                ))
                .header("content-type", "application/json")
                .body(Body::from(publish_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["id"], workflow_version_id);
    assert_eq!(payload["status"], "published");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows?tenant_id=tenant-1&project_id=project-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let workflow = payload
        .as_array()
        .unwrap()
        .iter()
        .find(|workflow| workflow["id"] == workflow_id)
        .unwrap();

    assert_eq!(workflow["status"], "published");
    assert_eq!(workflow["latest_version_id"], workflow_version_id);
}

#[tokio::test]
async fn approval_node_waits_and_resumes_on_approve() {
    let app = router();

    let request_body = json!({
        "tenant_id": "tenant-1",
        "workflow_id": "workflow-1",
        "workflow_version_id": "version-a",
        "graph": {
            "workflow_version_id": "version-a",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "approve", "type": "approval"}
            ],
            "edges": [{"source": "trigger", "target": "approve"}]
        },
        "input_json": "{}"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["status"], "running");
    let execution_id = payload["id"].as_str().unwrap().to_string();

    // Give the executor time to reach the approval node
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Status should be waiting_approval
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["status"], "waiting_approval");

    // Approve
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/executions/{execution_id}/approve"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id": "tenant-1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Wait for the execution to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["status"], "succeeded");
}

#[tokio::test]
async fn approval_node_fails_on_reject() {
    let app = router();

    let request_body = json!({
        "tenant_id": "tenant-1",
        "workflow_id": "workflow-1",
        "workflow_version_id": "version-b",
        "graph": {
            "workflow_version_id": "version-b",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "approve", "type": "approval"}
            ],
            "edges": [{"source": "trigger", "target": "approve"}]
        },
        "input_json": "{}"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let execution_id = payload["id"].as_str().unwrap().to_string();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Reject
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/executions/{execution_id}/reject"))
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/executions/{execution_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["status"], "failed");
}

#[tokio::test]
async fn approve_returns_404_when_no_pending_approval() {
    let app = router();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions/no-such-execution/approve")
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn creates_and_triggers_webhook_over_http() {
    let app = router();

    // Create webhook for the dev-seeded version-1
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-versions/version-1/webhook")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id": "tenant-1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let webhook: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = webhook["token"].as_str().unwrap();
    assert!(!token.is_empty());
    assert_eq!(webhook["url"], format!("/v1/webhooks/{token}"));

    // Idempotent: same call returns the same token
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-versions/version-1/webhook")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id": "tenant-1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let webhook2: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(webhook2["token"], webhook["token"]);

    // Trigger the webhook with a JSON body
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/webhooks/{token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"source": "crm", "lead_id": "lead-99"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let execution: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(execution["tenant_id"], "tenant-1");
    assert_eq!(execution["workflow_id"], "workflow-1");
    assert_eq!(execution["status"], "running");

    // Unknown token → 404
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/webhooks/not-a-real-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_invalid_graph_over_http() {
    let app = router();
    let request_body = json!({
        "tenant_id": "tenant-1",
        "workflow_id": "workflow-1",
        "workflow_version_id": "version-1",
        "graph": {
            "workflow_version_id": "version-1",
            "nodes": [
                {"id": "a", "type": "http"},
                {"id": "b", "type": "agent"}
            ],
            "edges": [
                {"source": "a", "target": "b"},
                {"source": "b", "target": "a"}
            ]
        },
        "input_json": "{}"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn creates_lists_and_deletes_credentials_over_http() {
    let app = router();

    // List empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/credentials?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/credentials")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id": "tenant-1", "name": "my-api-key", "value": "sk-secret"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let cred: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(cred["name"], "my-api-key");
    assert!(cred.get("value").is_none(), "value must not be returned");
    let cred_id = cred["id"].as_str().unwrap().to_string();

    // List shows one
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/credentials?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/credentials/{cred_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // List empty again
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/credentials?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn credential_reference_resolved_before_execution() {
    let cred_store = PlatformCredentialStore::default();
    cred_store
        .create("tenant-1", "my-token", "Bearer secret-abc")
        .await
        .unwrap();

    let store = PlatformExecutionStore::memory();
    let gate = Arc::new(ApprovalGate::default());
    let service = ExecutionService::new(
        store.clone(),
        PlatformExecutorClient::inline_with_gate(store, Arc::clone(&gate)),
    );
    let workflow_service =
        WorkflowService::new(PlatformWorkflowVersionStore::memory_with_dev_seed());
    let app = router_with_services(
        service,
        workflow_service,
        PlatformWebhookStore::default(),
        gate,
        cred_store,
    );

    // Start execution with a graph whose node config has a credential reference.
    let request_body = json!({
        "tenant_id": "tenant-1",
        "workflow_id": "workflow-1",
        "workflow_version_id": "version-cred",
        "graph": {
            "workflow_version_id": "version-cred",
            "nodes": [{
                "id": "trigger", "type": "trigger",
                "config": {"auth": "{{credential.my-token}}"}
            }],
            "edges": []
        },
        "input_json": "{}"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // The graph stored in the execution record should have the resolved value.
    assert_eq!(
        payload["graph"]["nodes"][0]["config"]["auth"],
        "Bearer secret-abc"
    );
}

#[tokio::test]
async fn publishing_with_schedule_trigger_registers_schedule() {
    let app = router();

    // Create a new workflow
    let wf_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id":"tenant-1","workspace_id":"ws-1","project_id":"proj-1","name":"Scheduled WF"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    let body = to_bytes(wf_response.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let wf_id = wf["id"].as_str().unwrap();

    // Create a version with interval_secs on the trigger node
    let ver_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "tenant-1",
                            "graph": {
                                "workflow_version_id": "temp",
                                "nodes": [{"id":"trigger","type":"trigger","config":{"interval_secs":3600}}],
                                "edges": []
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    let body = to_bytes(ver_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let ver: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ver_id = ver["id"].as_str().unwrap();

    // Initially no schedules
    let sched_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/schedules?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(sched_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(schedules.as_array().unwrap().len(), 0);

    // Publish the version
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflow-versions/{ver_id}/publish"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"tenant-1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Schedule should now be registered
    let sched_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/schedules?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(sched_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let list = schedules.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["workflow_id"], wf_id);
    assert_eq!(list[0]["interval_secs"], 3600);
}

#[tokio::test]
async fn audit_log_records_execution_started() {
    let app = router();

    let request_body = json!({
        "tenant_id": "tenant-audit",
        "workflow_id": "workflow-1",
        "workflow_version_id": "version-1",
        "graph": {
            "workflow_version_id": "version-1",
            "nodes": [{"id": "trigger", "type": "trigger"}],
            "edges": []
        },
        "input_json": "{}"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/audit-log?tenant_id=tenant-audit")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let events: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let list = events.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["action"], "execution.started");
    assert_eq!(list[0]["tenant_id"], "tenant-audit");
    assert_eq!(list[0]["resource_type"], "execution");
}

#[tokio::test]
async fn exports_workflow_graph_over_http() {
    let app = router();

    // Export the dev-seeded workflow-1 (has published version-1)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows/workflow-1/export?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let export: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(export["name"], "Dev Lead Workflow");
    assert!(export["graph"]["nodes"].as_array().unwrap().len() >= 1);
    assert!(export["exported_at"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn imports_workflow_from_json_over_http() {
    let app = router();

    let body = json!({
        "tenant_id": "tenant-1",
        "workspace_id": "workspace-1",
        "project_id": "project-1",
        "name": "Imported Copy",
        "graph": {
            "workflow_version_id": "ignored-id",
            "nodes": [
                {"id": "trigger", "type": "trigger"},
                {"id": "agent",   "type": "agent"}
            ],
            "edges": [{"source": "trigger", "target": "agent"}]
        }
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/import")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let workflow: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(workflow["name"], "Imported Copy");
    assert_eq!(workflow["status"], "draft");
    // Import creates the workflow together with a draft version and points
    // latest_version_id at it (see import/duplicate latest_version_id fix).
    assert!(workflow["latest_version_id"].is_string());

    // A draft version should have been created with the imported graph
    let wf_id = workflow["id"].as_str().unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/workflows/{wf_id}/versions?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let versions: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let list = versions.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["status"], "draft");
    assert_eq!(list[0]["graph"]["nodes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn archiving_workflow_removes_schedule() {
    let app = router();

    // Use the dev-seeded workflow-1 / version-1 — first add a schedule manually via publish.
    // Create version with schedule
    let ver_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/workflows/workflow-1/versions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "tenant_id": "tenant-1",
                            "graph": {
                                "workflow_version_id": "temp",
                                "nodes": [{"id":"trigger","type":"trigger","config":{"interval_secs":60}}],
                                "edges": []
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    let body = to_bytes(ver_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let ver: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ver_id = ver["id"].as_str().unwrap();

    // Publish to register schedule
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflow-versions/{ver_id}/publish"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"tenant-1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify schedule registered
    let sched_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/schedules?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(sched_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(schedules.as_array().unwrap().len(), 1);

    // Archive the workflow
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/archive")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"tenant-1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Schedule should be gone
    let sched_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/schedules?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(sched_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let schedules: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(schedules.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn retry_execution_creates_new_execution() {
    let store = PlatformExecutionStore::memory();
    let service = ExecutionService::new(store.clone(), PlatformExecutorClient::noop());
    let workflow_service =
        WorkflowService::new(PlatformWorkflowVersionStore::memory_with_dev_seed());
    let gate = Arc::new(ApprovalGate::default());
    let app = router_with_services(
        service,
        workflow_service,
        PlatformWebhookStore::default(),
        gate,
        PlatformCredentialStore::default(),
    );

    // Start original execution
    let exec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "tenant_id": "tenant-1", "input_json": "{\"key\":\"val\"}" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(exec_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let original_id = original["id"].as_str().unwrap().to_string();

    // Retry it
    let retry_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/executions/{original_id}/retry"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "tenant_id": "tenant-1" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry_response.status(), StatusCode::CREATED);
    let bytes = to_bytes(retry_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retried: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // New execution has a different ID
    assert_ne!(retried["id"], original["id"]);
    // Same workflow and input
    assert_eq!(retried["workflow_id"], original["workflow_id"]);
    assert_eq!(retried["input_json"], original["input_json"]);
    assert_eq!(retried["status"], "running");
}

#[tokio::test]
async fn cancel_execution_over_http() {
    // Noop executor leaves execution in Running state so we can cancel it.
    let store = PlatformExecutionStore::memory();
    let service = ExecutionService::new(store.clone(), PlatformExecutorClient::noop());
    let workflow_service =
        WorkflowService::new(PlatformWorkflowVersionStore::memory_with_dev_seed());
    let gate = Arc::new(ApprovalGate::default());
    let app = router_with_services(
        service,
        workflow_service,
        PlatformWebhookStore::default(),
        gate,
        PlatformCredentialStore::default(),
    );

    // Start an execution
    let exec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows/workflow-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "tenant_id": "tenant-1", "input_json": "{}" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(exec_response.status().is_success());
    let bytes = to_bytes(exec_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let exec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let exec_id = exec["id"].as_str().unwrap().to_string();

    // Cancel it
    let cancel_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/executions/{exec_id}/cancel"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "tenant_id": "tenant-1" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_response.status(), StatusCode::OK);

    // Verify status is cancelled
    let get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/executions/{exec_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["status"], "cancelled");
    assert!(updated["finished_at"].is_number());
}

#[tokio::test]
async fn duplicate_workflow_creates_copy() {
    let app = router();

    // Create a workflow to duplicate
    let create_body = json!({
        "tenant_id": "tenant-1",
        "workspace_id": "workspace-1",
        "project_id": "project-1",
        "name": "Original Workflow"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(create_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let original: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let original_id = original["id"].as_str().unwrap().to_string();

    // Duplicate it
    let dup_body = json!({ "tenant_id": "tenant-1" });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{original_id}/duplicate"))
                .header("content-type", "application/json")
                .body(Body::from(dup_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let copy: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_ne!(copy["id"], original["id"]);
    assert_eq!(copy["name"], "Original Workflow (copy)");
    assert_eq!(copy["status"], "draft");
    assert_eq!(copy["workspace_id"], original["workspace_id"]);
    assert_eq!(copy["project_id"], original["project_id"]);
}

#[tokio::test]
async fn create_token_returns_jwt_for_valid_key() {
    // Uses default DEV_API_KEY = "dev"
    let app = router();
    let body = serde_json::json!({ "api_key": "dev" });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/token")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(resp["token"]
        .as_str()
        .map(|t| t.len() > 20)
        .unwrap_or(false));
    assert_eq!(resp["tenant_id"], "tenant-1");
    assert_eq!(resp["workspace_id"], "workspace-1");
}

#[tokio::test]
async fn create_token_rejects_wrong_key() {
    // Uses default DEV_API_KEY = "dev"; submits wrong key
    let app = router();
    let body = serde_json::json!({ "api_key": "definitely-not-dev" });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/token")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn effective_tenant_id_jwt_always_wins() {
    // JWT present → JWT tenant_id wins regardless of auth_required flag.
    let claims_a = Some(Claims {
        sub: "user".to_string(),
        tenant_id: "tenant-a".to_string(),
        workspace_id: "ws".to_string(),
        project_id: "proj".to_string(),
        exp: u64::MAX,
        role: crate::auth::Role::Editor,
        ..Default::default()
    });
    // Even in dev mode (auth_required=false), JWT tenant cannot be spoofed.
    assert_eq!(
        effective_tenant_id_with_flag(false, &claims_a, "tenant-b"),
        "tenant-a"
    );
    // No JWT → fall back to supplied (dev mode only).
    assert_eq!(
        effective_tenant_id_with_flag(false, &None, "tenant-b"),
        "tenant-b"
    );
}

#[test]
fn effective_tenant_id_auth_mode_uses_jwt_tenant() {
    // auth_required=true + claims → JWT tenant overrides supplied value
    let claims_a = Some(Claims {
        sub: "user".to_string(),
        tenant_id: "tenant-a".to_string(),
        workspace_id: "ws".to_string(),
        project_id: "proj".to_string(),
        exp: u64::MAX,
        role: crate::auth::Role::Editor,
        ..Default::default()
    });
    // Claims present → JWT tenant wins
    assert_eq!(
        effective_tenant_id_with_flag(true, &claims_a, "tenant-b"),
        "tenant-a",
        "JWT tenant should override supplied value"
    );
    // No claims → fallback to supplied
    assert_eq!(
        effective_tenant_id_with_flag(true, &None, "tenant-b"),
        "tenant-b",
        "No claims → use supplied fallback"
    );
}

#[tokio::test]
async fn jwt_claims_injected_into_extensions() {
    // Verify that a valid Bearer token causes claims to be available in extensions,
    // and that GET /v1/executions succeeds with a JWT (dev mode, no AUTH_REQUIRED).
    use crate::auth::{sign_token, Claims};

    let app = router();

    let claims = Claims {
        sub: "user".to_string(),
        tenant_id: "tenant-a".to_string(),
        workspace_id: "ws".to_string(),
        project_id: "proj".to_string(),
        exp: u64::MAX,
        role: crate::auth::Role::Editor,
        ..Default::default()
    };
    let token = sign_token(&claims).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/executions?tenant_id=tenant-a")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // Tenant-A has no executions in fresh router → empty array
    assert!(payload.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn workspace_crud_over_http() {
    let app = router();

    // Create workspace
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "tenant_id": "tenant-1", "name": "Engineering" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let ws: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let ws_id = ws["id"].as_str().unwrap().to_string();
    assert_eq!(ws["name"], "Engineering");

    // List workspaces
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workspaces?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list.as_array().unwrap().iter().any(|w| w["id"] == ws_id));

    // Create project in workspace
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{ws_id}/projects"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "tenant_id": "tenant-1", "name": "Backend" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let proj: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let proj_id = proj["id"].as_str().unwrap().to_string();
    assert_eq!(proj["name"], "Backend");

    // List projects
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/v1/workspaces/{ws_id}/projects?tenant_id=tenant-1"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let projects: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(projects.as_array().unwrap().len(), 1);
    assert_eq!(projects[0]["id"], proj_id);

    // Delete project
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/projects/{proj_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Delete workspace
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/workspaces/{ws_id}?tenant_id=tenant-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Workspace gone
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workspaces?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!list.as_array().unwrap().iter().any(|w| w["id"] == ws_id));
}

#[tokio::test]
async fn system_info_returns_expected_fields() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/system/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(info["version"].is_string());
    assert!(info["node_types"].as_u64().unwrap() > 0);
    assert!(info["features"].is_array());
    assert!(info["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f.as_str() == Some("jwt-auth")));
}

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_text() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/plain"));
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    assert!(body.contains("af_executions_started_total"));
    assert!(body.contains("af_http_requests_total"));
}

#[tokio::test]
async fn search_returns_matching_workflows() {
    let app = router();
    // First create a workflow
    let body = serde_json::json!({ "name": "SearchTargetWorkflow", "workspace_id": "ws-1", "project_id": "project-1", "tenant_id": "t1" });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Search for it
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/search?q=SearchTarget&tenant_id=t1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(result["workflows"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false));
}

#[tokio::test]
async fn search_empty_query_returns_all() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/search?q=&tenant_id=t1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let result: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(result["workflows"].is_array());
    assert!(result["executions"].is_array());
}

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

#[test]
fn rbac_viewer_blocked_from_write() {
    use crate::auth::{Claims, Role};
    let viewer = Some(Claims {
        sub: "u".to_string(),
        tenant_id: "t".to_string(),
        workspace_id: "w".to_string(),
        project_id: "p".to_string(),
        exp: u64::MAX,
        role: Role::Viewer,
        ..Default::default()
    });
    // dev mode: always allowed
    assert!(require_write_inner(&viewer, false).is_ok());
    // enforced: viewer blocked
    assert!(require_write_inner(&viewer, true).is_err());
}

#[test]
fn rbac_editor_allowed_to_write() {
    use crate::auth::{Claims, Role};
    let editor = Some(Claims {
        sub: "u".to_string(),
        tenant_id: "t".to_string(),
        workspace_id: "w".to_string(),
        project_id: "p".to_string(),
        exp: u64::MAX,
        role: Role::Editor,
        ..Default::default()
    });
    assert!(require_write_inner(&editor, true).is_ok());
}

#[test]
fn rbac_no_claims_blocked_when_enforced() {
    assert!(require_write_inner(&None, true).is_err());
    assert!(require_admin_inner(&None, true).is_err());
    // dev mode: always allowed even without claims
    assert!(require_write_inner(&None, false).is_ok());
    assert!(require_admin_inner(&None, false).is_ok());
}

#[test]
fn rbac_editor_cannot_perform_admin_ops() {
    use crate::auth::{Claims, Role};
    let editor = Some(Claims {
        sub: "u".to_string(),
        tenant_id: "t".to_string(),
        workspace_id: "w".to_string(),
        project_id: "p".to_string(),
        exp: u64::MAX,
        role: Role::Editor,
        ..Default::default()
    });
    assert!(require_admin_inner(&editor, true).is_err());
}

#[test]
fn rbac_admin_can_perform_all_ops() {
    use crate::auth::{Claims, Role};
    let admin = Some(Claims {
        sub: "u".to_string(),
        tenant_id: "t".to_string(),
        workspace_id: "w".to_string(),
        project_id: "p".to_string(),
        exp: u64::MAX,
        role: Role::Admin,
        ..Default::default()
    });
    assert!(require_write_inner(&admin, true).is_ok());
    assert!(require_admin_inner(&admin, true).is_ok());
}

// ── Form store HTTP tests ───────────────────────────────────────────────

#[tokio::test]
async fn form_publish_and_get() {
    let app = router();

    // Create a workflow first
    let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"FormWf"}).to_string())).unwrap()
        ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();

    // Publish a form
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/publish-form"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id":"t1","title":"My Form"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let form: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = form["token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());

    // Get the form by token
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/forms/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let got: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got["title"].as_str().unwrap(), "My Form");
    assert_eq!(got["workflow_id"].as_str().unwrap(), wf_id);
}

#[tokio::test]
async fn form_list_and_delete() {
    let app = router();

    let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"FormWf2"}).to_string())).unwrap()
        ).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();

    // Publish two forms
    for title in ["Form A", "Form B"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workflows/{wf_id}/publish-form"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tenant_id":"t1","title":title}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // List forms for the workflow
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/workflows/{wf_id}/forms?tenant_id=t1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let forms: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(forms.as_array().unwrap().len(), 2);

    // Get token of first form
    let token = forms[0]["token"].as_str().unwrap().to_string();

    // Delete that form
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/forms/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Confirm it's gone
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/forms/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Test case HTTP tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_case_create_list_update_delete() {
    let app = router();

    let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"TcWf"}).to_string())).unwrap()
        ).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();

    // Create test case
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/test-cases"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tenant_id": "t1",
                        "name": "TC1",
                        "input_json": r#"{"x":1}"#,
                        "expected_output": r#"{"result":2}"#
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let tc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let tc_id = tc["id"].as_str().unwrap().to_string();
    assert_eq!(tc["name"].as_str().unwrap(), "TC1");

    // List test cases
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/workflows/{wf_id}/test-cases?tenant_id=t1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let tcs: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(tcs.as_array().unwrap().len(), 1);

    // Get test case
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/test-cases/{tc_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Update test case
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/test-cases/{tc_id}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "TC1 Updated"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["name"].as_str().unwrap(), "TC1 Updated");

    // Delete test case
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/test-cases/{tc_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Confirm deletion
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/test-cases/{tc_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Comment HTTP tests ──────────────────────────────────────────────────

#[tokio::test]
async fn workflow_comments_crud() {
    let app = router();

    // Create a workflow first
    let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"CommentWf"}).to_string())).unwrap()
        ).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();

    // Create comment
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/comments"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id":"t1","author":"alice","body":"Hello world"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let comment: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let comment_id = comment["id"].as_str().unwrap().to_string();
    assert_eq!(comment["author"].as_str().unwrap(), "alice");
    assert_eq!(comment["body"].as_str().unwrap(), "Hello world");
    assert!(comment["edited_at"].is_null());

    // List comments
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/workflows/{wf_id}/comments?tenant_id=t1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let comments: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(comments.as_array().unwrap().len(), 1);

    // Edit comment
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/comments/{comment_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id":"t1","body":"Updated body"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let edited: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(edited["body"].as_str().unwrap(), "Updated body");
    assert!(!edited["edited_at"].is_null());

    // Delete comment
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/comments/{comment_id}?tenant_id=t1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // List should now be empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/workflows/{wf_id}/comments?tenant_id=t1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let comments: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(comments.as_array().unwrap().is_empty());
}

// ── Workflow locking HTTP tests ─────────────────────────────────────────

#[tokio::test]
async fn workflow_lock_blocks_version_save() {
    let app = router();

    // Create workflow
    let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","workspace_id":"ws1","project_id":"proj1","name":"LockWf"}).to_string())).unwrap()
        ).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();
    assert!(!wf["locked"].as_bool().unwrap_or(false));

    // Lock it
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/lock"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let locked_wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(locked_wf["locked"].as_bool(), Some(true));

    let min_graph = json!({
        "workflow_version_id": "v1",
        "nodes": [{"id": "trigger-1", "type": "trigger"}],
        "edges": [],
        "input_schema": []
    });

    // Attempt to save a version — must be rejected (workflow is locked)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/versions"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id": "t1", "graph": min_graph}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Unlock it
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/unlock"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Now save should succeed
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/versions"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id": "t1", "graph": min_graph}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn event_subscriptions_crud() {
    let app = router();

    // List — initially empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/event-subscriptions?tenant_id=t1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);

    // Create
    let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/event-subscriptions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tenant_id":"t1","url":"https://example.com/hook","events":["execution.started"],"description":"test hook"}).to_string())).unwrap()
        ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let sub: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let sub_id = sub["id"].as_str().unwrap().to_string();
    assert_eq!(sub["url"].as_str(), Some("https://example.com/hook"));
    assert_eq!(sub["description"].as_str(), Some("test hook"));
    assert_eq!(sub["events"].as_array().unwrap().len(), 1);

    // List — now has one
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/event-subscriptions?tenant_id=t1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Delete
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/event-subscriptions/{sub_id}?tenant_id=t1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // List — empty again
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/event-subscriptions?tenant_id=t1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn openapi_json_returns_valid_spec() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let spec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(spec["openapi"], "3.0.3");
    assert!(spec["paths"].as_object().unwrap().len() > 20);
}

#[tokio::test]
async fn openapi_docs_returns_html() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/docs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("swagger-ui"));
    assert!(body.contains("/openapi.json"));
}

// ── User auth HTTP tests ────────────────────────────────────────────────

#[tokio::test]
async fn register_and_login_user() {
    let app = router();

    // Register
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                        "password": "hunter2",
                        "name": "Alice"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["token"].as_str().is_some());
    assert_eq!(body["user"]["email"], "alice@example.com");
    assert_eq!(body["user"]["name"], "Alice");

    // Login
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "alice@example.com",
                        "password": "hunter2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["token"].as_str().is_some());
}

#[tokio::test]
async fn register_duplicate_email_returns_conflict() {
    let app = router();

    for _ in 0..2 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "email": "dup@example.com",
                            "password": "secret"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "dup@example.com",
                        "password": "secret"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_wrong_password_returns_unauthorized() {
    let app = router();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "bob@example.com",
                        "password": "correct"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "bob@example.com",
                        "password": "wrong"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_handler_returns_user_for_user_jwt() {
    let app = router();

    // Register to get a user token
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "email": "carol@example.com",
                        "password": "pass123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = reg["token"].as_str().unwrap().to_string();

    // Use the token to call /me
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let me: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(me["email"], "carol@example.com");
}

#[tokio::test]
async fn update_me_name() {
    let app = router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "upme1@example.com", "password": "pass123"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = reg["token"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/auth/me")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(json!({"name": "Updated Name"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let me: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(me["name"], "Updated Name");
}

#[tokio::test]
async fn update_me_password_success() {
    let app = router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "upme2@example.com", "password": "oldpass"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = reg["token"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/auth/me")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({"current_password": "oldpass", "new_password": "newpass"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify login works with the new password
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "upme2@example.com", "password": "newpass"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn update_me_wrong_current_password_returns_401() {
    let app = router();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "upme3@example.com", "password": "rightpass"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = reg["token"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/auth/me")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({"current_password": "wrongpass", "new_password": "newpass"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_list_users() {
    let app = router();

    // Register two users
    for email in &["adm1@example.com", "adm2@example.com"] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": "pw"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // List users (no auth required in test mode)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/admin/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let users: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(users.as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn admin_delete_user_success() {
    let app = router();

    // Register the user to delete
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "del_target@example.com", "password": "pw"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let target_id = reg["user"]["id"].as_str().unwrap().to_string();

    // Register admin caller (different user)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "del_admin@example.com", "password": "pw"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let admin_reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let admin_token = admin_reg["token"].as_str().unwrap().to_string();

    // Delete the target user
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/admin/users/{target_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn admin_cannot_delete_self() {
    let app = router();

    // Register a user
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "del_self@example.com", "password": "pw"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let reg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = reg["token"].as_str().unwrap().to_string();
    let user_id = reg["user"]["id"].as_str().unwrap().to_string();

    // Attempt to delete own account
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/admin/users/{user_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn invite_flow_create_and_accept() {
    let app = router();

    // Create an invitation (admin; no AUTH_REQUIRED so passes through)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/admin/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "invited@example.com", "role": "editor"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = inv["token"].as_str().unwrap().to_string();

    // Look up the invite
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/invitations/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(info["email"], "invited@example.com");
    assert_eq!(info["valid"], true);

    // Accept the invite
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/accept-invite")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"token": token, "password": "securepass", "name": "Invited User"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let auth: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(auth["user"]["email"], "invited@example.com");
    assert!(auth["token"]
        .as_str()
        .map(|t| t.len() > 10)
        .unwrap_or(false));
}

#[tokio::test]
async fn invite_cannot_be_used_twice() {
    let app = router();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/admin/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "twice@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = inv["token"].as_str().unwrap().to_string();

    // Use it once
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/accept-invite")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"token": &token, "password": "pw1"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Second attempt — should return GONE
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/accept-invite")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"token": &token, "password": "pw2"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::GONE);
}

#[tokio::test]
async fn invite_list_and_revoke() {
    let app = router();

    // Create an invite
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/admin/invitations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "revoke_me@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let inv: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let invite_id = inv["id"].as_str().unwrap().to_string();

    // List — should contain the invite
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/admin/invitations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == invite_id));

    // Revoke it
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/admin/invitations/{invite_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// ── Org management HTTP tests ───────────────────────────────────────────

async fn register_and_get_token(app: axum::Router, email: &str) -> String {
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "pw1234"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    body["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_and_list_org() {
    let app = router();
    let token = register_and_get_token(app.clone(), "owner@org.test").await;

    // Create org
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/orgs")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(json!({"name": "Acme Corp"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let org: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(org["name"], "Acme Corp");

    // List orgs
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/orgs")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let orgs: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(orgs.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn add_member_and_switch_org() {
    let app = router();
    let owner_token = register_and_get_token(app.clone(), "owner2@org.test").await;

    // Register second user
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "member@org.test", "password": "pw1234"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let member_data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let member_id = member_data["user"]["id"].as_str().unwrap().to_string();
    let member_token = member_data["token"].as_str().unwrap().to_string();

    // Create org
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/orgs")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {owner_token}"))
                .body(Body::from(json!({"name": "Beta Inc"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let org: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let org_id = org["id"].as_str().unwrap().to_string();

    // Add member
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/orgs/{org_id}/members"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {owner_token}"))
                .body(Body::from(
                    json!({"user_id": member_id, "role": "editor"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Switch org as member
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/orgs/{org_id}/switch"))
                .header("authorization", format!("Bearer {member_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let switched: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(switched["token"].as_str().is_some());
    assert_eq!(switched["org_id"], org_id);
}

#[tokio::test]
async fn non_member_cannot_switch_org() {
    let app = router();
    let owner_token = register_and_get_token(app.clone(), "owner3@org.test").await;
    let stranger_token = register_and_get_token(app.clone(), "stranger@org.test").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/orgs")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {owner_token}"))
                .body(Body::from(json!({"name": "Gamma LLC"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let org: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let org_id = org["id"].as_str().unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/orgs/{org_id}/switch"))
                .header("authorization", format!("Bearer {stranger_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn workflow_visibility_set_and_filter() {
    let app = router();

    // Create a workflow via the API
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tenant_id": "vis-tenant",
                        "workspace_id": "ws-1",
                        "project_id": "proj-1",
                        "name": "Private WF"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();
    assert_eq!(wf["visibility"], "tenant");

    // Set visibility to private
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/workflows/{wf_id}/visibility"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id": "vis-tenant", "visibility": "private"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["visibility"], "private");

    // List should still include it (no auth in test mode — created_by is None, caller is None)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/workflows?tenant_id=vis-tenant")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let found = list.as_array().unwrap().iter().any(|w| w["id"] == wf_id);
    assert!(
        found,
        "workflow with visibility=private still visible when created_by matches caller (both None)"
    );

    // Set back to tenant
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/workflows/{wf_id}/visibility"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id": "vis-tenant", "visibility": "tenant"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let restored: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored["visibility"], "tenant");
}

#[tokio::test]
async fn set_visibility_rejects_invalid_value() {
    let app = router();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tenant_id": "vis2-tenant",
                        "workspace_id": "ws-1",
                        "project_id": "proj-1",
                        "name": "WF2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/workflows/{wf_id}/visibility"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tenant_id": "vis2-tenant", "visibility": "public"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn forgot_password_always_returns_ok() {
    let app = router();

    // Unknown email still returns 200 (no enumeration)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/forgot-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "nobody@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["message"].as_str().is_some());
}

#[tokio::test]
async fn password_reset_full_flow() {
    let app = router();

    // Register a user
    let email = "resetme@test.com";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "oldpassword"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Request a reset
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/forgot-password")
                .header("content-type", "application/json")
                .body(Body::from(json!({"email": email}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // In dev mode (AUTH_REQUIRED not set) the token is returned
    let token = body["token"]
        .as_str()
        .expect("token returned in dev mode")
        .to_string();

    // Reset with new password
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/reset-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"token": token, "new_password": "newpassword123"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Login with new password succeeds
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "newpassword123"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Old password no longer works
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "oldpassword"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Token cannot be reused
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/reset-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"token": token, "new_password": "anotherpassword"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::GONE);
}

#[tokio::test]
async fn reset_password_rejects_short_password() {
    let app = router();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/reset-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"token": "fake-token", "new_password": "abc"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn email_verification_full_flow() {
    let app = router();

    // Register a user — email_verified should be false initially
    let email = "verify@test.com";
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": email, "password": "password123"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["user"]["email_verified"].as_bool(), Some(false));

    // Resend verification always returns 200 (enumeration safe)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/resend-verification")
                .header("content-type", "application/json")
                .body(Body::from(json!({"email": email}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify with invalid token returns 404
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/verify-email")
                .header("content-type", "application/json")
                .body(Body::from(json!({"token": "no-such-token"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn notification_prefs_get_and_update() {
    let app = router();
    let token = register_and_get_token(app.clone(), "notif@test.com").await;

    // Default prefs: both false
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/me/notifications")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["email_on_failure"].as_bool(), Some(false));
    assert_eq!(body["email_on_success"].as_bool(), Some(false));

    // Update prefs
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/auth/me/notifications")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email_on_failure": true, "email_on_success": false}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify persisted
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/me/notifications")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["email_on_failure"].as_bool(), Some(true));
    assert_eq!(body["email_on_success"].as_bool(), Some(false));
}

#[tokio::test]
async fn drain_mode_rejects_new_executions() {
    // Reset drain flag before test (other parallel tests must not interfere)
    super::DRAINING.store(false, std::sync::atomic::Ordering::SeqCst);

    let app = router();
    let body = serde_json::json!({
        "tenant_id": "tenant-drain",
        "workflow_id": "wf-1",
        "workflow_version_id": "v-1",
        "graph": {"workflow_version_id": "v-1", "nodes": [{"id": "t", "type": "trigger"}], "edges": []},
        "input_json": "{}"
    });

    // Should succeed before drain
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Activate drain mode
    super::DRAINING.store(true, std::sync::atomic::Ordering::SeqCst);

    // Should now return 503
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    // Also blocks start-from-workflow-version
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-versions/v-1/executions")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"tenant_id": "tenant-drain", "input_json": "{}"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    // Reset for subsequent tests
    super::DRAINING.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[tokio::test]
async fn quota_exceeded_returns_402() {
    use crate::billing::{BillingStore, TenantQuota};
    use std::sync::Arc;

    // Build a state where billing quota is exhausted
    let state = super::default_app_state();
    // Set a quota of 0 executions for the test tenant
    let zero_quota = TenantQuota {
        tenant_id: "tenant-quota".to_string(),
        tier: "free".to_string(),
        max_executions_per_month: 0,
        max_concurrent_executions: 10,
        max_workflows: 50,
    };
    state.billing_store.set_quota(zero_quota);

    let app = super::build_router(state);
    let body = json!({
        "tenant_id": "tenant-quota",
        "workflow_id": "wf-1",
        "workflow_version_id": "v-1",
        "graph": {"workflow_version_id": "v-1", "nodes": [{"id": "t", "type": "trigger"}], "edges": []},
        "input_json": "{}"
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/executions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn billing_status_endpoint_returns_ok() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/billing/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.get("quota").is_some());
    assert!(v.get("usage").is_some());
    assert!(v.get("usage_pct").is_some());
}

// ── Slice 358: Webhook replay protection ──────────────────────────────────

#[tokio::test]
async fn webhook_replay_protection_rejects_stale_timestamp() {
    use crate::webhook::{WebhookRecord, WebhookStore};

    // Build state and insert a webhook record directly with a secret
    let state = super::default_app_state();
    let token = "replay-token-secret".to_string();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: token.clone(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: Some("replay-secret".to_string()),
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();

    let app = super::build_router(state);

    // Trigger with stale timestamp (epoch 0 far outside ±300s) → expect 400
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/webhooks/{token}"))
                .header("content-type", "application/json")
                .header("x-trigix-timestamp", "0")
                .header("x-webhook-signature", "invalid")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn webhook_without_secret_skips_replay_check() {
    use crate::webhook::{WebhookRecord, WebhookStore};

    // Insert a webhook record WITHOUT a secret — replay check must not fire
    let state = super::default_app_state();
    let token = "replay-token-nosecret".to_string();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: token.clone(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: None,
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();

    let app = super::build_router(state);

    // No timestamp header, no secret → must NOT return 400 (replay check skipped)
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/webhooks/{token}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── Slice 359: Workflow version rollback ──────────────────────────────────

#[tokio::test]
async fn rollback_creates_new_draft_version() {
    let app = router();
    // Create a workflow
    let create_body = json!({
        "tenant_id": "t-rollback",
        "workspace_id": "ws-1",
        "project_id": "proj-1",
        "name": "Rollback Test Workflow"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflows")
                .header("content-type", "application/json")
                .body(Body::from(create_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let wf: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let wf_id = wf["id"].as_str().unwrap();

    // Create a version
    let version_body = json!({
        "tenant_id": "t-rollback",
        "graph": { "workflow_version_id": "v-rb", "nodes": [{"id": "n1", "type": "trigger"}], "edges": [] },
        "status": "draft",
        "message": "initial"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workflows/{wf_id}/versions"))
                .header("content-type", "application/json")
                .body(Body::from(version_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let version: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let version_id = version["id"].as_str().unwrap();

    // Rollback to that version
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/workflows/{wf_id}/rollback/{version_id}?tenant_id=t-rollback"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rolled: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // New version should be a draft with a rollback message
    assert_eq!(rolled["status"].as_str().unwrap(), "draft");
    let msg = rolled["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("Rollback") || msg.contains("ollback"),
        "message should mention rollback: {msg}"
    );
}

// ── Slice 360: MCP server ─────────────────────────────────────────────────

#[tokio::test]
async fn mcp_manifest_returns_json() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/.well-known/mcp.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|a| a.len() >= 2)
        .unwrap_or(false));
}

#[tokio::test]
async fn mcp_list_workflows_tool() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp/tools")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool": "list_workflows",
                        "tenant_id": "tenant-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.get("workflows").and_then(|w| w.as_array()).is_some());
}

#[tokio::test]
async fn mcp_unknown_tool_returns_400() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp/tools")
                .header("content-type", "application/json")
                .body(Body::from(json!({"tool": "nonexistent_tool"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn request_id_header_present_on_response() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        resp.headers().contains_key("x-request-id"),
        "x-request-id header missing from response"
    );
    let id = resp
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(id.len(), 36, "request ID should be a 36-char UUID");
}

#[tokio::test]
async fn security_headers_present_on_response() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let headers = resp.headers();
    assert_eq!(
        headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
    assert_eq!(
        headers
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert!(
        headers.contains_key("strict-transport-security"),
        "missing HSTS header"
    );
    assert!(
        headers.contains_key("referrer-policy"),
        "missing referrer-policy header"
    );
}

#[tokio::test]
async fn request_ids_are_unique_across_requests() {
    let r1 = router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let r2 = router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let id1 = r1
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let id2 = r2
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(id1, id2, "each request should get a unique request ID");
}

#[tokio::test]
async fn webhook_delivery_recorded_on_trigger_success() {
    use crate::webhook::{WebhookRecord, WebhookStore};
    let state = default_app_state();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: "test-delivery-token".to_string(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: None,
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();
    let app = build_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/webhooks/test-delivery-token")
                .header("content-type", "application/json")
                .body(Body::from(json!({"x": 1}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let deliveries = state
        .webhook_store
        .list_deliveries("test-delivery-token", 10)
        .await;
    assert_eq!(deliveries.len(), 1);
    assert!(deliveries[0].success);
    assert_eq!(deliveries[0].status_code, Some(202));
    assert!(deliveries[0].execution_id.is_some());
}

#[tokio::test]
async fn webhook_delivery_recorded_on_quota_failure() {
    use crate::billing::{BillingStore, TenantQuota};
    use crate::webhook::{WebhookRecord, WebhookStore};
    let state = default_app_state();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: "test-quota-tok".to_string(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: None,
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();
    state.billing_store.set_quota(TenantQuota {
        tenant_id: "tenant-1".to_string(),
        tier: "free".to_string(),
        max_executions_per_month: 0,
        max_concurrent_executions: 10,
        max_workflows: 100,
    });
    let app = build_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/webhooks/test-quota-tok")
                .header("content-type", "application/json")
                .body(Body::from("{}".to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    let deliveries = state
        .webhook_store
        .list_deliveries("test-quota-tok", 10)
        .await;
    assert_eq!(deliveries.len(), 1);
    assert!(!deliveries[0].success);
    assert_eq!(deliveries[0].status_code, Some(402));
    assert!(deliveries[0].error_message.is_some());
}

#[tokio::test]
async fn list_webhook_deliveries_endpoint() {
    use crate::webhook::{WebhookDelivery, WebhookRecord, WebhookStore};
    let state = default_app_state();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: "tok-list-del".to_string(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: None,
            condition_expr: None,
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();
    // Pre-populate a delivery
    state
        .webhook_store
        .record_delivery(WebhookDelivery {
            id: "del-1".to_string(),
            webhook_token: "tok-list-del".to_string(),
            tenant_id: "tenant-1".to_string(),
            delivered_at: 1_700_000_000,
            status_code: Some(202),
            success: true,
            error_message: None,
            execution_id: Some("exec-1".to_string()),
        })
        .await;
    let app = build_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/webhooks/tok-list-del/deliveries")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["success"], true);
}

#[tokio::test]
async fn run_test_case_returns_404_for_unknown_id() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/test-cases/nonexistent-tc-id/run")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Slice 392: Notification center ────────────────────────────────────────

#[tokio::test]
async fn notifications_list_and_mark_read() {
    use crate::notifications::NotificationStore;
    let state = default_app_state();
    state.notification_store.create(
        "tenant-1",
        None,
        "Test alert",
        "Something happened",
        "warning",
    );
    let app = build_router(state);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/notifications?tenant_id=tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["unread_count"], 1);
    assert_eq!(body["notifications"].as_array().unwrap().len(), 1);
    let notif_id = body["notifications"][0]["id"].as_str().unwrap().to_string();

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/notifications/{notif_id}/read"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
}

// ── Slice 393: Webhook condition filter ────────────────────────────────────

#[tokio::test]
async fn webhook_condition_blocks_non_matching_payload() {
    use crate::webhook::{WebhookRecord, WebhookStore};
    let state = default_app_state();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: "cond-filter-tok".to_string(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: None,
            condition_expr: Some("event == \"purchase\"".to_string()),
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();
    let app = build_router(state);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/webhooks/cond-filter-tok")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event":"login"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .starts_with("filtered:"));
}

#[tokio::test]
async fn webhook_condition_passes_matching_payload() {
    use crate::webhook::{WebhookRecord, WebhookStore};
    let state = default_app_state();
    state
        .webhook_store
        .upsert(WebhookRecord {
            token: "cond-match-tok".to_string(),
            tenant_id: "tenant-1".to_string(),
            workflow_id: "workflow-1".to_string(),
            workflow_version_id: "version-1".to_string(),
            secret: None,
            condition_expr: Some("event == \"purchase\"".to_string()),
            max_calls_per_minute: None,
            paused: false,
            payload_transform_script: None,
        })
        .await
        .unwrap();
    let app = build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/webhooks/cond-match-tok")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event":"purchase","amount":99.99}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["id"].is_string());
}

#[test]
fn eval_condition_unit_tests() {
    use crate::webhook::eval_condition;
    let payload =
        serde_json::json!({ "event": "purchase", "amount": 150.0, "nested": { "x": true } });
    assert!(eval_condition("event == \"purchase\"", &payload));
    assert!(!eval_condition("event == \"login\"", &payload));
    assert!(eval_condition("event != \"login\"", &payload));
    assert!(eval_condition("amount > 100", &payload));
    assert!(!eval_condition("amount > 200", &payload));
    assert!(eval_condition("amount < 200", &payload));
    assert!(eval_condition("nested.x == true", &payload));
    assert!(!eval_condition("nested.y == \"foo\"", &payload));
    assert!(eval_condition("", &payload));
}

// ── Slice 395: Credential expiry ───────────────────────────────────────────

#[tokio::test]
async fn credential_update_and_expiry() {
    use crate::credentials::CredentialStore;
    let state = default_app_state();
    let cred = state
        .credential_store
        .create("tenant-1", "my-key", "secret-value")
        .await
        .unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expires_soon = now + 3 * 86400;
    let updated = state
        .credential_store
        .update(
            "tenant-1",
            &cred.id,
            Some("new-secret"),
            Some(Some("A description")),
            Some(Some(expires_soon)),
        )
        .await
        .unwrap();
    assert_eq!(updated.description.as_deref(), Some("A description"));
    assert_eq!(updated.expires_at, Some(expires_soon));

    let expiring = state
        .credential_store
        .list_expiring("tenant-1", now + 7 * 86400)
        .await
        .unwrap();
    assert_eq!(expiring.len(), 1);
    assert_eq!(expiring[0].id, cred.id);

    let not_expiring = state
        .credential_store
        .list_expiring("tenant-1", now + 86400)
        .await
        .unwrap();
    assert_eq!(not_expiring.len(), 0);
}

#[tokio::test]
async fn credential_update_endpoint_returns_200() {
    let app = build_router(default_app_state());
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/credentials")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tenant_id": "tenant-1",
                        "name": "update-test-key",
                        "value": "initial"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let cred_id = body["id"].as_str().unwrap().to_string();

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/credentials/{cred_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tenant_id": "tenant-1",
                        "description": "Updated key",
                        "expires_at": 1999999999u64
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let bytes2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
    let body2: serde_json::Value = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(body2["description"].as_str().unwrap(), "Updated key");
    assert_eq!(body2["expires_at"].as_u64().unwrap(), 1999999999u64);
}

#[tokio::test]
async fn list_expiring_credentials_endpoint() {
    use crate::credentials::CredentialStore;
    let state = default_app_state();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let c1 = state
        .credential_store
        .create("tenant-1", "soon-key-exp", "v")
        .await
        .unwrap();
    state
        .credential_store
        .update("tenant-1", &c1.id, None, None, Some(Some(now + 5 * 86400)))
        .await
        .unwrap();
    let c2 = state
        .credential_store
        .create("tenant-1", "far-key-exp", "v")
        .await
        .unwrap();
    state
        .credential_store
        .update(
            "tenant-1",
            &c2.id,
            None,
            None,
            Some(Some(now + 365 * 86400)),
        )
        .await
        .unwrap();

    let app = build_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/credentials/expiring?tenant_id=tenant-1&within_days=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["name"].as_str().unwrap(), "soon-key-exp");
}

#[tokio::test]
async fn sso_connection_crud_and_public_list() {
    let app = router();

    // Create a connection (dev mode: require_admin passes without a token).
    let body = json!({
        "slug": "Acme-Okta",
        "provider": "Okta",
        "issuer": "https://acme.okta.com/",
        "client_id": "client-123",
        "client_secret": "super-secret-value",
        "scopes": "openid email profile"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sso-connections")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // slug is normalized to lowercase; issuer trailing slash trimmed.
    assert_eq!(created["slug"], "acme-okta");
    assert_eq!(created["issuer"], "https://acme.okta.com");
    // The client secret must never be serialized back.
    assert!(!String::from_utf8_lossy(&bytes).contains("super-secret-value"));

    // Admin list shows it, still without the secret.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sso-connections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("super-secret-value"));

    // Public list exposes only slug + provider.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sso/public")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let pubs: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(pubs.len(), 1);
    assert_eq!(pubs[0]["slug"], "acme-okta");
    assert_eq!(pubs[0]["provider"], "Okta");
    assert!(pubs[0].get("client_id").is_none());
}

#[tokio::test]
async fn sso_create_rejects_bad_slug() {
    let app = router();
    let body = json!({
        "slug": "bad slug!",
        "provider": "Okta",
        "issuer": "https://x",
        "client_id": "c",
        "client_secret": "s"
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sso-connections")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sso_disable_hides_from_public_and_rejects_login() {
    let app = router();
    let body = json!({
        "slug": "togg-okta", "provider": "Okta", "kind": "oidc",
        "issuer": "https://x.okta.com", "client_id": "c", "client_secret": "s"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/sso-connections")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = created["id"].as_str().unwrap();

    // Disable it.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/sso-connections/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"enabled": false}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Public list no longer includes it.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sso/public")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let pubs: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(!pubs.iter().any(|p| p["slug"] == "togg-okta"));

    // Login is rejected (redirects to the SPA with an error).
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sso/togg-okta/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FOUND);
    let loc = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("sso_error"));
}

#[tokio::test]
async fn custom_node_import_rejects_empty_base_url() {
    let app = router();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/custom-nodes/import")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "base_url": "" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn custom_node_crud_over_http() {
    let app = router();
    let body = json!({
        "slug": "Greet", "label": "Greeter",
        "endpoint": "http://localhost:9000/nodes/greet",
        "config_schema": { "type": "object" }
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/custom-nodes")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let def: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(def["slug"], "greet"); // normalized

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/custom-nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["label"], "Greeter");
}
