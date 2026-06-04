// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Non-OIDC SSO adapters for Chinese providers whose login is a custom OAuth2
//! flow rather than standard OpenID Connect:
//!
//! - **Feishu / Lark** (`feishu`)        — passport.feishu.cn OAuth2
//! - **DingTalk** (`dingtalk`)           — login.dingtalk.com OAuth2 (v1.0 API)
//! - **WeChat Work** (`wechat_work`)     — qyapi.weixin.qq.com corp app login
//!
//! Standard-OIDC IdPs (Alibaba Cloud IDaaS, Huawei OneAccess, Tencent Cloud,
//! Authing, Okta, Azure AD, …) do NOT come through here — they use the OIDC
//! path in `sso.rs`.
//!
//! Each adapter exposes the same two operations: build the IdP authorize URL,
//! and exchange the returned `code` for the end user's identity. The network
//! calls follow each vendor's documented API; the URL construction and response
//! parsing are pure and unit-tested.

use serde_json::Value;

/// Identity resolved from a provider's user-info endpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct OAuthUserInfo {
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// True if `kind` is one of the custom-OAuth2 providers handled here.
pub fn is_oauth_kind(kind: &str) -> bool {
    matches!(kind, "feishu" | "dingtalk" | "wechat_work")
}

/// Minimal percent-encoding for URL query values.
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build the IdP authorization URL to redirect the browser to. Returns `None`
/// when `kind` is not a custom-OAuth2 provider.
pub fn authorize_url(
    kind: &str,
    client_id: &str,
    agent_id: Option<&str>,
    redirect_uri: &str,
    state: &str,
) -> Option<String> {
    match kind {
        "feishu" => Some(format!(
            "https://passport.feishu.cn/suite/passport/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}",
            enc(client_id),
            enc(redirect_uri),
            enc(state),
        )),
        "dingtalk" => Some(format!(
            "https://login.dingtalk.com/oauth2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid&prompt=consent&state={}",
            enc(client_id),
            enc(redirect_uri),
            enc(state),
        )),
        "wechat_work" => Some(format!(
            "https://login.work.weixin.qq.com/wwlogin/sso/login?login_type=CorpApp&appid={}&agentid={}&redirect_uri={}&state={}",
            enc(client_id),
            enc(agent_id.unwrap_or("")),
            enc(redirect_uri),
            enc(state),
        )),
        _ => None,
    }
}

