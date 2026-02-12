# Deployment & Infrastructure Guide

## Overview

Complete guide for deploying the Tempo Webhook Service on a **bootstrap budget** ($12-15/month). This guide assumes you're starting from scratch.

---

## Infrastructure Stack

```mermaid
graph TB
    subgraph "External Services (Free)"
        TEMPO[Tempo RPC<br/>wss://rpc.tempo.xyz<br/>$0/month]
        STRIPE[Stripe<br/>Payment Processing<br/>2.9% + 30¢]
    end
    
    subgraph "Hosting ($13/month)"
        RENDER[Render PostgreSQL<br/>2GB RAM, 20GB storage<br/>$7/month]
        UPSTASH[Upstash Redis<br/>100MB, 10k cmds/day<br/>$0/month Free Tier]
        FLY[Fly.io Rust App<br/>256MB RAM<br/>$5/month]
        CF[Cloudflare<br/>Domain + DNS<br/>$1/month]
    end
    
    subgraph "Monitoring (Optional)"
        SENTRY[Sentry<br/>Error Tracking<br/>$0 Free Tier]
        BETTER[BetterUptime<br/>Uptime Monitoring<br/>$0 Free Tier]
    end
    
    TEMPO --> FLY
    FLY --> RENDER
    FLY --> UPSTASH
    FLY --> STRIPE
    FLY --> SENTRY
    BETTER --> FLY
    
    style RENDER fill:#4F46E5
    style UPSTASH fill:#059669
    style FLY fill:#7C3AED
    style CF fill:#F97316
```

---

## Phase 1: Database Setup (Render PostgreSQL)

### 1.1 Create Database

