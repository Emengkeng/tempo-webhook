use lettre::{
    message::{header::ContentType, Message},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use crate::error::{AppError, AppResult};

pub struct EmailService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from_email: String,
    base_url: String,
}

impl EmailService {
    pub fn new(
        smtp_host: String,
        smtp_port: u16,
        smtp_username: String,
        smtp_password: String,
        from_email: String,
        base_url: String,
    ) -> AppResult<Self> {
        let creds = Credentials::new(smtp_username, smtp_password);
        
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)
            .map_err(|e| AppError::Internal(format!("SMTP configuration error: {}", e)))?
            .port(smtp_port)
            .credentials(creds)
            .build();

        Ok(Self {
            mailer,
            from_email,
            base_url,
        })
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        verification_token: &str,
    ) -> AppResult<()> {
        let verification_link = format!(
            "{}/verify-email?token={}",
            self.base_url, verification_token
        );

        let html_body = format!(
            r#"
            <html>
                <body>
                    <h2>Welcome to Eventop Tempo Webhooks!</h2>
                    <p>Please verify your email address by clicking the link below:</p>
                    <p><a href="{}">{}</a></p>
                    <p>This link will expire in 24 hours.</p>
                    <p>If you didn't create an account, you can safely ignore this email.</p>
                </body>
            </html>
            "#,
            verification_link, verification_link
        );

        let email = Message::builder()
            .from(self.from_email.parse().map_err(|e| {
                AppError::Internal(format!("Invalid from email: {}", e))
            })?)
            .to(to_email.parse().map_err(|e| {
                AppError::Internal(format!("Invalid to email: {}", e))
            })?)
            .subject("Verify your email address")
            .header(ContentType::TEXT_HTML)
            .body(html_body)
            .map_err(|e| AppError::Internal(format!("Failed to build email: {}", e)))?;

        self.mailer
            .send(email)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {}", e)))?;

        Ok(())
    }
}