// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! AI workflow generation & copilot handlers.

use super::*;

/// Resolve the OpenAI-compatible chat-completions endpoint for a provider key,
/// or honour an explicit base_url override. Returns None for an unknown provider
/// with no override.
fn resolve_generation_base_url(provider: &str, base_url: Option<&str>) -> Option<String> {
    if let Some(b) = base_url {
        let b = b.trim();
        if !b.is_empty() {
            return Some(b.to_string());
        }
    }
    let url = match provider {
        "openai" => "https://api.openai.com/v1/chat/completions",
        "deepseek" => "https://api.deepseek.com/v1/chat/completions",
        "qwen" => "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        "zhipu" => "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        "moonshot" => "https://api.moonshot.cn/v1/chat/completions",
        "grok" => "https://api.x.ai/v1/chat/completions",
        _ => return None,
    };
    Some(url.to_string())
}

async fn generate_workflow(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<GenerateWorkflowRequest>,
) -> Result<(StatusCode, Json<GenerateWorkflowResponse>), ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let provider = body
        .provider
        .as_deref()
        .unwrap_or("anthropic")
        .to_lowercase();
    let is_anthropic = provider == "anthropic" || provider == "claude";

    let api_key = body
        .api_key
        .clone()
        .or_else(|| {
            if is_anthropic {
                std::env::var("ANTHROPIC_API_KEY").ok()
            } else {
                std::env::var("OPENAI_API_KEY").ok()
            }
        })
        .ok_or_else(|| {
            ApiError::bad_request(
                "No generation API key: provide api_key in the request (or set ANTHROPIC_API_KEY / OPENAI_API_KEY)",
            )
        })?;
    let model = body
        .model
        .as_deref()
        .unwrap_or(if is_anthropic {
            "claude-sonnet-4-6"
        } else {
            "gpt-5.4-mini"
        })
        .to_string();
    let temperature = body.temperature.unwrap_or(0.4).clamp(0.0, 2.0);

    let mut system_prompt = String::from(
        r#"You are an expert workflow designer for Trigix, an AI-powered automation platform.

Generate a workflow graph JSON based on the user's description. Respond with ONLY valid JSON in this exact structure:
{
  "name": "Workflow name (concise)",
  "description": "One sentence description",
  "graph": {
    "workflow_version_id": "draft",
    "nodes": [
      { "id": "node_1", "type": "trigger", "config": {} },
      { "id": "node_2", "type": "...", "config": { ... } }
    ],
    "edges": [
      { "source": "node_1", "target": "node_2" }
    ]
  }
}

Available node types and their required config fields:
- trigger: {} (always start here)
- http: { url, method (GET/POST), headers?, body? }
- claude: { api_key, model (claude-sonnet-4-6), prompt_template, system_prompt?, max_tokens? }
- openai: { api_key, model (gpt-5.4-mini), prompt_template, system_prompt?, max_tokens? }
- gemini: { api_key, model (gemini-2.5-flash), prompt_template, system_prompt? }
- deepseek/qwen/zhipu/moonshot/grok: { api_key, model, prompt_template, system_prompt?, max_tokens? } (OpenAI-compatible LLMs; e.g. deepseek model=deepseek-v4-flash, qwen=qwen-max, zhipu=glm-4.6, moonshot=kimi-latest, grok=grok-4.3)
- condition: { field (dot-path), operator (equals/not_equals/contains/gt/lt/exists/not_exists), value? }
- transform: { template (JSON with {{node_id.field}} placeholders) }
- filter: { items (expr), field, operator, value? }
- aggregate: { items (expr), operation (count/sum/avg/min/max/join/first/last), field? }
- delay: { delay_secs }
- slack: { webhook_url, message_template }
- github: { token, endpoint, method }
- jira: { base_url, email, token, endpoint, method, body? }
- notion: { token, endpoint, method, body? }
- database: { url, query }
- code: { code (Rhai script) }
- sub_workflow: { workflow_id }
- fan_out: {} (parallel split)
- fan_in: {} (wait for all parallel branches)
- assert: { condition, message? }
- validate: { source, schema }
- loop: { items, template? }
- extract: { source, path }
- merge: { fields: [{source, key?}] }
- catch: {} (error handler — connect with error edge from failing node)
- note: { text } (documentation only)

Template variables: {{input.field}}, {{node_id.field}}, {{credential.name}}, {{env.KEY}}
Edges: source → target. For condition nodes add condition_label: "true" or "false" on edges.

Rules:
- Always start with a trigger node as node_1
- Use descriptive node IDs like "fetch_data", "parse_response", "send_slack"
- Keep graphs focused — 3-8 nodes is ideal
- Use {{credential.name}} for sensitive values (API keys, tokens)
- Return ONLY the JSON, no explanation"#,
    );

    // Append caller-supplied constraints from the advanced options.
    let mut rules: Vec<String> = Vec::new();
    if !body.allowed_modules.is_empty() {
        rules.push(format!(
            "Use ONLY these node types besides the trigger: {}.",
            body.allowed_modules.join(", ")
        ));
    }
    if let Some(n) = body.max_nodes.filter(|n| *n > 0) {
        rules.push(format!(
            "Use at most {n} nodes in total (including the trigger)."
        ));
    }
    match body.error_handling {
        Some(true) => rules.push(
            "Include error handling: add catch nodes connected via error edges for steps that can fail.".into(),
        ),
        Some(false) => rules.push("Keep it simple — do not add catch / error-handling nodes.".into()),
        None => {}
    }
    if let Some(lang) = body.language.as_deref() {
        if lang.eq_ignore_ascii_case("zh") {
            rules.push("Write the workflow name and description in Simplified Chinese.".into());
        } else {
            rules.push("Write the workflow name and description in English.".into());
        }
    }
    if !rules.is_empty() {
        system_prompt.push_str("\n\nAdditional constraints (must follow):\n");
        for r in &rules {
            system_prompt.push_str("- ");
            system_prompt.push_str(r);
            system_prompt.push('\n');
        }
    }

    // Call the generation LLM: Anthropic, or any OpenAI-compatible provider.
    let http_client = reqwest::Client::new();
    let raw_content = if is_anthropic {
        let payload = serde_json::json!({
            "model": model,
            "max_tokens": 2048,
            "temperature": temperature,
            "system": system_prompt,
            "messages": [{ "role": "user", "content": body.prompt }],
        });
        let resp = http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiError::bad_request(&format!("Claude request failed: {e}")))?;
        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::bad_request(&format!("Claude API {code}: {text}")));
        }
        let j: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::bad_request(&format!("Claude response parse error: {e}")))?;
        j["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string()
    } else {
        let url =
            resolve_generation_base_url(&provider, body.base_url.as_deref()).ok_or_else(|| {
                ApiError::bad_request(
                "Unknown provider: set base_url to an OpenAI-compatible /chat/completions endpoint",
            )
            })?;
        let payload = serde_json::json!({
            "model": model,
            "max_tokens": 2048,
            "temperature": temperature,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": body.prompt },
            ],
        });
        let resp = http_client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiError::bad_request(&format!("Generation request failed: {e}")))?;
        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::bad_request(&format!(
                "Generation API {code}: {text}"
            )));
        }
        let j: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::bad_request(&format!("Generation response parse error: {e}")))?;
        j["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string()
    };

    // Extract JSON from the response (may be wrapped in markdown code blocks)
    let json_str = if raw_content.contains("```") {
        raw_content
            .split("```")
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.trim_start_matches("json").trim())
            .next()
            .unwrap_or(&raw_content)
            .to_string()
    } else {
        raw_content.clone()
    };

    let generated: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|_| ApiError::bad_request(&format!("Claude returned invalid JSON: {json_str}")))?;

    let name = generated["name"]
        .as_str()
        .unwrap_or("Generated Workflow")
        .to_string();
    let description = generated["description"].as_str().unwrap_or("").to_string();
    let graph = generated["graph"].clone();

    if graph.is_null() {
        return Err(ApiError::bad_request(
            "Claude response missing 'graph' field",
        ));
    }

    let mut workflow_record: Option<crate::workflow::WorkflowRecord> = None;

    if body.create {
        let wf = state
            .workflow_service
            .create_workflow(crate::workflow::CreateWorkflowRequest {
                tenant_id: body.tenant_id.clone(),
                workspace_id: body.workspace_id.unwrap_or_default(),
                project_id: body.project_id.unwrap_or_default(),
                name: name.clone(),
                description: Some(description.clone()),
                folder: None,
                created_by: claims.as_ref().and_then(|c| c.user_id.clone()),
            })
            .await?;

        // Deserialize the graph JSON into WorkflowGraph so create_version can store it
        let mut graph_val = graph.clone();
        if let Some(obj) = graph_val.as_object_mut() {
            obj.insert(
                "workflow_version_id".to_string(),
                serde_json::Value::String("draft".to_string()),
            );
        }
        let workflow_graph: workflow_core::WorkflowGraph = serde_json::from_value(graph_val)
            .map_err(|e| ApiError::bad_request(&format!("Invalid graph structure: {e}")))?;

        state
            .workflow_service
            .create_version(
                &wf.id,
                crate::workflow::CreateWorkflowVersionRequest {
                    tenant_id: body.tenant_id.clone(),
                    graph: workflow_graph,
                    status: None,
                    message: Some("Generated by AI".to_string()),
                },
            )
            .await?;

        state.audit_store.record(
            &body.tenant_id,
            audit_action::WORKFLOW_CREATED,
            "workflow",
            &wf.id,
            Some(serde_json::Value::String("ai_generated".to_string())),
        );

        workflow_record = Some(wf);
    }

    Ok((
        StatusCode::CREATED,
        Json(GenerateWorkflowResponse {
            graph,
            name,
            description,
            workflow: workflow_record,
        }),
    ))
}

