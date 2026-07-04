// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! AI workflow generation & copilot handlers.

use super::*;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Stream an LLM chat completion (Anthropic or OpenAI-compatible), invoking
/// `on_delta` for each text token and returning the full accumulated text.
#[allow(clippy::too_many_arguments)]
async fn stream_chat(
    http_client: &reqwest::Client,
    is_anthropic: bool,
    url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f64,
    mut on_delta: impl FnMut(&str),
) -> Result<String, ApiError> {
    use tokio_stream::StreamExt;
    let resp = if is_anthropic {
        let payload = serde_json::json!({
            "model": model, "max_tokens": max_tokens, "temperature": temperature,
            "system": system, "stream": true,
            "messages": [{ "role": "user", "content": user }],
        });
        http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
    } else {
        let payload = serde_json::json!({
            "model": model, "max_tokens": max_tokens, "temperature": temperature, "stream": true,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });
        http_client
            .post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
    }
    .map_err(|e| ApiError::bad_request(&format!("LLM request failed: {e}")))?;
    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(&format!("LLM API {code}: {text}")));
    }

    let mut full = String::new();
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| ApiError::bad_request(&format!("LLM stream error: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(nl) = buf.find('\n') {
            let line: String = buf.drain(..=nl).collect();
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                let delta = if is_anthropic {
                    v["delta"]["text"].as_str()
                } else {
                    v["choices"][0]["delta"]["content"].as_str()
                };
                if let Some(d) = delta {
                    if !d.is_empty() {
                        full.push_str(d);
                        on_delta(d);
                    }
                }
            }
        }
    }
    Ok(full)
}

/// Resolve the API key for an AI-assist request: an explicit `api_key` wins;
/// otherwise a named stored credential is decrypted server-side. `None` lets the
/// caller fall back to an environment variable.
async fn resolve_api_key(
    state: &AppState,
    tenant_id: &str,
    api_key: Option<String>,
    credential_name: Option<&str>,
) -> Option<String> {
    if let Some(k) = api_key.filter(|k| !k.trim().is_empty()) {
        return Some(k);
    }
    let name = credential_name?.trim();
    if name.is_empty() {
        return None;
    }
    state
        .credential_store
        .get_by_name(tenant_id, name)
        .await
        .ok()
        .flatten()
}

/// Resolve provider / key / endpoint for an AI-assist request (shared by the
/// buffered and streaming copilot/generation handlers).
fn resolve_assist_llm(
    provider: Option<&str>,
    api_key: Option<String>,
    base_url: Option<&str>,
) -> Result<(bool, String, String), ApiError> {
    let provider = provider.unwrap_or("anthropic").to_lowercase();
    let is_anthropic = provider == "anthropic" || provider == "claude";
    let key = api_key
        .or_else(|| {
            if is_anthropic {
                std::env::var("ANTHROPIC_API_KEY").ok()
            } else {
                std::env::var("OPENAI_API_KEY").ok()
            }
        })
        .ok_or_else(|| {
            ApiError::bad_request(
                "No API key: provide api_key (or set ANTHROPIC_API_KEY / OPENAI_API_KEY)",
            )
        })?;
    let url = if is_anthropic {
        String::new()
    } else {
        resolve_generation_base_url(&provider, base_url).ok_or_else(|| {
            ApiError::bad_request(
                "Unknown provider: set base_url to an OpenAI-compatible /chat/completions endpoint",
            )
        })?
    };
    Ok((is_anthropic, key, url))
}

/// Emit an error frame + return, from inside an SSE task.
fn sse_err(tx: &mpsc::Sender<Result<SseEvent, Infallible>>, msg: &str) {
    let data = serde_json::json!({ "error": msg }).to_string();
    let _ = tx.try_send(Ok(SseEvent::default().event("error").data(data)));
}

