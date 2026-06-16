// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputField {
    pub key: String,
    #[serde(default = "default_field_type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

fn default_field_type() -> String {
    "string".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub workflow_version_id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_schema: Vec<InputField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    /// For edges leaving a condition node: "true" or "false".
    /// Omitted for unconditional edges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Trigger,
    Http,
    Agent,
    Condition,
    Approval,
    Map,
    Filter,
    Aggregate,
    Sort,
    Transform,
    Delay,
    SubWorkflow,
    Assert,
    Catch,
    FanOut,
    FanIn,
    Code,
    Slack,
    Email,
    Openai,
    Gemini,
    Database,
    Extract,
    Merge,
    Loop,
    Graphql,
    Validate,
    Note,
    Claude,
    Split,
    Join,
    Switch,
    Random,
    Dedupe,
    Regex,
    Csv,
    Rename,
    Format,
    Github,
    Webhook,
    Jira,
    Notion,
    Linear,
    Airtable,
    ForEach,
    Discord,
    Teams,
    Sheets,
    Xml,
    Yaml,
    Twilio,
    Stripe,
    Crypto,
    Hubspot,
    Date,
    Zendesk,
    Redis,
    Elasticsearch,
    Pagerduty,
    Handlebars,
    Math,
    ArrayUtils,
    Shopify,
    Datadog,
    Salesforce,
    Freshdesk,
    Mailgun,
    Asana,
    Servicenow,
    Confluence,
    Bitbucket,
    AzureDevops,
    Twitch,
    Figma,
    Dropbox,
    Cloudflare,
    Box,
    Okta,
    Zoom,
    Spotify,
    Typeform,
    Webflow,
    Intercom,
    Pipedrive,
    Trello,
    Monday,
    Clickup,
    Amplitude,
    Mixpanel,
    Segment,
    Sendgrid,
    Braintree,
    Paypal,
    Razorpay,
    Firebase,
    Supabase,
    Mailchimp,
    Activecampaign,
    Klaviyo,
    Resend,
    Contentful,
    Algolia,
    Postmark,
    Vonage,
    Telegram,
    Replicate,
    Mistral,
    Whatsapp,
    Googledocs,
    Perplexity,
    Cohere,
    Googledrive,
    Woocommerce,
    Pinecone,
    Togetherai,
    Awss3,
    Huggingface,
    Groq,
    Openrouter,
    Qdrant,
    Cloudinary,
    Gcal,
    Docusign,
    Xero,
    Calendly,
    Apify,
    Ganalytics,
    Neon,
    Copper,
    /// Azure OpenAI (deployment-based chat completions, api-key header).
    AzureOpenai,
    /// xAI Grok (OpenAI-compatible chat completions).
    Grok,
    /// Ollama — self-hosted OpenAI-compatible local models (configurable base URL).
    Ollama,
    /// Weaviate vector store (REST + GraphQL search).
    Weaviate,
    /// Chroma vector store (REST data API).
    Chroma,
    /// MongoDB via the Atlas Data API (HTTP).
    Mongodb,
    /// ClickHouse via its HTTP interface.
    Clickhouse,
    /// Google Cloud Storage (JSON API, OAuth2 bearer token).
    Gcs,
    /// Azure Blob Storage (REST + SAS token).
    AzureBlob,
    /// Cryptographic hashing / HMAC (SHA-256/384/512).
    Hash,
    /// HMAC-signed JWT (HS256/384/512) sign & verify.
    Jwt,
    /// Google Vertex AI (Gemini generateContent, OAuth2 bearer token).
    Vertex,
    /// AWS SQS (query protocol, SigV4-signed).
    Sqs,
    /// AWS SNS (query protocol, SigV4-signed).
    Sns,
    /// AWS Bedrock InvokeModel (SigV4-signed, model-native body).
    Bedrock,
    /// Milvus / Zilliz vector store (REST API v2).
    Milvus,
    /// Kafka via the Confluent REST Proxy.
    Kafka,
    /// RabbitMQ via the Management HTTP API.
    Rabbitmq,
    /// Zip archive create / extract (base64 payloads).
    Zip,
    /// Image resize / convert / metadata (base64 payloads).
    Image,
    /// PDF text extraction (base64 input).
    PdfExtract,
    /// OCR via the tesseract CLI (base64 image input).
    Ocr,
    /// 飞书 / Lark (bot webhook or app message API).
    Feishu,
    /// 钉钉 / DingTalk custom robot (optional HMAC sign).
    Dingtalk,
    /// 企业微信 / WeChat Work group robot.
    Wecom,
    /// Text embeddings (OpenAI-compatible).
    Embedding,
    /// Document reranking (Cohere/Jina-compatible).
    Reranker,
    /// Text splitting / chunking (pure compute).
    TextSplitter,
    /// LLM structured (JSON) output.
    StructuredOutput,
    /// LLM classification into one of N categories.
    Classifier,
    /// Image generation (OpenAI-compatible images).
    ImageGen,
    /// Speech-to-text transcription (Whisper-compatible).
    SpeechToText,
    /// Text-to-speech synthesis (OpenAI-compatible).
    Tts,
    /// HTML extraction via CSS selectors.
    HtmlExtract,
    /// RSS / Atom feed reading.
    Rss,
    // ── 中国国内大模型 ───────────────────────────────────
    Deepseek,
    Qwen,
    Zhipu,
    Moonshot,
    Doubao,
    Minimax,
    Ernie,
    Hunyuan,
    /// Retrieval-Augmented Generation: query a pgvector knowledge base via the
    /// AI runtime and return the most relevant chunks.
    Rag,
    /// Ingest a document into a pgvector knowledge base via the AI runtime.
    RagIngest,
    /// Community/third-party node served over HTTP via the node SDK. The node's
    /// endpoint is resolved from the custom-node registry at execution start.
    Custom,
}

