# Tempo Webhook Service

Real-time blockchain webhook notification service for Tempo with self-indexing architecture and Polar billing integration.

## 🚀 Features

- ✅ **Self-indexing** - Zero external API costs (no IndexSupply needed)
- ✅ **Direct Tempo RPC integration** - Free public endpoints
- ✅ **Sub-second webhook latency** (~500-700ms)
- ✅ **Advanced filtering** - Amount, address, memo pattern matching
- ✅ **Battle-tested retry logic** - Exponential backoff
- ✅ **Reorg protection** - Handles blockchain reorganizations
- ✅ **Polar billing integration** - Subscription management via webhooks
- ✅ **Multi-tenancy** - Organization-based isolation
- ✅ **Bootstrap-friendly** - $12-15/month infrastructure cost

## 📋 Prerequisites

- **Rust** 1.75 or higher
- **PostgreSQL** 16+
- **Redis** 7+
- **NATS Server** (for message queue)
- **Tempo RPC access** (free public endpoints)

## 🛠️ Installation

### 1. Clone and Setup

```bash
# Clone the repository
git clone https://github.com/Emengkeng/tempo-webhook.git
cd tempo-webhook

# Copy environment file
cp .env.example .env

# Edit .env with your configuration
nano .env
```

### 2. Generate Secrets

```bash
# Generate JWT secret
openssl rand -hex 32

# Generate API key encryption key
openssl rand -hex 32
```

Add these to your `.env` file.

### 3. Setup Database

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
sqlx migrate run
```

### 4. Start Services

Make sure you have Redis and NATS running:

```bash
# Redis (using Docker)
docker run -d -p 6379:6379 redis:7

# NATS (using Docker)
docker run -d -p 4222:4222 nats:latest
```

### 5. Run the Application

```bash
# Development
cargo run

# Production
cargo build --release
./target/release/tempo-webhooks
```

## 📖 API Usage

### Authentication

All API requests require an API key in the header:

```bash
X-API-Key: tempo_live_xxxxxxxxxxxxx
```

### Create a Subscription

```bash
curl -X POST https://api.tempohooks.com/api/v1/subscriptions \
  -H "X-API-Key: tempo_live_xxxxx" \
  -H "Content-Type: application/json" \
  -d '{
    "type": "TRANSFER",
    "network": "mainnet",
    "address": "0x20c0000000000000000000000000000000000001",
    "webhook_url": "https://myapp.com/webhooks",
    "webhook_secret": "whsec_xxxxx",
    "filters": [
      {"type": "amount_min", "value": "1000000"}
    ]
  }'
```

### List Subscriptions

```bash
curl https://api.tempohooks.com/api/v1/subscriptions \
  -H "X-API-Key: tempo_live_xxxxx"
```

### Get Webhook Logs

```bash
curl https://api.tempohooks.com/api/v1/webhooks/logs?subscription_id=sub_abc123 \
  -H "X-API-Key: tempo_live_xxxxx"
```

## 🔒 Webhook Verification

Verify webhook signatures in your application:

```javascript
const crypto = require('crypto');

function verifyWebhook(payload, signature, secret) {
  const [timestampPart, hashPart] = signature.split(',');
  const timestamp = timestampPart.split('=')[1];
  const hash = hashPart.split('=')[1];
  
  // Prevent replay attacks (5 minute window)
  if (Date.now() / 1000 - timestamp > 300) {
    return false;
  }
  
  const signedPayload = `${timestamp}.${JSON.stringify(payload)}`;
  const expectedHash = crypto
    .createHmac('sha256', secret)
    .update(signedPayload)
    .digest('hex');
  
  return crypto.timingSafeEqual(
    Buffer.from(hash),
    Buffer.from(expectedHash)
  );
}
```

## 🚢 Deployment

### Fly.io (Recommended)

```bash
# Install Fly CLI
brew install flyctl

# Login
flyctl auth login

# Deploy
flyctl deploy

# Set secrets
flyctl secrets set DATABASE_URL="postgres://..."
flyctl secrets set REDIS_URL="redis://..."
flyctl secrets set JWT_SECRET="..."
# ... etc
```

### Docker

```bash
# Build
docker build -t tempo-webhooks .

# Run
docker run -p 8080:8080 \
  -e DATABASE_URL="postgres://..." \
  -e REDIS_URL="redis://..." \
  tempo-webhooks
```

## 📊 Infrastructure Costs

Bootstrap budget (monthly):

| Service | Provider | Cost |
|---------|----------|------|
| PostgreSQL | Render.com | $7 |
| Redis | Upstash | $0 (free tier) |
| App Hosting | Fly.io | $5 |
| Domain | Cloudflare | $1 |
| **Total** | | **$13/month** |

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run with logs
RUST_LOG=debug cargo test

# Run specific test
cargo test test_webhook_delivery
```

## 📝 Environment Variables

See `.env.example` for all required environment variables.

### Required Variables

- `DATABASE_URL` - PostgreSQL connection string
- `REDIS_URL` - Redis connection string
- `TEMPO_MAINNET_WS` - Tempo mainnet WebSocket URL
- `TEMPO_MAINNET_HTTP` - Tempo mainnet HTTP RPC URL
- `JWT_SECRET` - Secret for JWT tokens
- `API_KEY_ENCRYPTION_KEY` - Secret for API key encryption
- `POLAR_SECRET_KEY` - Polar API key
- `POLAR_WEBHOOK_SECRET` - Polar webhook secret

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Tempo Blockchain                        │
│              (wss://rpc.tempo.xyz)                          │
└──────────────────────┬──────────────────────────────────────┘
                       │ WebSocket
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  WebSocket Listener                         │
│              (Real-time block notifications)                │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   Self-Indexer                              │
│        (Selective indexing + Event parsing)                 │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  Event Matcher                              │
│             (Apply filters + Match subscriptions)           │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                Webhook Dispatcher                           │
│         (HMAC signatures + Retry logic)                     │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  User Applications                          │
│                 (Receive webhooks)                          │
└─────────────────────────────────────────────────────────────┘
```

## 🤝 Contributing

Contributions welcome! Please open an issue or PR.