/// Exchange the authorization `code` for the user's identity, dispatching to the
/// right vendor adapter.
pub async fn fetch_user(
    kind: &str,
    client_id: &str,
    client_secret: &str,
    agent_id: Option<&str>,
    code: &str,
    redirect_uri: &str,
) -> Result<OAuthUserInfo, String> {
    match kind {
        "feishu" => feishu_fetch_user(client_id, client_secret, code, redirect_uri).await,
        "dingtalk" => dingtalk_fetch_user(client_id, client_secret, code).await,
        "wechat_work" => wechat_work_fetch_user(client_id, client_secret, agent_id, code).await,
        _ => Err(format!("unsupported OAuth provider kind: {kind}")),
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// ── Feishu / Lark ───────────────────────────────────────────────────────────

async fn feishu_fetch_user(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OAuthUserInfo, String> {
    // 1. code → user access_token
    let token: Value = client()
        .post("https://passport.feishu.cn/suite/passport/oauth/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| format!("feishu token request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("feishu token parse failed: {e}"))?;
    let access = token
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("feishu token response missing access_token")?;

    // 2. access_token → userinfo
    let info: Value = client()
        .get("https://passport.feishu.cn/suite/passport/oauth/userinfo")
        .bearer_auth(access)
        .send()
        .await
        .map_err(|e| format!("feishu userinfo request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("feishu userinfo parse failed: {e}"))?;
    parse_feishu_userinfo(&info)
}

fn parse_feishu_userinfo(v: &Value) -> Result<OAuthUserInfo, String> {
    let subject = v
        .get("open_id")
        .or_else(|| v.get("union_id"))
        .and_then(Value::as_str)
        .ok_or("feishu userinfo missing open_id")?
        .to_string();
    let email = v
        .get("enterprise_email")
        .or_else(|| v.get("email"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let name = v.get("name").and_then(Value::as_str).map(str::to_string);
    Ok(OAuthUserInfo {
        subject,
        email,
        name,
    })
}

// ── DingTalk ────────────────────────────────────────────────────────────────

async fn dingtalk_fetch_user(
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<OAuthUserInfo, String> {
    // 1. code → user access token
    let token: Value = client()
        .post("https://api.dingtalk.com/v1.0/oauth2/userAccessToken")
        .json(&serde_json::json!({
            "clientId": client_id,
            "clientSecret": client_secret,
            "code": code,
            "grantType": "authorization_code",
        }))
        .send()
        .await
        .map_err(|e| format!("dingtalk token request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("dingtalk token parse failed: {e}"))?;
    let access = token
        .get("accessToken")
        .and_then(Value::as_str)
        .ok_or("dingtalk token response missing accessToken")?;

    // 2. access token → contact/users/me
    let info: Value = client()
        .get("https://api.dingtalk.com/v1.0/contact/users/me")
        .header("x-acs-dingtalk-access-token", access)
        .send()
        .await
        .map_err(|e| format!("dingtalk userinfo request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("dingtalk userinfo parse failed: {e}"))?;
    parse_dingtalk_userinfo(&info)
}

fn parse_dingtalk_userinfo(v: &Value) -> Result<OAuthUserInfo, String> {
    let subject = v
        .get("openId")
        .or_else(|| v.get("unionId"))
        .and_then(Value::as_str)
        .ok_or("dingtalk userinfo missing openId")?
        .to_string();
    let email = v
        .get("email")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let name = v.get("nick").and_then(Value::as_str).map(str::to_string);
    Ok(OAuthUserInfo {
        subject,
        email,
        name,
    })
}

// ── WeChat Work (企业微信) ───────────────────────────────────────────────────

async fn wechat_work_fetch_user(
    corp_id: &str,
    corp_secret: &str,
    _agent_id: Option<&str>,
    code: &str,
) -> Result<OAuthUserInfo, String> {
    // 1. corp credentials → access_token
    let tok: Value = client()
        .get(format!(
            "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
            enc(corp_id),
            enc(corp_secret)
        ))
        .send()
        .await
        .map_err(|e| format!("wechat_work gettoken failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("wechat_work gettoken parse failed: {e}"))?;
    let access = tok
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("wechat_work missing access_token")?;

    // 2. access_token + code → userid
    let id: Value = client()
        .get(format!(
            "https://qyapi.weixin.qq.com/cgi-bin/auth/getuserinfo?access_token={}&code={}",
            enc(access),
            enc(code)
        ))
        .send()
        .await
        .map_err(|e| format!("wechat_work getuserinfo failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("wechat_work getuserinfo parse failed: {e}"))?;
    let userid = id
        .get("userid")
        .or_else(|| id.get("UserId"))
        .and_then(Value::as_str)
        .ok_or("wechat_work getuserinfo missing userid")?
        .to_string();

    // 3. userid → user detail (name + email)
    let detail: Value = client()
        .get(format!(
            "https://qyapi.weixin.qq.com/cgi-bin/user/get?access_token={}&userid={}",
            enc(access),
            enc(&userid)
        ))
        .send()
        .await
        .map_err(|e| format!("wechat_work user/get failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("wechat_work user/get parse failed: {e}"))?;
    parse_wechat_work_user(&userid, &detail)
}

fn parse_wechat_work_user(userid: &str, v: &Value) -> Result<OAuthUserInfo, String> {
    let email = v
        .get("biz_mail")
        .or_else(|| v.get("email"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let name = v.get("name").and_then(Value::as_str).map(str::to_string);
    Ok(OAuthUserInfo {
        subject: userid.to_string(),
        email,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn authorize_urls_are_provider_correct() {
        let fs = authorize_url("feishu", "cli_app", None, "https://app/cb", "ST").unwrap();
        assert!(fs.starts_with("https://passport.feishu.cn/suite/passport/oauth/authorize?"));
        assert!(fs.contains("client_id=cli_app"));
        assert!(fs.contains("redirect_uri=https%3A%2F%2Fapp%2Fcb"));
        assert!(fs.contains("state=ST"));

        let dt = authorize_url("dingtalk", "ding123", None, "https://app/cb", "ST").unwrap();
        assert!(dt.starts_with("https://login.dingtalk.com/oauth2/auth?"));
        assert!(dt.contains("client_id=ding123"));
        assert!(dt.contains("scope=openid"));

        let ww = authorize_url(
            "wechat_work",
            "corpid",
            Some("1000002"),
            "https://app/cb",
            "ST",
        )
        .unwrap();
        assert!(ww.contains("appid=corpid"));
        assert!(ww.contains("agentid=1000002"));

        assert!(authorize_url("oidc", "x", None, "y", "z").is_none());
    }

    #[test]
    fn is_oauth_kind_classifies() {
        assert!(is_oauth_kind("feishu"));
        assert!(is_oauth_kind("dingtalk"));
        assert!(is_oauth_kind("wechat_work"));
        assert!(!is_oauth_kind("oidc"));
    }

    #[test]
    fn feishu_userinfo_prefers_enterprise_email() {
        let v = json!({
            "open_id": "ou_123",
            "name": "Alice",
            "email": "personal@x.com",
            "enterprise_email": "alice@corp.com"
        });
        let u = parse_feishu_userinfo(&v).unwrap();
        assert_eq!(u.subject, "ou_123");
        assert_eq!(u.email.as_deref(), Some("alice@corp.com"));
        assert_eq!(u.name.as_deref(), Some("Alice"));
    }

    #[test]
    fn dingtalk_userinfo_maps_nick_and_openid() {
        let v = json!({ "openId": "abc", "nick": "Bob", "email": "" });
        let u = parse_dingtalk_userinfo(&v).unwrap();
        assert_eq!(u.subject, "abc");
        assert_eq!(u.name.as_deref(), Some("Bob"));
        // empty email is treated as None
        assert_eq!(u.email, None);
    }

    #[test]
    fn wechat_work_uses_biz_mail_and_userid_subject() {
        let v = json!({ "name": "Carol", "biz_mail": "carol@corp.cn" });
        let u = parse_wechat_work_user("zhangsan", &v).unwrap();
        assert_eq!(u.subject, "zhangsan");
        assert_eq!(u.email.as_deref(), Some("carol@corp.cn"));
        assert_eq!(u.name.as_deref(), Some("Carol"));
    }
}