async fn copilot_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CopilotRequest>,
) -> Result<Json<CopilotResponse>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let api_key = body
        .api_key
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .ok_or_else(|| {
            ApiError::bad_request("No Claude API key: provide api_key or set ANTHROPIC_API_KEY")
        })?;

    let graph_context = if let Some(g) = &body.graph_json {
        format!("\n\nCurrent workflow graph (JSON):\n```json\n{}\n```", g)
    } else {
        String::new()
    };

    let system = format!(
        "You are an expert assistant for Trigix, an AI-powered workflow automation platform.\
\n\nYou help users understand, debug, and improve their workflows. You have deep knowledge of:\
\n- All 180 node types (trigger, http, claude, openai, gemini, slack, github, database, code, condition, loop, etc.)\
\n- Template variables: {{{{input.field}}}}, {{{{node_id.field}}}}, {{{{credential.name}}}}, {{{{env.KEY}}}}\
\n- Best practices for workflow design (error handling with catch nodes, validation, retry logic)\
\n- Integration patterns (webhooks, scheduled triggers, fan-out/fan-in parallelism)\
\n\nWhen asked to suggest changes, provide concrete, actionable advice with example node configs in JSON.\
\nKeep replies concise and practical — 2-5 sentences for simple questions, structured lists for complex ones.{}",
        graph_context
    );

    let payload = serde_json::json!({
        "model": body.model,
        "max_tokens": 1024,
        "system": system,
        "messages": [{ "role": "user", "content": body.message }],
    });

    let http_client = reqwest::Client::new();
    let resp = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude request failed: {e}")))?;

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(&format!("Claude API {code}: {text}")));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::bad_request(&format!("Claude response parse: {e}")))?;

    let reply = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("(no response)")
        .trim()
        .to_string();

    state.audit_store.record(
        &body.tenant_id,
        "copilot.query",
        "copilot",
        &body.tenant_id,
        None,
    );

    Ok(Json(CopilotResponse { reply }))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/workflows/generate",
            get(method_not_allowed).post(generate_workflow),
        )
        .route("/v1/copilot", get(method_not_allowed).post(copilot_handler))
}