/// Every dispatchable node type (source of truth: `node_type_to_str`). Fed to the
/// generator so it can use the full palette, not just the ~30 detailed above.
/// Keep in sync when node types are added/removed.
const ALL_NODE_TYPES: &str = "activecampaign, agent, aggregate, airtable, algolia, amplitude, apify, approval, array_utils, \
asana, assert, awss3, azure_blob, azure_openai, azure_devops, bedrock, bigquery, bitbucket, box, braintree, calendly, catch, \
chroma, classifier, claude, clickhouse, clickup, cloudflare, cloudinary, code, cohere, condition, confluence, contentful, \
copper, crypto, csv, custom, database, datadog, date, dedupe, delay, dingtalk, discord, docusign, dropbox, elasticsearch, \
email, embedding, ernie, extract, fan_in, fan_out, feishu, figma, filter, firebase, for_each, format, freshdesk, ftp, \
ganalytics, gcal, gcs, gemini, github, googledocs, googledrive, graphql, groq, handlebars, hash, html_extract, http, hubspot, \
huggingface, image, image_gen, imap, intercom, jira, join, jwt, kafka, klaviyo, linear, loop, mailchimp, mailgun, map, math, \
merge, milvus, minimax, mistral, mixpanel, monday, mongodb, mysql, neon, note, notion, ocr, okta, openai, openai_compat, \
openrouter, pagerduty, paypal, pdf_extract, perplexity, pinecone, pipedrive, postmark, qdrant, rabbitmq, rag, rag_ingest, \
random, razorpay, redis, regex, rename, replicate, reranker, resend, rss, salesforce, segment, sendgrid, servicenow, sftp, \
sheets, shopify, slack, snowflake, sns, sort, speech_to_text, split, spotify, sqlserver, sqs, ssh, stripe, structured_output, \
sub_workflow, supabase, switch, teams, telegram, text_splitter, togetherai, transform, trello, trigger, tts, twilio, twitch, \
typeform, validate, vertex, video_gen, vonage, wait, weaviate, webflow, webhook, wecom, whatsapp, woocommerce, xero, xml, \
yaml, zendesk, zip, zoom";