impl WorkflowGraph {
    pub fn validate(&self) -> Result<(), GraphError> {
        if self.workflow_version_id.is_empty() {
            return Err(GraphError::MissingWorkflowVersion);
        }
        if self.nodes.is_empty() {
            return Err(GraphError::EmptyGraph);
        }

        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            if node.id.is_empty() {
                return Err(GraphError::EmptyNodeId);
            }
            if !node_ids.insert(node.id.as_str()) {
                return Err(GraphError::DuplicateNode(node.id.clone()));
            }
        }

        for edge in &self.edges {
            if !node_ids.contains(edge.source.as_str()) {
                return Err(GraphError::UnknownEdgeSource(edge.source.clone()));
            }
            if !node_ids.contains(edge.target.as_str()) {
                return Err(GraphError::UnknownEdgeTarget(edge.target.clone()));
            }
        }

        self.topological_order().map(|_| ())
    }

    pub fn topological_order(&self) -> Result<Vec<String>, GraphError> {
        let mut indegree: HashMap<&str, usize> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), 0usize))
            .collect();
        let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();

        for edge in &self.edges {
            let Some(target_degree) = indegree.get_mut(edge.target.as_str()) else {
                return Err(GraphError::UnknownEdgeTarget(edge.target.clone()));
            };
            *target_degree += 1;
            outgoing
                .entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
        }

        let mut ready: Vec<&str> = indegree
            .iter()
            .filter_map(|(node_id, degree)| (*degree == 0).then_some(*node_id))
            .collect();
        ready.sort_unstable();

        let mut ordered = Vec::with_capacity(self.nodes.len());
        while let Some(node_id) = ready.pop() {
            ordered.push(node_id.to_string());

            for target in outgoing.get(node_id).into_iter().flatten() {
                let degree = indegree
                    .get_mut(target)
                    .ok_or_else(|| GraphError::UnknownEdgeTarget((*target).to_string()))?;
                *degree -= 1;
                if *degree == 0 {
                    ready.push(target);
                    ready.sort_unstable();
                }
            }
        }

        if ordered.len() != self.nodes.len() {
            return Err(GraphError::CycleDetected);
        }

        Ok(ordered)
    }

    /// Returns nodes grouped by execution level — all nodes within a level can run in parallel
    /// because they share no direct dependency edges. Levels are ordered dependency-first.
    pub fn topological_levels(&self) -> Result<Vec<Vec<String>>, GraphError> {
        self.validate()?;

        let mut indegree: HashMap<&str, usize> =
            self.nodes.iter().map(|n| (n.id.as_str(), 0usize)).collect();
        let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();

        for edge in &self.edges {
            let Some(deg) = indegree.get_mut(edge.target.as_str()) else {
                return Err(GraphError::UnknownEdgeTarget(edge.target.clone()));
            };
            *deg += 1;
            outgoing
                .entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
        }

        let mut current_level: Vec<&str> = indegree
            .iter()
            .filter_map(|(id, &deg)| (deg == 0).then_some(*id))
            .collect();
        current_level.sort_unstable();

        let mut levels: Vec<Vec<String>> = Vec::new();
        let mut total = 0usize;

        while !current_level.is_empty() {
            levels.push(current_level.iter().map(|s| s.to_string()).collect());
            total += current_level.len();

            let mut next_level: Vec<&str> = Vec::new();
            for node_id in &current_level {
                for &target in outgoing.get(node_id).into_iter().flatten() {
                    let deg = indegree.get_mut(target).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next_level.push(target);
                    }
                }
            }
            next_level.sort_unstable();
            current_level = next_level;
        }

        if total != self.nodes.len() {
            return Err(GraphError::CycleDetected);
        }

        Ok(levels)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    MissingWorkflowVersion,
    EmptyGraph,
    EmptyNodeId,
    DuplicateNode(String),
    UnknownEdgeSource(String),
    UnknownEdgeTarget(String),
    CycleDetected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_single_node_graph() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![Node {
                id: "trigger".to_string(),
                node_type: NodeType::Trigger,
                config: None,
            }],
            edges: vec![],
            input_schema: vec![],
        };

        assert_eq!(graph.validate(), Ok(()));
        assert_eq!(graph.topological_order(), Ok(vec!["trigger".to_string()]));
    }

    #[test]
    fn rejects_empty_graph() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![],
            edges: vec![],
            input_schema: vec![],
        };

        assert_eq!(graph.validate(), Err(GraphError::EmptyGraph));
    }

    #[test]
    fn rejects_unknown_edge_target() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![Node {
                id: "trigger".to_string(),
                node_type: NodeType::Trigger,
                config: None,
            }],
            edges: vec![Edge {
                source: "trigger".to_string(),
                target: "missing".to_string(),
                condition_label: None,
            }],
            input_schema: vec![],
        };

        assert_eq!(
            graph.validate(),
            Err(GraphError::UnknownEdgeTarget("missing".to_string()))
        );
    }

    #[test]
    fn rejects_cycles() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![
                Node {
                    id: "a".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
                Node {
                    id: "b".to_string(),
                    node_type: NodeType::Agent,
                    config: None,
                },
            ],
            edges: vec![
                Edge {
                    source: "a".to_string(),
                    target: "b".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "b".to_string(),
                    target: "a".to_string(),
                    condition_label: None,
                },
            ],
            input_schema: vec![],
        };

        assert_eq!(graph.validate(), Err(GraphError::CycleDetected));
    }
}
