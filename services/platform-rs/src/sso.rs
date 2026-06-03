// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Enterprise SSO via OpenID Connect (OIDC).
//!
//! Each tenant configures one or more `SsoConnection`s pointing at an IdP
//! (Okta / Azure AD / Google Workspace / …). The login flow is:
//!   1. `GET /v1/sso/:slug/login`    → redirect to the IdP authorize endpoint
//!   2. IdP authenticates the user, redirects back with `?code=&state=`
//!   3. `GET /v1/sso/:slug/callback`  → exchange code, verify the ID token
//!      against the IdP's JWKS, map the email to a user, and issue our own JWT.
//!
//! Endpoint URLs are obtained from the IdP's OIDC discovery document
//! (`{issuer}/.well-known/openid-configuration`). The CSRF `state` is a signed,
//! short-lived JWT that also carries the `nonce`, so no server-side session
//! storage is required and the callback works across instances.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// ── Domain types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoConnection {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub provider: String,
    pub issuer: String,
    pub client_id: String,
    /// Never serialized back to clients.
    #[serde(skip_serializing)]
    pub client_secret: String,
    pub scopes: String,
    pub enabled: bool,
    pub created_at: i64,
}

/// Safe-to-expose view used to render login buttons (no secrets).
#[derive(Debug, Clone, Serialize)]
pub struct PublicSsoConnection {
    pub slug: String,
    pub provider: String,
}

impl From<&SsoConnection> for PublicSsoConnection {
    fn from(c: &SsoConnection) -> Self {
        Self {
            slug: c.slug.clone(),
            provider: c.provider.clone(),
        }
    }
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── Store (Memory / Postgres / Platform enum) ───────────────────────────────

#[derive(Default)]
pub struct MemorySsoStore {
    rows: RwLock<Vec<SsoConnection>>,
}

impl MemorySsoStore {
    pub async fn create(&self, c: SsoConnection) -> SsoConnection {
        self.rows.write().unwrap().push(c.clone());
        c
    }
    pub async fn list_by_tenant(&self, tenant_id: &str) -> Vec<SsoConnection> {
        self.rows
            .read()
            .unwrap()
            .iter()
            .filter(|c| c.tenant_id == tenant_id)
            .cloned()
            .collect()
    }
    pub async fn get_by_slug(&self, slug: &str) -> Option<SsoConnection> {
        self.rows
            .read()
            .unwrap()
            .iter()
            .find(|c| c.slug == slug)
            .cloned()
    }
    pub async fn list_enabled_public(&self) -> Vec<PublicSsoConnection> {
        self.rows
            .read()
            .unwrap()
            .iter()
            .filter(|c| c.enabled)
            .map(PublicSsoConnection::from)
            .collect()
    }
    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        let mut rows = self.rows.write().unwrap();
        let before = rows.len();
        rows.retain(|c| !(c.tenant_id == tenant_id && c.id == id));
        rows.len() != before
    }
}

pub struct PostgresSsoStore {
    pool: sqlx::PgPool,
}

impl PostgresSsoStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, c: SsoConnection) -> SsoConnection {
        let _ = sqlx::query(
            "INSERT INTO af_sso_connections \
             (id, tenant_id, slug, provider, issuer, client_id, client_secret, scopes, enabled, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(&c.id)
        .bind(&c.tenant_id)
        .bind(&c.slug)
        .bind(&c.provider)
        .bind(&c.issuer)
        .bind(&c.client_id)
        .bind(&c.client_secret)
        .bind(&c.scopes)
        .bind(c.enabled)
        .bind(c.created_at)
        .execute(&self.pool)
        .await;
        c
    }

