use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub exp: u64,
}

fn jwt_secret() -> Vec<u8> {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production".to_string())
        .into_bytes()
}

pub fn sign_token(claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
    encode(&Header::default(), claims, &EncodingKey::from_secret(&jwt_secret()))
}

pub fn verify_token(token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(&jwt_secret()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_token() {
        let claims = Claims {
            sub: "user-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            project_id: "project-1".to_string(),
            exp: u64::MAX,
        };
        let token = sign_token(&claims).unwrap();
        let decoded = verify_token(&token).unwrap();
        assert_eq!(decoded.tenant_id, "tenant-1");
        assert_eq!(decoded.workspace_id, "workspace-1");
    }

    #[test]
    fn invalid_token_returns_none() {
        assert!(verify_token("not.a.token").is_none());
    }
}
