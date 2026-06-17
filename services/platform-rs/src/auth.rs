// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// User role within a tenant. Defaults to `Editor` when not present in JWT.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Role {
    Viewer,
    #[default]
    Editor,
    Admin,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Editor => "editor",
            Role::Admin => "admin",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "editor" => Ok(Role::Editor),
            "viewer" => Ok(Role::Viewer),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub exp: u64,
    #[serde(default)]
    pub role: Role,
    /// Set when authenticated via email/password (user ID).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Set when authenticated via email/password.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl Claims {
    pub fn is_admin(&self) -> bool {
        self.role >= Role::Admin
    }
    pub fn can_write(&self) -> bool {
        self.role >= Role::Editor
    }
    pub fn can_read(&self) -> bool {
        true
    }
}

impl Default for Claims {
    fn default() -> Self {
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 3600;
        Self {
            sub: String::new(),
            tenant_id: String::new(),
            workspace_id: String::new(),
            project_id: String::new(),
            exp,
            role: Role::default(),
            user_id: None,
            email: None,
        }
    }
}

fn jwt_secret() -> Vec<u8> {
    std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production".to_string())
        .into_bytes()
}

pub fn sign_token(claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(&jwt_secret()),
    )
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
            role: Role::Admin,
            ..Default::default()
        };
        let token = sign_token(&claims).unwrap();
        let decoded = verify_token(&token).unwrap();
        assert_eq!(decoded.tenant_id, "tenant-1");
        assert_eq!(decoded.role, Role::Admin);
    }

    #[test]
    fn invalid_token_returns_none() {
        assert!(verify_token("not.a.token").is_none());
    }

    #[test]
    fn default_role_is_editor() {
        let role: Role = Default::default();
        assert_eq!(role, Role::Editor);
    }

    #[test]
    fn role_ordering_is_correct() {
        assert!(Role::Admin > Role::Editor);
        assert!(Role::Editor > Role::Viewer);
    }

    #[test]
    fn viewer_cannot_write() {
        let claims = Claims {
            sub: "u".to_string(),
            tenant_id: "t".to_string(),
            workspace_id: "w".to_string(),
            project_id: "p".to_string(),
            exp: u64::MAX,
            role: Role::Viewer,
            ..Default::default()
        };
        assert!(!claims.can_write());
        assert!(claims.can_read());
    }

    #[test]
    fn token_without_role_defaults_to_editor() {
        // Simulate old token (no role field) by using serde default
        let json =
            r#"{"sub":"u","tenant_id":"t","workspace_id":"w","project_id":"p","exp":9999999999}"#;
        let claims: Claims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.role, Role::Editor);
    }
}
