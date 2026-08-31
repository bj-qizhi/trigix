use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const REALTIME_SESSION_TTL_SECONDS: u64 = 120;
pub const REALTIME_SESSION_MAX_SECONDS: u64 = 3_300;
const DEFAULT_MODEL: &str = "gpt-realtime-2.1";
const DEFAULT_TRANSCRIPTION_MODEL: &str = "gpt-4o-mini-transcribe";
const DEFAULT_VOICE: &str = "marin";
const OPENAI_CLIENT_SECRETS_URL: &str = "https://api.openai.com/v1/realtime/client_secrets";
const OPENAI_CALLS_URL: &str = "https://api.openai.com/v1/realtime/calls";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealtimeVoiceBootstrap {
    pub schema_version: &'static str,
    pub session_id: String,
    pub provider: &'static str,
    pub model: String,
    pub client_secret: String,
    pub client_secret_expires_at_unix_seconds: u64,
    pub session_expires_at_unix_seconds: u64,
    pub calls_url: &'static str,
    pub policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeVoiceGrant {
    pub tenant_id: String,
    pub device_id: String,
    pub actor_id: String,
    pub policy_version: String,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Default)]
pub struct RealtimeVoiceSessionStore {
    inner: Arc<Mutex<HashMap<String, RealtimeVoiceGrant>>>,
}

impl RealtimeVoiceSessionStore {
    pub fn insert(
        &self,
        session_id: String,
        grant: RealtimeVoiceGrant,
    ) -> Result<(), RealtimeVoiceError> {
        self.inner
            .lock()
            .map_err(|_| RealtimeVoiceError::Unavailable)?
            .insert(session_id, grant);
        Ok(())
    }

    pub fn authorize(
        &self,
        session_id: &str,
        tenant_id: &str,
        device_id: &str,
        now_unix_seconds: u64,
    ) -> Result<RealtimeVoiceGrant, RealtimeVoiceError> {
        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| RealtimeVoiceError::Unavailable)?;
        sessions.retain(|_, grant| grant.expires_at_unix_seconds > now_unix_seconds);
        let grant = sessions
            .get(session_id)
            .filter(|grant| grant.tenant_id == tenant_id && grant.device_id == device_id)
            .cloned()
            .ok_or(RealtimeVoiceError::Unauthorized)?;
        Ok(grant)
    }

    pub fn revoke_device(&self, device_id: &str) {
        if let Ok(mut sessions) = self.inner.lock() {
            sessions.retain(|_, grant| grant.device_id != device_id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeVoiceError {
    NotConfigured,
    ProviderUnavailable,
    InvalidProviderResponse,
    Unauthorized,
    Unavailable,
}

#[derive(Clone)]
pub struct OpenAiRealtimeProvider {
    api_key: String,
    model: String,
    transcription_model: String,
    voice: String,
    client: reqwest::Client,
}

impl OpenAiRealtimeProvider {
    pub fn from_env() -> Result<Self, RealtimeVoiceError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or(RealtimeVoiceError::NotConfigured)?;
        let model = bounded_env("VOICE_REALTIME_MODEL", DEFAULT_MODEL, 128)?;
        let transcription_model = bounded_env(
            "VOICE_TRANSCRIPTION_MODEL",
            DEFAULT_TRANSCRIPTION_MODEL,
            128,
        )?;
        let voice = bounded_env("VOICE_REALTIME_VOICE", DEFAULT_VOICE, 64)?;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|_| RealtimeVoiceError::Unavailable)?;
        Ok(Self {
            api_key,
            model,
            transcription_model,
            voice,
            client,
        })
    }

    pub async fn create_bootstrap(
        &self,
        tenant_id: &str,
        device_id: &str,
        actor_id: &str,
        policy_version: &str,
        now_unix_seconds: u64,
    ) -> Result<RealtimeVoiceBootstrap, RealtimeVoiceError> {
        let session_id = format!("voice-{}", uuid::Uuid::new_v4());
        let safety_identifier = safety_identifier(tenant_id, device_id, actor_id);
        let request = provider_request(&self.model, &self.transcription_model, &self.voice);
        let response = self
            .client
            .post(OPENAI_CLIENT_SECRETS_URL)
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(CONTENT_TYPE, "application/json")
            .header("OpenAI-Safety-Identifier", safety_identifier)
            .json(&request)
            .send()
            .await
            .map_err(|_| RealtimeVoiceError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(RealtimeVoiceError::ProviderUnavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > 65_536)
        {
            return Err(RealtimeVoiceError::InvalidProviderResponse);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| RealtimeVoiceError::InvalidProviderResponse)?;
        let secret = parse_provider_secret(&bytes, now_unix_seconds)?;
        Ok(RealtimeVoiceBootstrap {
            schema_version: "realtime-voice-bootstrap-v1",
            session_id,
            provider: "openai",
            model: self.model.clone(),
            client_secret: secret.value,
            client_secret_expires_at_unix_seconds: secret.expires_at,
            session_expires_at_unix_seconds: now_unix_seconds + REALTIME_SESSION_MAX_SECONDS,
            calls_url: OPENAI_CALLS_URL,
            policy_version: policy_version.to_owned(),
        })
    }
}

#[derive(Debug, Serialize)]
struct ProviderClientSecretRequest<'a> {
    expires_after: ProviderExpiry,
    session: ProviderSession<'a>,
}

#[derive(Debug, Serialize)]
struct ProviderExpiry {
    anchor: &'static str,
    seconds: u64,
}

#[derive(Debug, Serialize)]
struct ProviderSession<'a> {
    #[serde(rename = "type")]
    session_type: &'static str,
    model: &'a str,
    audio: ProviderAudio<'a>,
    tools: [serde_json::Value; 0],
    tool_choice: &'static str,
}