    pub async fn list_by_tenant(&self, tenant_id: &str) -> Vec<SsoConnection> {
        sqlx::query_as::<_, SsoRow>(
            "SELECT * FROM af_sso_connections WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect()
    }

    pub async fn get_by_slug(&self, slug: &str) -> Option<SsoConnection> {
        sqlx::query_as::<_, SsoRow>("SELECT * FROM af_sso_connections WHERE slug = $1")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .map(Into::into)
    }

    pub async fn list_enabled_public(&self) -> Vec<PublicSsoConnection> {
        sqlx::query_as::<_, SsoRow>("SELECT * FROM af_sso_connections WHERE enabled = TRUE")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .iter()
            .map(|r| PublicSsoConnection {
                slug: r.slug.clone(),
                provider: r.provider.clone(),
            })
            .collect()
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        sqlx::query("DELETE FROM af_sso_connections WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .unwrap_or(false)
    }
}

#[derive(sqlx::FromRow)]
struct SsoRow {
    id: String,
    tenant_id: String,
    slug: String,
    provider: String,
    issuer: String,
    client_id: String,
    client_secret: String,
    scopes: String,
    enabled: bool,
    created_at: i64,
}

impl From<SsoRow> for SsoConnection {
    fn from(r: SsoRow) -> Self {
        Self {
            id: r.id,
            tenant_id: r.tenant_id,
            slug: r.slug,
            provider: r.provider,
            issuer: r.issuer,
            client_id: r.client_id,
            client_secret: r.client_secret,
            scopes: r.scopes,
            enabled: r.enabled,
            created_at: r.created_at,
        }
    }
}

pub enum PlatformSsoStore {
    Memory(MemorySsoStore),
    Postgres(PostgresSsoStore),
}

impl Default for PlatformSsoStore {
    fn default() -> Self {
        Self::Memory(MemorySsoStore::default())
    }
}

impl PlatformSsoStore {
    pub fn memory() -> Self {
        Self::Memory(MemorySsoStore::default())
    }
    pub fn postgres(s: PostgresSsoStore) -> Self {
        Self::Postgres(s)
    }
    pub async fn create(&self, c: SsoConnection) -> SsoConnection {
        match self {
            Self::Memory(s) => s.create(c).await,
            Self::Postgres(s) => s.create(c).await,
        }
    }
    pub async fn list_by_tenant(&self, tenant_id: &str) -> Vec<SsoConnection> {
        match self {
            Self::Memory(s) => s.list_by_tenant(tenant_id).await,
            Self::Postgres(s) => s.list_by_tenant(tenant_id).await,
        }
    }
    pub async fn get_by_slug(&self, slug: &str) -> Option<SsoConnection> {
        match self {
            Self::Memory(s) => s.get_by_slug(slug).await,
            Self::Postgres(s) => s.get_by_slug(slug).await,
        }
    }
    pub async fn list_enabled_public(&self) -> Vec<PublicSsoConnection> {
        match self {
            Self::Memory(s) => s.list_enabled_public().await,
            Self::Postgres(s) => s.list_enabled_public().await,
        }
    }
    pub async fn delete(&self, tenant_id: &str, id: &str) -> bool {
        match self {
            Self::Memory(s) => s.delete(tenant_id, id).await,
            Self::Postgres(s) => s.delete(tenant_id, id).await,
        }
    }
}

// ── OIDC discovery + JWKS ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct OidcMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
}

/// In-process cache of discovery documents keyed by issuer (10-minute TTL).
fn discovery_cache() -> &'static RwLock<HashMap<String, (Instant, OidcMetadata)>> {
    static CACHE: std::sync::OnceLock<RwLock<HashMap<String, (Instant, OidcMetadata)>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub async fn discover(issuer: &str) -> Result<OidcMetadata, String> {
    if let Some((at, md)) = discovery_cache().read().unwrap().get(issuer).cloned() {
        if at.elapsed() < Duration::from_secs(600) {
            return Ok(md);
        }
    }
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let md: OidcMetadata = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("discovery request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("discovery returned error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("discovery parse failed: {e}"))?;
    discovery_cache()
        .write()
        .unwrap()
        .insert(issuer.to_string(), (Instant::now(), md.clone()));
    Ok(md)
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
    #[serde(default)]
    alg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub id_token: String,
    #[serde(default)]
    pub access_token: Option<String>,
}

/// Claims we extract from the verified ID token.
#[derive(Debug, Deserialize)]
pub struct IdTokenClaims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
}