/// The trailing rules for the generation prompt (kept out of the raw catalog
/// literal so the full node-type list can be injected between them).
const GEN_RULES: &str = "\n\nRules:\n\
- Always start with a trigger node as node_1\n\
- Use descriptive node IDs like \"fetch_data\", \"parse_response\", \"send_slack\"\n\
- Keep graphs focused — 3-8 nodes is ideal\n\
- Use {{credential.name}} for sensitive values (API keys, tokens)\n\
- Return ONLY the JSON, no explanation";

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

    let api_key = resolve_api_key(
        &state,
        &body.tenant_id,
        body.api_key.clone(),
        body.credential_name.as_deref(),
    )
    .await
    .or_else(|| {
        if is_anthropic {
            std::env::var("ANTHROPIC_API_KEY").ok()
        } else {
            std::env::var("OPENAI_API_KEY").ok()
        }
    })
    .ok_or_else(|| {
        ApiError::bad_request(
            "No generation API key: provide api_key or credential_name (or set ANTHROPIC_API_KEY / OPENAI_API_KEY)",
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
- openai_compat: { provider (deepseek/qwen/zhipu/moonshot/grok/hunyuan/ollama/doubao) OR base_url, api_key, model, prompt_template, system_prompt?, max_tokens? } (any OpenAI-compatible LLM; provider sets a default endpoint+model, e.g. deepseek=deepseek-v4-flash, qwen=qwen-max, zhipu=glm-4.6, moonshot=kimi-latest, grok=grok-4.3)
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
Edges: source → target. For condition nodes add condition_label: "true" or "false" on edges."#,
    );
    // The detailed catalog above covers the common nodes; also give the model the
    // full palette so it can reach any integration node, not just those ~30.
    system_prompt.push_str(
        "\n\nAll available node types (the ones above have their config detailed; for any other, \
use its standard config — usually { api_key/token, url/endpoint, method, body?, … }):\n",
    );
    system_prompt.push_str(ALL_NODE_TYPES);
    system_prompt.push_str(GEN_RULES);

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

/// Call the copilot/generation LLM (Anthropic, or any OpenAI-compatible
/// endpoint) and return the raw assistant text.
#[allow(clippy::too_many_arguments)]
async fn chat_once(
    http_client: &reqwest::Client,
    is_anthropic: bool,
    url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f64,
) -> Result<String, ApiError> {
    if is_anthropic {
        let payload = serde_json::json!({
            "model": model, "max_tokens": max_tokens, "temperature": temperature,
            "system": system, "messages": [{ "role": "user", "content": user }],
        });
        let resp = http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
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
            .map_err(|e| ApiError::bad_request(&format!("Claude response parse: {e}")))?;
        Ok(j["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string())
    } else {
        let payload = serde_json::json!({
            "model": model, "max_tokens": max_tokens, "temperature": temperature,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });
        let resp = http_client
            .post(url)
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
            .map_err(|e| ApiError::bad_request(&format!("Generation response parse: {e}")))?;
        Ok(j["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string())
    }
}

/// Pull a fenced ```json graph block out of a copilot reply, validate it as a
/// `WorkflowGraph`, and return `(reply_without_block, normalized_graph)`.
/// Returns `(original, None)` when there is no valid graph block (pure Q&A).
fn extract_proposed_graph(content: &str) -> (String, Option<serde_json::Value>) {
    let Some(start) = content.find("```") else {
        return (content.to_string(), None);
    };
    let after = &content[start + 3..];
    let after = after.strip_prefix("json").unwrap_or(after);
    let after = after.trim_start_matches(['\n', '\r']);
    let Some(end) = after.find("```") else {
        return (content.to_string(), None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(after[..end].trim()) else {
        return (content.to_string(), None);
    };
    // Accept a bare {nodes,edges} or a wrapped {graph:{...}}.
    let mut graph = if v.get("nodes").is_some() {
        v
    } else {
        v.get("graph").cloned().unwrap_or(serde_json::Value::Null)
    };
    if graph.get("nodes").is_none() {
        return (content.to_string(), None);
    }
    if let Some(o) = graph.as_object_mut() {
        o.entry("workflow_version_id".to_string())
            .or_insert_with(|| serde_json::Value::String("draft".to_string()));
    }
    // Structural validation (also migrates any legacy node types in the graph).
    if serde_json::from_value::<workflow_core::WorkflowGraph>(graph.clone()).is_err() {
        return (content.to_string(), None);
    }
    let reply = format!("{}{}", &content[..start], &after[end + 3..]);
    (reply.trim().to_string(), Some(graph))
}

fn copilot_system_prompt(graph_json: Option<&str>) -> String {
    let graph_context = graph_json
        .map(|g| format!("\n\nCurrent workflow graph (JSON):\n```json\n{g}\n```"))
        .unwrap_or_default();
    format!(
        "You are an expert assistant for Trigix, an AI workflow automation platform. You help users \
understand, debug, and EDIT their workflows.\n\n\
When the user asks to CHANGE the workflow (add / remove / fix / rewire nodes or configs), reply with BOTH:\n\
1. A brief plain-text explanation (1-3 sentences) of what you changed.\n\
2. Then the COMPLETE updated graph as ONE ```json fenced block: \
{{ \"nodes\": [{{ \"id\", \"type\", \"config\" }}], \"edges\": [{{ \"source\", \"target\", \"condition_label\"? }}] }}. \
Include ALL nodes (unchanged ones verbatim), keep existing node ids stable, and start with a trigger node.\n\
For pure questions (no change requested), reply in plain text ONLY — no code block.\n\n\
Common node types: trigger, http, openai_compat (config.provider=deepseek/qwen/grok/zhipu/moonshot/hunyuan/ollama/doubao, or base_url), \
openai, claude, gemini, condition (field/operator/value + true/false condition_label on edges), transform (template), filter, aggregate, \
code (Rhai), slack, database, rag, extract, merge, loop, fan_out/fan_in, catch (connect via error edge), assert, delay, note. \
Template variables: {{{{input.x}}}}, {{{{node_id.x}}}}, {{{{credential.name}}}}, {{{{env.KEY}}}}.{graph_context}"
    )
}

/// Streaming copilot: SSE of `data:{"delta":…}` frames as the model types, then a
/// final `data:{"done":true,"reply":…,"proposed_graph":…}` (or `{"error":…}`).
async fn copilot_stream(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CopilotRequest>,
) -> Sse<ReceiverStream<Result<SseEvent, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<SseEvent, Infallible>>(256);
    tokio::spawn(async move {
        if require_write(&claims).is_err() {
            sse_err(&tx, "Forbidden: write access required");
            return;
        }
        body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
        let key = resolve_api_key(
            &state,
            &body.tenant_id,
            body.api_key.clone(),
            body.credential_name.as_deref(),
        )
        .await;
        let (is_anthropic, api_key, url) =
            match resolve_assist_llm(body.provider.as_deref(), key, body.base_url.as_deref()) {
                Ok(v) => v,
                Err(e) => {
                    sse_err(&tx, &e.message);
                    return;
                }
            };
        let system = copilot_system_prompt(body.graph_json.as_deref());
        let http_client = reqwest::Client::new();
        let delta_tx = tx.clone();
        let raw = stream_chat(
            &http_client,
            is_anthropic,
            &url,
            &api_key,
            &body.model,
            &system,
            &body.message,
            2048,
            0.4,
            |d| {
                let data = serde_json::json!({ "delta": d }).to_string();
                let _ = delta_tx.try_send(Ok(SseEvent::default().event("delta").data(data)));
            },
        )
        .await;
        match raw {
            Ok(full) => {
                let (reply, proposed_graph) = extract_proposed_graph(&full);
                state.audit_store.record(
                    &body.tenant_id,
                    "copilot.query",
                    "copilot",
                    &body.tenant_id,
                    None,
                );
                let done = serde_json::json!({
                    "done": true, "reply": reply, "proposed_graph": proposed_graph,
                });
                let _ = tx.try_send(Ok(SseEvent::default().event("done").data(done.to_string())));
            }
            Err(e) => sse_err(&tx, &e.message),
        }
    });
    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

async fn copilot_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<CopilotRequest>,
) -> Result<Json<CopilotResponse>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);

    let key = resolve_api_key(
        &state,
        &body.tenant_id,
        body.api_key.clone(),
        body.credential_name.as_deref(),
    )
    .await;
    let (is_anthropic, api_key, url) =
        resolve_assist_llm(body.provider.as_deref(), key, body.base_url.as_deref())?;
    let system = copilot_system_prompt(body.graph_json.as_deref());

    let http_client = reqwest::Client::new();
    let raw = chat_once(
        &http_client,
        is_anthropic,
        &url,
        &api_key,
        &body.model,
        &system,
        &body.message,
        2048,
        0.4,
    )
    .await?;
    let (reply, proposed_graph) = extract_proposed_graph(&raw);

    state.audit_store.record(
        &body.tenant_id,
        "copilot.query",
        "copilot",
        &body.tenant_id,
        None,
    );

    Ok(Json(CopilotResponse {
        reply,
        proposed_graph,
    }))
}

/// Kind-specific system prompt for node-level field assist. Each one forces a
/// bare value (no prose, no code fence) so the result drops straight into the
/// config field.
fn field_assist_system_prompt(kind: &str) -> &'static str {
    match kind {
        "regex" => "You write regular expressions for a workflow automation tool (Rust regex crate syntax). Given a description, output ONE regular expression only — no delimiters, no surrounding quotes, no flags unless asked, no explanation, no code fence.",
        "prompt" => "You write concise, effective LLM prompt templates for a workflow automation tool. Templates may reference workflow variables like {{input.field}} or {{node_id.field}}. Output ONLY the prompt template text — no explanation, no code fence.",
        "code" => "You write Rhai scripts for a workflow 'code' node. `input` holds the incoming JSON value; the script's final expression is the node's output. Output ONLY the script — no explanation, no code fence.",
        "jsonpath" => "You write JSONPath expressions. Output ONE JSONPath expression only — no surrounding quotes, no explanation, no code fence.",
        "template" => "You write output/transform templates for a workflow tool that reference variables like {{input.field}} / {{node_id.field}}. Output ONLY the template — no explanation, no code fence.",
        "sql" => "You write SQL queries. Output ONE SQL query only — no explanation, no code fence.",
        "jq" => "You write jq filters. Output ONE jq filter only — no explanation, no code fence.",
        _ => "Write exactly the value the user describes. Output ONLY the value — no explanation, no code fence.",
    }
}

/// Strip a wrapping ```lang … ``` fence if the model added one despite the
/// instruction, and trim surrounding whitespace.
fn clean_field_value(raw: &str) -> String {
    let t = raw.trim();
    if let Some(after) = t.strip_prefix("```") {
        // Drop the (optional) language token on the fence's first line.
        let body = after.split_once('\n').map(|(_, b)| b).unwrap_or(after);
        let inner = body.strip_suffix("```").unwrap_or(body);
        return inner.trim().to_string();
    }
    t.to_string()
}

/// Generate one config-field value (regex / prompt / code / …) from a
/// plain-language description.
async fn field_assist_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(mut body): Json<FieldAssistRequest>,
) -> Result<Json<FieldAssistResponse>, ApiError> {
    require_write(&claims)?;
    body.tenant_id = effective_tenant_id(&claims, &body.tenant_id);
    if body.instruction.trim().is_empty() {
        return Err(ApiError::bad_request("instruction is required"));
    }

    let key = resolve_api_key(
        &state,
        &body.tenant_id,
        body.api_key.clone(),
        body.credential_name.as_deref(),
    )
    .await;
    let (is_anthropic, api_key, url) =
        resolve_assist_llm(body.provider.as_deref(), key, body.base_url.as_deref())?;

    let system = field_assist_system_prompt(&body.kind);
    let user = match body.context.as_deref().map(str::trim) {
        Some(c) if !c.is_empty() => format!("{}\n\nContext:\n{}", body.instruction, c),
        _ => body.instruction.clone(),
    };

    let http_client = reqwest::Client::new();
    let raw = chat_once(
        &http_client,
        is_anthropic,
        &url,
        &api_key,
        &body.model,
        system,
        &user,
        512,
        0.2,
    )
    .await?;

    Ok(Json(FieldAssistResponse {
        value: clean_field_value(&raw),
    }))
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/workflows/generate",
            get(method_not_allowed).post(generate_workflow),
        )
        .route("/v1/copilot", get(method_not_allowed).post(copilot_handler))
        .route(
            "/v1/copilot/stream",
            get(method_not_allowed).post(copilot_stream),
        )
        .route(
            "/v1/assist/field",
            get(method_not_allowed).post(field_assist_handler),
        )
}

#[cfg(test)]
mod tests {
    use super::{clean_field_value, extract_proposed_graph, ALL_NODE_TYPES};

    #[test]
    fn field_value_is_unwrapped_and_trimmed() {
        assert_eq!(clean_field_value("  \\d{3}-\\d{4}  "), "\\d{3}-\\d{4}");
        assert_eq!(clean_field_value("```regex\n\\d+\n```"), "\\d+");
        assert_eq!(clean_field_value("```\nSELECT 1\n```"), "SELECT 1");
        // No fence, multi-line body preserved.
        assert_eq!(
            clean_field_value("let x = input.n;\nx + 1"),
            "let x = input.n;\nx + 1"
        );
    }

    #[test]
    fn full_node_catalog_covers_the_palette_and_stays_current() {
        let types: Vec<&str> = ALL_NODE_TYPES.split(',').map(|s| s.trim()).collect();
        // The generator must know integration nodes beyond the detailed ~30.
        for t in [
            "discord",
            "stripe",
            "notion",
            "shopify",
            "openai_compat",
            "trigger",
        ] {
            assert!(types.contains(&t), "generator catalog missing '{t}'");
        }
        // The consolidated legacy per-vendor LLM types must NOT reappear here.
        for t in ["grok", "deepseek", "qwen", "hunyuan"] {
            assert!(
                !types.contains(&t),
                "'{t}' was consolidated into openai_compat"
            );
        }
        assert!(
            types.len() > 150,
            "expected the full palette, got {}",
            types.len()
        );
    }

    #[test]
    fn extracts_and_validates_a_proposed_graph_block() {
        let reply = "I added a Slack notification after the HTTP call.\n\n```json\n{\
\"nodes\":[{\"id\":\"trigger\",\"type\":\"trigger\",\"config\":{}},\
{\"id\":\"notify\",\"type\":\"slack\",\"config\":{\"webhook_url\":\"x\"}}],\
\"edges\":[{\"source\":\"trigger\",\"target\":\"notify\"}]}\n```";
        let (text, graph) = extract_proposed_graph(reply);
        assert!(text.starts_with("I added a Slack"));
        assert!(
            !text.contains("```"),
            "the fenced block is stripped from the reply"
        );
        let g = graph.expect("a valid graph should be extracted");
        assert_eq!(g["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(g["workflow_version_id"], "draft"); // normalized
    }

    #[test]
    fn pure_text_reply_has_no_graph() {
        let (text, graph) =
            extract_proposed_graph("This workflow fetches data then posts to Slack.");
        assert!(graph.is_none());
        assert!(text.contains("fetches data"));
    }

    #[test]
    fn a_non_graph_or_invalid_block_is_ignored() {
        // A code block that isn't a workflow graph must not become a proposed graph.
        let (_t, g1) = extract_proposed_graph("here:\n```json\n{\"foo\":1}\n```");
        assert!(g1.is_none());
        // Malformed JSON.
        let (_t2, g2) = extract_proposed_graph("```json\n{ not json\n```");
        assert!(g2.is_none());
    }
}