#[derive(Debug, Serialize)]
struct ProviderAudio<'a> {
    input: ProviderAudioInput<'a>,
    output: ProviderAudioOutput<'a>,
}

#[derive(Debug, Serialize)]
struct ProviderAudioInput<'a> {
    transcription: ProviderTranscription<'a>,
    noise_reduction: ProviderNoiseReduction,
    turn_detection: ProviderTurnDetection,
}

#[derive(Debug, Serialize)]
struct ProviderTranscription<'a> {
    model: &'a str,
}

#[derive(Debug, Serialize)]
struct ProviderNoiseReduction {
    #[serde(rename = "type")]
    reduction_type: &'static str,
}

#[derive(Debug, Serialize)]
struct ProviderTurnDetection {
    #[serde(rename = "type")]
    detection_type: &'static str,
    create_response: bool,
    interrupt_response: bool,
}

#[derive(Debug, Serialize)]
struct ProviderAudioOutput<'a> {
    voice: &'a str,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct ProviderClientSecret {
    value: String,
    expires_at: u64,
}

fn provider_request<'a>(
    model: &'a str,
    transcription_model: &'a str,
    voice: &'a str,
) -> ProviderClientSecretRequest<'a> {
    ProviderClientSecretRequest {
        expires_after: ProviderExpiry {
            anchor: "created_at",
            seconds: REALTIME_SESSION_TTL_SECONDS,
        },
        session: ProviderSession {
            session_type: "realtime",
            model,
            audio: ProviderAudio {
                input: ProviderAudioInput {
                    transcription: ProviderTranscription {
                        model: transcription_model,
                    },
                    noise_reduction: ProviderNoiseReduction {
                        reduction_type: "near_field",
                    },
                    turn_detection: ProviderTurnDetection {
                        detection_type: "server_vad",
                        create_response: true,
                        interrupt_response: true,
                    },
                },
                output: ProviderAudioOutput { voice },
            },
            tools: [],
            tool_choice: "none",
        },
    }
}

fn safety_identifier(tenant_id: &str, device_id: &str, actor_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"trigix-realtime-voice-v1\0");
    digest.update(tenant_id.as_bytes());
    digest.update(b"\0");
    digest.update(device_id.as_bytes());
    digest.update(b"\0");
    digest.update(actor_id.as_bytes());
    hex::encode(digest.finalize())
}

fn valid_client_secret(value: &str) -> bool {
    value.starts_with("ek_") && value.len() <= 2048 && value.is_ascii()
}

fn parse_provider_secret(
    bytes: &[u8],
    now_unix_seconds: u64,
) -> Result<ProviderClientSecret, RealtimeVoiceError> {
    if bytes.len() > 65_536 {
        return Err(RealtimeVoiceError::InvalidProviderResponse);
    }
    let secret: ProviderClientSecret =
        serde_json::from_slice(bytes).map_err(|_| RealtimeVoiceError::InvalidProviderResponse)?;
    if !valid_client_secret(&secret.value)
        || secret.expires_at <= now_unix_seconds
        || secret.expires_at > now_unix_seconds + 300
    {
        return Err(RealtimeVoiceError::InvalidProviderResponse);
    }
    Ok(secret)
}