1. Go to [render.com](https://render.com)
2. Sign up (free account)
3. Click "New +" → "PostgreSQL"
4. Configure:
   - **Name**: `tempo-webhooks-db`
   - **Database**: `tempo_webhooks`
   - **User**: `tempo_admin`
   - **Region**: `Oregon (US West)` (or closest to you)
   - **Plan**: `Starter ($7/month)`
   - **PostgreSQL Version**: `16`

5. Click "Create Database"
6. Save credentials:
   ```
   Internal Database URL: postgres://tempo_admin:xxx@xxx.oregon-postgres.render.com/tempo_webhooks
   External Database URL: postgres://tempo_admin:xxx@xxx.oregon-postgres.render.com/tempo_webhooks
   ```

### 1.2 Run Migrations

**Install sqlx-cli**:
```bash
cargo install sqlx-cli --no-default-features --features postgres
```

**Set environment variable**:
```bash
export DATABASE_URL="postgres://tempo_admin:xxx@xxx.oregon-postgres.render.com/tempo_webhooks"
```

**Create migrations**:
```bash
# Initialize migrations directory
sqlx migrate add initial_schema

# Copy the SQL from ARCHITECTURE.md into migrations/xxx_initial_schema.sql
```

**Run migrations**:
```bash
sqlx migrate run
```

**Verify**:
```bash
# Connect to database
psql $DATABASE_URL

# List tables
\dt

# Should see:
# organizations, users, subscription_plans, api_keys, subscriptions, 
# filters, indexed_blocks, transfer_events, webhook_logs, usage_records
```

---

## Phase 2: Redis Setup (Upstash)

### 2.1 Create Redis Instance

1. Go to [upstash.com](https://upstash.com)
2. Sign up (free account)
3. Click "Create Database"
4. Configure:
   - **Name**: `tempo-webhooks-redis`
   - **Type**: `Regional`
   - **Region**: `us-west-1` (Oregon)
   - **TLS**: Enabled
   - **Eviction**: Enabled

5. Copy connection details:
   ```
   UPSTASH_REDIS_REST_URL: https://xxx.upstash.io
   UPSTASH_REDIS_REST_TOKEN: xxx
   ```

### 2.2 Test Connection

```bash
# Using redis-cli
redis-cli -u redis://default:xxx@xxx.upstash.io:6379

# Test commands
> PING
PONG
> SET test "hello"
OK
> GET test
"hello"
```

---

## Phase 3: Application Deployment (Fly.io)

### 3.1 Install Fly CLI

```bash
# macOS
brew install flyctl

# Linux
curl -L https://fly.io/install.sh | sh

# Authenticate
flyctl auth login
```

### 3.2 Initialize Fly App

```bash
# In your project root
flyctl launch

# Answer prompts:
# App name: tempo-webhooks
# Region: sjc (San Jose) or closest
# PostgreSQL: No (we're using Render)
# Redis: No (we're using Upstash)
# Deploy now: No
```

This creates `fly.toml`:

```toml
app = "tempo-webhooks"
primary_region = "sjc"

[build]
  builder = "paketobuildpacks/builder:base"
  buildpacks = ["gcr.io/paketo-buildpacks/rust"]

[env]
  PORT = "8080"
  RUST_LOG = "info"

[[services]]
  internal_port = 8080
  protocol = "tcp"

  [[services.ports]]
    handlers = ["http"]
    port = 80
    force_https = true

  [[services.ports]]
    handlers = ["tls", "http"]
    port = 443

  [services.concurrency]
    type = "connections"
    hard_limit = 25
    soft_limit = 20

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 256
```

### 3.3 Set Secrets

```bash
# Database
flyctl secrets set DATABASE_URL="postgres://tempo_admin:xxx@xxx.oregon-postgres.render.com/tempo_webhooks"

# Redis
flyctl secrets set REDIS_URL="redis://default:xxx@xxx.upstash.io:6379"

# Tempo RPC endpoints
flyctl secrets set TEMPO_MAINNET_WS="wss://rpc.tempo.xyz"
flyctl secrets set TEMPO_MAINNET_HTTP="https://rpc.tempo.xyz"
flyctl secrets set TEMPO_TESTNET_WS="wss://rpc.moderato.tempo.xyz"
flyctl secrets set TEMPO_TESTNET_HTTP="https://rpc.moderato.tempo.xyz"

# Stripe (get from stripe.com dashboard)
flyctl secrets set STRIPE_SECRET_KEY="sk_test_xxx"
flyctl secrets set STRIPE_WEBHOOK_SECRET="whsec_xxx"

# JWT secret (generate random)
flyctl secrets set JWT_SECRET=$(openssl rand -hex 32)

# API key encryption (generate random)
flyctl secrets set API_KEY_ENCRYPTION_KEY=$(openssl rand -hex 32)
```

### 3.4 Create Dockerfile

**Dockerfile**:
```dockerfile
# Build stage
FROM rust:1.75-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/tempo-webhooks /usr/local/bin/tempo-webhooks

# Create non-root user
RUN useradd -m -u 1000 appuser && \
    chown -R appuser:appuser /usr/local/bin/tempo-webhooks

USER appuser

EXPOSE 8080

CMD ["tempo-webhooks"]
```

### 3.5 Deploy

```bash
# Deploy to Fly.io
flyctl deploy

# View logs
flyctl logs

# Check status
flyctl status

# Open in browser
flyctl open
```

---

## Phase 4: Domain Setup (Cloudflare)

### 4.1 Purchase Domain

1. Go to Cloudflare (or Namecheap, etc.)
2. Purchase domain (e.g., `tempohooks.com`)
3. Cost: ~$10-15/year

### 4.2 Configure DNS

**In Cloudflare dashboard**:

1. Add A record:
   ```
   Type: A
   Name: @
   Value: <Fly.io IP address>
   Proxy: Enabled (orange cloud)
   ```

2. Add CNAME for www:
   ```
   Type: CNAME
   Name: www
   Value: @
   Proxy: Enabled
   ```

3. Add CNAME for API:
   ```
   Type: CNAME
   Name: api
   Value: tempo-webhooks.fly.dev
   Proxy: Enabled
   ```

### 4.3 SSL Certificate

**In Fly.io**:
```bash
# Add custom domain
flyctl certs create api.tempohooks.com

# Verify
flyctl certs show api.tempohooks.com
```

**In Cloudflare**:
- SSL/TLS mode: `Full (strict)`
- Always Use HTTPS: `On`
- Automatic HTTPS Rewrites: `On`

---

## Phase 5: Monitoring Setup

### 5.1 Sentry (Error Tracking)

1. Sign up at [sentry.io](https://sentry.io) (free tier)
2. Create new project: `Rust`
3. Get DSN: `https://xxx@xxx.ingest.sentry.io/xxx`
4. Add to Fly secrets:
   ```bash
   flyctl secrets set SENTRY_DSN="https://xxx@xxx.ingest.sentry.io/xxx"
   ```

**In your code**:
```rust
use sentry;

fn main() {
    let _guard = sentry::init((
        env::var("SENTRY_DSN").unwrap_or_default(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some("production".into()),
            ..Default::default()
        },
    ));
    
    // Your app code
}
```

### 5.2 BetterUptime (Uptime Monitoring)

1. Sign up at [betteruptime.com](https://betteruptime.com) (free tier)
2. Add monitor:
   - **URL**: `https://api.tempohooks.com/health`
   - **Check interval**: 1 minute
   - **Timeout**: 10 seconds
   - **Expected status code**: 200

3. Add alert channels:
   - Email
   - Slack (optional)
   - PagerDuty (optional)

---

## Phase 6: Environment Variables

### 6.1 Complete Environment Config

**Create `.env.example`**:
```bash
# Database
DATABASE_URL=postgres://user:pass@localhost/tempo_webhooks
DATABASE_MAX_CONNECTIONS=10

# Redis
REDIS_URL=redis://localhost:6379
REDIS_MAX_CONNECTIONS=5

# Tempo RPC
TEMPO_MAINNET_WS=wss://rpc.tempo.xyz
TEMPO_MAINNET_HTTP=https://rpc.tempo.xyz
TEMPO_TESTNET_WS=wss://rpc.moderato.tempo.xyz
TEMPO_TESTNET_HTTP=https://rpc.moderato.tempo.xyz

# Server
PORT=8080
HOST=0.0.0.0
RUST_LOG=info,tempo_webhooks=debug

# Security
JWT_SECRET=<generate-with-openssl-rand-hex-32>
API_KEY_ENCRYPTION_KEY=<generate-with-openssl-rand-hex-32>

# Stripe
STRIPE_SECRET_KEY=sk_test_xxx
STRIPE_WEBHOOK_SECRET=whsec_xxx
STRIPE_PUBLISHABLE_KEY=pk_test_xxx

# Monitoring
SENTRY_DSN=https://xxx@xxx.ingest.sentry.io/xxx

# Features
ENABLE_MAINNET=true
ENABLE_TESTNET=true
CONFIRMATION_BLOCKS=1
```

### 6.2 Load in Application

**src/config.rs**:
```rust
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub tempo_mainnet_ws: String,
    pub tempo_mainnet_http: String,
    pub tempo_testnet_ws: String,
    pub tempo_testnet_http: String,
    pub port: u16,
    pub host: String,
    pub jwt_secret: String,
    pub stripe_secret_key: String,
    pub sentry_dsn: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::dotenv().ok(); // Load .env in development
        
        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            redis_url: env::var("REDIS_URL")?,
            tempo_mainnet_ws: env::var("TEMPO_MAINNET_WS")?,
            tempo_mainnet_http: env::var("TEMPO_MAINNET_HTTP")?,
            tempo_testnet_ws: env::var("TEMPO_TESTNET_WS")?,
            tempo_testnet_http: env::var("TEMPO_TESTNET_HTTP")?,
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            host: env::var("HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            jwt_secret: env::var("JWT_SECRET")?,
            stripe_secret_key: env::var("STRIPE_SECRET_KEY")?,
            sentry_dsn: env::var("SENTRY_DSN").ok(),
        })
    }
}
```

---

## Phase 7: CI/CD Setup (GitHub Actions)

### 7.1 Create Workflow

**.github/workflows/deploy.yml**:
```yaml
name: Deploy to Fly.io

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: postgres
          POSTGRES_DB: tempo_webhooks_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6379:6379
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Cache target
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-target-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run tests
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost/tempo_webhooks_test
          REDIS_URL: redis://localhost:6379
        run: cargo test --verbose
      
      - name: Run clippy
        run: cargo clippy -- -D warnings
      
      - name: Check formatting
        run: cargo fmt -- --check

  deploy:
    needs: test
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main' && github.event_name == 'push'
    
    steps:
      - uses: actions/checkout@v3
      
      - uses: superfly/flyctl-actions/setup-flyctl@master
      
      - name: Deploy to Fly.io
        run: flyctl deploy --remote-only
```

### 7.2 Add Secrets to GitHub

1. Go to repository → Settings → Secrets
2. Add:
   - `FLY_API_TOKEN`: Get from `flyctl auth token`

---

## Phase 8: Database Backups

### 8.1 Automated Backups (Render)

**Render automatically backs up your database**:
- Daily backups (retained for 7 days on Starter plan)
- Point-in-time recovery available

**Manual backup**:
```bash
# Backup to local file
pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql

# Restore from backup
psql $DATABASE_URL < backup_20260212.sql
```

### 8.2 Backup to S3 (Optional)

**Install AWS CLI**:
```bash
brew install awscli
aws configure
```

**Backup script** (`scripts/backup-db.sh`):
```bash
#!/bin/bash
set -e

BACKUP_FILE="tempo_webhooks_$(date +%Y%m%d_%H%M%S).sql.gz"
S3_BUCKET="s3://my-backups/tempo-webhooks/"

# Dump and compress
pg_dump $DATABASE_URL | gzip > $BACKUP_FILE

# Upload to S3
aws s3 cp $BACKUP_FILE $S3_BUCKET

# Keep only last 30 days locally
find . -name "tempo_webhooks_*.sql.gz" -mtime +30 -delete

echo "Backup complete: $BACKUP_FILE"
```

**Cron job** (on your local machine or a cheap VPS):
```bash
# Run daily at 2 AM
0 2 * * * /path/to/scripts/backup-db.sh
```

---

## Phase 9: Scaling Checklist

### When to Scale Up

| Metric | Threshold | Action |
|--------|-----------|--------|
| **Database connections** | >80% of max | Increase `DATABASE_MAX_CONNECTIONS` |
| **CPU usage** | >70% sustained | Upgrade Fly.io plan to 512MB RAM |
| **Memory usage** | >200MB | Upgrade Fly.io plan |
| **Database size** | >15GB | Upgrade Render plan to $15/month |
| **Redis usage** | >80MB | Upgrade to Upstash paid ($10/month) |
| **Active users** | >500 orgs | Consider multi-instance deployment |

### Scaling Strategies

**Horizontal Scaling** (Add more instances):
```bash
# Scale to 2 instances
flyctl scale count 2

# Auto-scaling (requires higher plan)
flyctl autoscale set min=1 max=3
```

**Vertical Scaling** (Bigger instances):
```bash
# Upgrade to 512MB RAM
flyctl scale vm shared-cpu-1x --memory 512

# Upgrade to dedicated CPU
flyctl scale vm dedicated-cpu-1x
```

---

## Phase 10: Health Checks

### 10.1 Health Endpoint

**src/routes/health.rs**:
```rust
use axum::{Extension, Json};
use serde_json::json;
use sqlx::PgPool;
use redis::Client as RedisClient;

pub async fn health_check(
    Extension(db): Extension<PgPool>,
    Extension(redis): Extension<RedisClient>,
) -> Json<serde_json::Value> {
    let mut checks = vec![];
    
    // Check database
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&db)
        .await
        .is_ok();
    checks.push(("database", db_healthy));
    
    // Check Redis
    let mut redis_conn = redis.get_async_connection().await;
    let redis_healthy = if let Ok(mut conn) = redis_conn {
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .is_ok()
    } else {
        false
    };
    checks.push(("redis", redis_healthy));
    
    // Check RPC (optional - adds latency)
    // let rpc_healthy = check_tempo_rpc().await;
    // checks.push(("rpc", rpc_healthy));
    
    let all_healthy = checks.iter().all(|(_, status)| *status);
    
    Json(json!({
        "status": if all_healthy { "healthy" } else { "unhealthy" },
        "checks": checks.iter().map(|(name, status)| {
            json!({
                "name": name,
                "status": if *status { "up" } else { "down" }
            })
        }).collect::<Vec<_>>(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
```

---

## Cost Breakdown Summary

| Service | Plan | Monthly Cost | Notes |
|---------|------|--------------|-------|
| **PostgreSQL** | Render Starter | $7.00 | 2GB RAM, 20GB storage |
| **Redis** | Upstash Free | $0.00 | 100MB, 10k commands/day |
| **App Hosting** | Fly.io | $5.00 | 256MB RAM, 1 instance |
| **Domain** | Cloudflare | $1.00 | .com domain (~$12/year) |
| **SSL** | Cloudflare | $0.00 | Free with Cloudflare |
| **Tempo RPC** | Public | $0.00 | Free public endpoints |
| **Error Tracking** | Sentry Free | $0.00 | 5k events/month |
| **Uptime Monitoring** | BetterUptime | $0.00 | 10 monitors |
| **Total** | | **$13.00/month** | |

**First month setup costs**:
- Domain registration: ~$12 (one-time)
- Total first month: ~$25

---

## Troubleshooting

### Database Connection Issues

**Error**: `connection pool exhausted`
```bash
# Increase max connections
flyctl secrets set DATABASE_MAX_CONNECTIONS=20

# Restart app
flyctl apps restart tempo-webhooks
```

### Redis Connection Issues

**Error**: `connection refused`
```bash
# Check Redis status in Upstash dashboard
# Verify REDIS_URL is correct
flyctl secrets set REDIS_URL="redis://default:xxx@xxx.upstash.io:6379"
```

### Deployment Failures

**Error**: `build failed`
```bash
# Check logs
flyctl logs

# Build locally to debug
docker build -t tempo-webhooks .
docker run -p 8080:8080 tempo-webhooks
```

---

**Last Updated**: February 12, 2026