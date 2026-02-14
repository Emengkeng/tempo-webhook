use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub full_name: Option<String>,
    pub role: String, // owner, admin, developer, viewer
    pub created_at: DateTime<Utc>,
    pub active: bool,
    pub email_verified: bool,
    #[serde(skip_serializing)]
    pub email_verification_token: Option<String>,
    #[serde(skip_serializing)]
    pub email_verification_token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub key_prefix: String,
    pub name: Option<String>,
    pub network: String, // mainnet, testnet, both
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
    pub request_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: Option<String>,
    pub network: String,
    pub expires_in_days: Option<i32>,
}

impl User {
    pub async fn create(
        pool: &sqlx::PgPool,
        org_id: Uuid,
        email: String,
        password_hash: String,
        full_name: Option<String>,
        role: String,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (organization_id, email, password_hash, full_name, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, organization_id, email, password_hash, full_name, role, created_at, active,email_verified, email_verification_token,
                      email_verification_token_expires_at
            "#,
            org_id,
            email,
            password_hash,
            full_name,
            role
        )
        .fetch_one(pool)
        .await
    }

    pub async fn set_verification_token(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        token: String,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE users 
            SET email_verification_token = $1,
                email_verification_token_expires_at = $2
            WHERE id = $3
            "#,
            token,
            expires_at,
            user_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn verify_email(
        pool: &sqlx::PgPool,
        token: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            UPDATE users 
            SET email_verified = true,
                email_verification_token = NULL,
                email_verification_token_expires_at = NULL
            WHERE email_verification_token = $1
              AND email_verification_token_expires_at > NOW()
              AND active = true
            RETURNING id, organization_id, email, password_hash, full_name, role,
                      created_at, active, email_verified, email_verification_token,
                      email_verification_token_expires_at
            "#,
            token
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn get_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id, organization_id, email, password_hash, full_name, role,
                      created_at, active, email_verified, email_verification_token,
                      email_verification_token_expires_at
               FROM users WHERE email = $1 AND active = true"#,
            email
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn get_by_id(pool: &sqlx::PgPool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id, organization_id, email, password_hash, full_name, role,
                      created_at, active, email_verified, email_verification_token,
                      email_verification_token_expires_at
               FROM users WHERE id = $1 AND active = true"#,
            id
        )
        .fetch_optional(pool)
        .await
    }
}

impl ApiKey {
    pub async fn create(
        pool: &sqlx::PgPool,
        org_id: Uuid,
        user_id: Uuid,
        key_hash: String,
        key_prefix: String,
        name: Option<String>,
        network: String,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ApiKey,
            r#"
            INSERT INTO api_keys (
                organization_id, user_id, key_hash, key_prefix, name, network, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING 
                id, organization_id, user_id, key_hash, key_prefix, 
                name, network, active, created_at, expires_at, last_used, request_count
            "#,
            org_id,
            user_id,
            key_hash,
            key_prefix,
            name,
            network,
            expires_at
        )
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_hash(pool: &sqlx::PgPool, key_hash: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            ApiKey,
            r#"
            SELECT 
                id, organization_id, user_id, key_hash, key_prefix, 
                name, network, active, created_at, expires_at, last_used, request_count
            FROM api_keys 
            WHERE key_hash = $1 AND active = true
            "#,
            key_hash
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn update_last_used(pool: &sqlx::PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE api_keys SET last_used = NOW(), request_count = request_count + 1 WHERE id = $1",
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_by_organization(
        pool: &sqlx::PgPool,
        org_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            ApiKey,
            r#"
            SELECT 
                id, organization_id, user_id, key_hash, key_prefix, 
                name, network, active, created_at, expires_at, last_used, request_count
            FROM api_keys 
            WHERE organization_id = $1 AND active = true
            ORDER BY created_at DESC
            "#,
            org_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(pool: &sqlx::PgPool, id: Uuid, org_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE api_keys SET active = false WHERE id = $1 AND organization_id = $2",
            id,
            org_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
