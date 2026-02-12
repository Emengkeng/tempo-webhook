# 🚀 Tempo Webhook Service - Quick Start Guide

Get up and running in 5 minutes!

## Prerequisites

- **Rust 1.75+** - [Install Rust](https://rustup.rs/)
- **Docker** (recommended) or PostgreSQL + Redis + NATS
- **Polar Account** (for billing) - [Sign up at Polar](https://polar.sh)

## Option 1: Quick Start with Docker (Recommended)

### Step 1: Extract and Navigate

```bash
unzip tempo-webhooks-complete.zip
cd tempo-webhooks
```

### Step 2: Start Infrastructure

```bash
docker-compose up -d
```

This starts:
- PostgreSQL on port 5432
- Redis on port 6379
- NATS on port 4222

### Step 3: Run Setup Script

```bash
./setup.sh
```

This will:
- Create `.env` file with generated secrets
- Install `sqlx-cli`
- Run database migrations
- Build the project

### Step 4: Configure Polar Integration

Edit `.env` and add your Polar credentials:

```bash
POLAR_SECRET_KEY=sk_test_your_key_here
POLAR_WEBHOOK_SECRET=whsec_your_secret_here
```

Get these from: https://polar.sh/dashboard/settings/webhooks

### Step 5: Start the Service

```bash
cargo run
```

The service will start on `http://localhost:8080`

✅ **You're ready!** Jump to [Testing](#testing) below.

---

## Option 2: Manual Setup (Without Docker)

### Step 1: Install Dependencies

```bash
# macOS
brew install postgresql redis nats-server

# Ubuntu/Debian
sudo apt-get install postgresql redis-server
# Download NATS from https://github.com/nats-io/nats-server/releases

# Start services
pg_ctl start
redis-server &
nats-server &
```

### Step 2: Setup Project

```bash
unzip tempo-webhooks-complete.zip
cd tempo-webhooks

# Copy environment file
cp .env.example .env

# Generate secrets
export JWT_SECRET=$(openssl rand -hex 32)
export API_KEY_SECRET=$(openssl rand -hex 32)

# Update .env (use your favorite editor)
nano .env
```

### Step 3: Database Setup

```bash
# Create database
createdb tempo_webhooks

# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
sqlx migrate run
```

### Step 4: Configure and Run

Update `.env` with your settings, then:

```bash
cargo run
```

---

## Testing

### 1. Check Health

```bash
curl http://localhost:8080/health
```

Expected response:
```json
{
  "status": "healthy",
  "checks": [
    {"name": "database", "status": "up"},
    {"name": "redis", "status": "up"},
    {"name": "nats", "status": "up"}
  ]
}
```

### 2. Register an Account

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "securepassword123",
    "full_name": "Test User",
    "organization_name": "My Company",
    "organization_slug": "my-company"
  }'
```

Response includes your API key:
```json
{
  "user": {...},
  "organization": {...},
  "api_key": {
    "key": "tempo_live_abc123...",
    "key_prefix": "tempo_live_abc",
    ...
  }
}
```

**⚠️ Save this API key!** You'll need it for all API requests.

### 3. Create a Webhook Subscription

```bash
export API_KEY="tempo_live_abc123..."  # Your API key from registration

curl -X POST http://localhost:8080/api/v1/subscriptions \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{
    "type": "TRANSFER",
    "network": "mainnet",
    "address": "0x20c0000000000000000000000000000000000001",
    "webhook_url": "https://webhook.site/your-unique-url",
    "filters": [
      {"type": "amount_min", "value": "1000000"}
    ]
  }'
```

💡 **Tip**: Use [webhook.site](https://webhook.site) to test webhook delivery!

### 4. List Your Subscriptions

```bash
curl http://localhost:8080/api/v1/subscriptions \
  -H "X-API-Key: $API_KEY"
```

### 5. Check Webhook Logs

```bash
curl http://localhost:8080/api/v1/webhooks/logs \
  -H "X-API-Key: $API_KEY"
```

---

## What Happens Next?

1. **Indexer starts** monitoring the Tempo blockchain
2. When a transfer event matches your subscription:
   - Event is detected in real-time (~2 seconds)
   - Filters are applied
   - Webhook is sent to your URL with HMAC signature
   - Delivery is logged in the database

3. **Check your webhook receiver** (webhook.site or your endpoint)

---

## Verifying Webhook Signatures

In your application, verify webhooks like this:

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

// Express.js example
app.post('/webhooks/tempo', (req, res) => {
  const signature = req.headers['x-tempo-signature'];
  const secret = 'whsec_your_secret';
  
  if (!verifyWebhook(req.body, signature, secret)) {
    return res.status(401).send('Invalid signature');
  }
  
  console.log('Transfer received:', req.body);
  res.status(200).send('OK');
});
```

---

## Polar Billing Integration

### Setup Polar Webhooks

1. Go to https://polar.sh/dashboard/settings/webhooks
2. Add webhook endpoint: `https://your-domain.com/webhooks/polar`
3. Select events:
   - `subscription.created`
   - `subscription.updated`
   - `subscription.active`
   - `subscription.canceled`
   - `subscription.revoked`

4. Copy the webhook secret to your `.env`:
```bash
POLAR_WEBHOOK_SECRET=whsec_your_secret_here
```

### When a User Subscribes

When a customer subscribes via Polar:
1. Polar sends webhook to your service
2. Service updates the organization's plan tier
3. Quota limits are automatically applied

---

## Production Deployment

### Deploy to Fly.io

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
flyctl secrets set TEMPO_MAINNET_WS="wss://rpc.tempo.xyz"
flyctl secrets set TEMPO_MAINNET_HTTP="https://rpc.tempo.xyz"
flyctl secrets set JWT_SECRET="$(openssl rand -hex 32)"
flyctl secrets set API_KEY_ENCRYPTION_KEY="$(openssl rand -hex 32)"
flyctl secrets set POLAR_SECRET_KEY="sk_test_..."
flyctl secrets set POLAR_WEBHOOK_SECRET="whsec_..."
```

**Monthly cost**: ~$13
- Fly.io: $5
- Render PostgreSQL: $7
- Upstash Redis: $0 (free tier)
- Domain: $1

---

## Troubleshooting

### Database Connection Failed

```bash
# Check PostgreSQL is running
pg_isready

# Check connection string
echo $DATABASE_URL
```

### Redis Connection Failed

```bash
# Check Redis is running
redis-cli ping

# Should return: PONG
```

### NATS Connection Failed

```bash
# Check NATS is running
curl http://localhost:8222/healthz

# Should return: ok
```

### Migrations Failed

```bash
# Reset database
sqlx database drop
sqlx database create
sqlx migrate run
```

---

## Next Steps

- 📖 Read the [full README](README.md)
- 🏗️ Review the [Architecture](COMPLETE_CODE_PACKAGE.md)
- 🔧 Explore the [API Documentation](#api-endpoints)
- 🚀 Deploy to production

---

## Support

- **Issues**: Open a GitHub issue
- **Documentation**: See `/docs` folder
- **Email**: support@tempohooks.com

---

**Ready to build amazing real-time blockchain applications!** 🎉