fn bounded_env(name: &str, default: &str, maximum: usize) -> Result<String, RealtimeVoiceError> {
    let value = std::env::var(name).unwrap_or_else(|_| default.to_owned());
    if value.is_empty()
        || value.len() > maximum
        || value.chars().any(|character| character.is_control())
    {
        return Err(RealtimeVoiceError::NotConfigured);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_contract_is_short_lived_and_cannot_call_tools() {
        let value = serde_json::to_value(provider_request(
            DEFAULT_MODEL,
            DEFAULT_TRANSCRIPTION_MODEL,
            DEFAULT_VOICE,
        ))
        .unwrap();
        assert_eq!(value["expires_after"]["seconds"], 120);
        assert_eq!(REALTIME_SESSION_MAX_SECONDS, 3_300);
        assert_eq!(value["session"]["type"], "realtime");
        assert_eq!(value["session"]["tools"], serde_json::json!([]));
        assert_eq!(value["session"]["tool_choice"], "none");
        assert_eq!(
            value["session"]["audio"]["input"]["turn_detection"]["interrupt_response"],
            true
        );
    }

    #[test]
    fn grants_are_device_bound_and_expire_fail_closed() {
        let store = RealtimeVoiceSessionStore::default();
        store
            .insert(
                "session-a".to_owned(),
                RealtimeVoiceGrant {
                    tenant_id: "tenant-a".to_owned(),
                    device_id: "device-a".to_owned(),
                    actor_id: "actor-a".to_owned(),
                    policy_version: "policy-a".to_owned(),
                    expires_at_unix_seconds: 200,
                },
            )
            .unwrap();
        assert!(store
            .authorize("session-a", "tenant-a", "device-a", 199)
            .is_ok());
        assert_eq!(
            store.authorize("session-a", "tenant-a", "device-b", 199),
            Err(RealtimeVoiceError::Unauthorized)
        );
        assert_eq!(
            store.authorize("session-a", "tenant-a", "device-a", 200),
            Err(RealtimeVoiceError::Unauthorized)
        );
    }

    #[test]
    fn revocation_removes_all_device_grants() {
        let store = RealtimeVoiceSessionStore::default();
        store
            .insert(
                "session-a".to_owned(),
                RealtimeVoiceGrant {
                    tenant_id: "tenant-a".to_owned(),
                    device_id: "device-a".to_owned(),
                    actor_id: "actor-a".to_owned(),
                    policy_version: "policy-a".to_owned(),
                    expires_at_unix_seconds: 200,
                },
            )
            .unwrap();
        store.revoke_device("device-a");
        assert_eq!(
            store.authorize("session-a", "tenant-a", "device-a", 100),
            Err(RealtimeVoiceError::Unauthorized)
        );
    }

    #[test]
    fn safety_identifier_is_stable_and_content_free() {
        let first = safety_identifier("tenant-a", "device-a", "actor-a");
        assert_eq!(first, safety_identifier("tenant-a", "device-a", "actor-a"));
        assert_eq!(first.len(), 64);
        assert!(!first.contains("tenant"));
        assert_ne!(first, safety_identifier("tenant-a", "device-b", "actor-a"));
    }

    #[test]
    fn deterministic_provider_fixture_rejects_expired_malformed_and_oversized_secrets() {
        let valid =
            parse_provider_secret(br#"{"value":"ek_fixture","expires_at":1120}"#, 1_000).unwrap();
        assert_eq!(valid.value, "ek_fixture");
        for fixture in [
            br#"{"value":"long_lived_key","expires_at":1120}"#.as_slice(),
            br#"{"value":"ek_expired","expires_at":1000}"#.as_slice(),
            br#"{"value":"ek_too_long","expires_at":1400}"#.as_slice(),
            br#"{"value":"ek_missing_expiry"}"#.as_slice(),
        ] {
            assert_eq!(
                parse_provider_secret(fixture, 1_000),
                Err(RealtimeVoiceError::InvalidProviderResponse)
            );
        }
        assert_eq!(
            parse_provider_secret(&vec![b'x'; 65_537], 1_000),
            Err(RealtimeVoiceError::InvalidProviderResponse)
        );
    }
}
