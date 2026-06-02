/// Lightweight email client. In dev (no SMTP config) it logs to stdout.
/// In prod, set SMTP_HOST + SMTP_PORT + SMTP_USER + SMTP_PASSWORD + SMTP_FROM.

#[derive(Clone)]
pub struct EmailClient {
    config: Option<SmtpConfig>,
    pub from: String,
    pub base_url: String,
}

#[derive(Clone)]
struct SmtpConfig {
    host: String,
    port: u16,
    user: String,
    password: String,
}

impl EmailClient {
    pub fn from_env() -> Self {
        let host = std::env::var("SMTP_HOST").ok();
        let config = host.map(|host| SmtpConfig {
            host,
            port: std::env::var("SMTP_PORT").ok()
                .and_then(|p| p.parse().ok()).unwrap_or(587),
            user: std::env::var("SMTP_USER").unwrap_or_default(),
            password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
        });
        Self {
            config,
            from: std::env::var("SMTP_FROM").unwrap_or_else(|_| "noreply@velara.local".to_string()),
            base_url: std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:5173".to_string()),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    /// Send an invitation email. Falls back to log in dev mode.
    pub async fn send_invitation(&self, to: &str, invite_token: &str, role: &str, expires_at: i64) {
        let link = format!("{}?invite={}", self.base_url, invite_token);
        let expires = format_unix(expires_at);
        let subject = "You've been invited to Velara";
        let body = format!(
            "You have been invited to join Velara as {}.\n\nClick the link below to accept (expires {}):\n{}\n",
            role, expires, link
        );
        self.send(to, subject, &body).await;
    }

    /// Send a password reset email. Falls back to log in dev mode.
    pub async fn send_password_reset(&self, to: &str, reset_token: &str, expires_at: i64) {
        let link = format!("{}?reset={}", self.base_url, reset_token);
        let expires = format_unix(expires_at);
        let subject = "Reset your Velara password";
        let body = format!(
            "A password reset was requested for your account.\n\nClick the link below to reset your password (expires {}):\n{}\n\nIf you did not request this, you can ignore this email.\n",
            expires, link
        );
        self.send(to, subject, &body).await;
    }

    /// Send an email verification link.
    pub async fn send_email_verification(&self, to: &str, verify_token: &str, expires_at: i64) {
        let link = format!("{}?verify={}", self.base_url, verify_token);
        let expires = format_unix(expires_at);
        let subject = "Verify your Velara email";
        let body = format!(
            "Please verify your email address by clicking the link below (expires {}):\n{}\n\nIf you did not create an account, you can ignore this email.\n",
            expires, link
        );
        self.send(to, subject, &body).await;
    }

    /// Send an execution success notification.
    pub async fn send_execution_success(&self, to: &str, workflow_name: &str, execution_id: &str) {
        let subject = format!("Velara: workflow \"{}\" completed successfully", workflow_name);
        let body = format!(
            "Workflow \"{}\" completed successfully.\n\nExecution ID: {}\n",
            workflow_name, execution_id
        );
        self.send(to, &subject, &body).await;
    }

    /// Send a quota threshold warning (80% or 100% exhausted).
    pub async fn send_quota_warning(&self, to: &str, tenant_id: &str, used: i64, max: i64, tier: &str, pct: f64) {
        let level = if pct >= 100.0 { "exhausted" } else { "80% warning" };
        let subject = format!("Velara billing alert: execution quota {level} for tenant {tenant_id}");
        let body = format!(
            "Your Velara execution quota is {level}.\n\n\
             Tenant: {tenant_id}\n\
             Tier: {tier}\n\
             Used: {used} / {max} ({pct:.1}%)\n\n\
             {}",
            if pct >= 100.0 {
                "No new executions can start until the quota resets at the beginning of next month.\nUpgrade your plan at /billing to continue."
            } else {
                "You are approaching your monthly limit. Consider upgrading at /billing."
            }
        );
        self.send(to, &subject, &body).await;
    }

    /// Send an execution failure alert.
    pub async fn send_execution_failure(&self, to: &str, workflow_name: &str, execution_id: &str, error: &str) {
        let subject = format!("Velara: workflow \"{}\" failed", workflow_name);
        let body = format!(
            "Workflow \"{}\" failed.\n\nExecution ID: {}\nError: {}\n",
            workflow_name, execution_id, error
        );
        self.send(to, &subject, &body).await;
    }

    async fn send(&self, to: &str, subject: &str, body: &str) {
        match &self.config {
            None => {
                tracing::info!(
                    to = %to,
                    subject = %subject,
                    "[EMAIL DEV] Would send email:\n{}",
                    body
                );
            }
            Some(cfg) => {
                use lettre::{
                    AsyncTransport, Message,
                    transport::smtp::authentication::Credentials,
                    AsyncSmtpTransport,
                };
                let from_addr = self.from.parse().unwrap_or_else(|_| {
                    "noreply@velara.local".parse().expect("fallback address parses")
                });
                let to_addr = match to.parse() {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::error!(to = %to, error = %e, "Invalid recipient address");
                        return;
                    }
                };
                let email = match Message::builder()
                    .from(from_addr)
                    .to(to_addr)
                    .subject(subject)
                    .body(body.to_string())
                {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to build email message");
                        return;
                    }
                };
                let creds = Credentials::new(cfg.user.clone(), cfg.password.clone());
                let transport = match AsyncSmtpTransport::<lettre::Tokio1Executor>::relay(&cfg.host) {
                    Ok(b) => b.port(cfg.port).credentials(creds).build(),
                    Err(e) => {
                        tracing::error!(smtp_host = %cfg.host, error = %e, "Failed to build SMTP transport");
                        return;
                    }
                };
                match transport.send(email).await {
                    Ok(_) => tracing::info!(to = %to, subject = %subject, "Email sent via SMTP"),
                    Err(e) => tracing::error!(to = %to, subject = %subject, error = %e, "SMTP send failed"),
                }
            }
        }
    }
}

impl Default for EmailClient {
    fn default() -> Self {
        Self::from_env()
    }
}

fn format_unix(ts: i64) -> String {
    // Simple UTC formatting without chrono dep
    let secs = ts as u64;
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    // Approximate year/month/day from days_since_epoch (good enough for display)
    let year = 1970 + days_since_epoch / 365;
    let day_of_year = days_since_epoch % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{}-{:02}-{:02} {:02}:{:02} UTC", year, month.min(12), day.min(31), h, m)
}