/// Exchange the authorization code for tokens at the IdP token endpoint.
pub async fn exchange_code(
    token_endpoint: &str,
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];
    reqwest::Client::new()
        .post(token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("token request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("token endpoint returned error: {e}"))?
        .json::<TokenResponse>()
        .await
        .map_err(|e| format!("token response parse failed: {e}"))
}

/// Verify an ID token against the IdP's JWKS, enforcing issuer, audience,
/// expiry, and the expected nonce. Returns the validated claims.
pub async fn verify_id_token(
    id_token: &str,
    jwks_uri: &str,
    issuer: &str,
    client_id: &str,
    expected_nonce: &str,
) -> Result<IdTokenClaims, String> {
    use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

    let header = decode_header(id_token).map_err(|e| format!("bad id_token header: {e}"))?;
    let kid = header.kid.ok_or("id_token missing kid")?;

    let jwks: Jwks = reqwest::Client::new()
        .get(jwks_uri)
        .send()
        .await
        .map_err(|e| format!("jwks request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("jwks parse failed: {e}"))?;

    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or("no matching JWKS key for id_token kid")?;
    if let Some(alg) = &jwk.alg {
        if alg != "RS256" {
            return Err(format!("unsupported JWKS alg: {alg}"));
        }
    }

    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| format!("invalid JWKS RSA key: {e}"))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[client_id]);

    let data = decode::<IdTokenClaims>(id_token, &key, &validation)
        .map_err(|e| format!("id_token verification failed: {e}"))?;
    let claims = data.claims;

    match &claims.nonce {
        Some(n) if n == expected_nonce => {}
        _ => return Err("id_token nonce mismatch".to_string()),
    }
    Ok(claims)
}

// ── Signed CSRF state (carries nonce + redirect, short-lived) ───────────────

fn state_secret() -> Vec<u8> {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production".to_string())
        .into_bytes()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SsoState {
    pub slug: String,
    pub nonce: String,
    pub exp: u64,
}

pub fn sign_state(slug: &str) -> Result<(String, String), String> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let nonce = uuid::Uuid::new_v4().to_string();
    let exp = unix_now() as u64 + 600; // 10 minutes
    let state = SsoState {
        slug: slug.to_string(),
        nonce: nonce.clone(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &state,
        &EncodingKey::from_secret(&state_secret()),
    )
    .map_err(|e| format!("state sign failed: {e}"))?;
    Ok((token, nonce))
}

pub fn verify_state(token: &str) -> Result<SsoState, String> {
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
    let validation = Validation::new(Algorithm::HS256);
    decode::<SsoState>(
        token,
        &DecodingKey::from_secret(&state_secret()),
        &validation,
    )
    .map(|d| d.claims)
    .map_err(|e| format!("invalid state: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip_carries_nonce_and_slug() {
        let (token, nonce) = sign_state("okta").unwrap();
        let st = verify_state(&token).unwrap();
        assert_eq!(st.slug, "okta");
        assert_eq!(st.nonce, nonce);
    }

    #[test]
    fn tampered_state_is_rejected() {
        let (token, _) = sign_state("okta").unwrap();
        let mut bad = token.clone();
        bad.push('x');
        assert!(verify_state(&bad).is_err());
    }

    #[tokio::test]
    async fn public_view_omits_secrets() {
        let store = MemorySsoStore::default();
        store
            .create(SsoConnection {
                id: "1".into(),
                tenant_id: "t".into(),
                slug: "okta".into(),
                provider: "Okta".into(),
                issuer: "https://example.okta.com".into(),
                client_id: "cid".into(),
                client_secret: "shhh".into(),
                scopes: "openid email".into(),
                enabled: true,
                created_at: unix_now(),
            })
            .await;
        let pubs = store.list_enabled_public().await;
        assert_eq!(pubs.len(), 1);
        assert_eq!(pubs[0].slug, "okta");
        // PublicSsoConnection has no secret field at all.
        let json = serde_json::to_string(&pubs[0]).unwrap();
        assert!(!json.contains("shhh"));
    }

    #[test]
    fn connection_serialization_skips_secret() {
        let c = SsoConnection {
            id: "1".into(),
            tenant_id: "t".into(),
            slug: "okta".into(),
            provider: "Okta".into(),
            issuer: "https://i".into(),
            client_id: "cid".into(),
            client_secret: "topsecret".into(),
            scopes: "openid".into(),
            enabled: true,
            created_at: 0,
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("topsecret"));
        assert!(json.contains("cid"));
    }
}
