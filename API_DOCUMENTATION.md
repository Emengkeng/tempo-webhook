# Tempo Webhook Service - Complete API Documentation

## Table of Contents
1. [Authentication](#authentication)
2. [Rate Limits & Quotas](#rate-limits--quotas)
3. [Plans & Pricing](#plans--pricing)
4. [Endpoints](#endpoints)
5. [Error Codes](#error-codes)
6. [Webhook Verification](#webhook-verification)

---

## Authentication

All API requests require an API key in the header:

```
X-API-Key: tempo_live_xxxxxxxxxxxxx
```

### Get Your API Key

1. Register an account: `POST /auth/register`
2. API key returned in response
3. Or create additional keys: `POST /api/v1/api-keys`

---

## Rate Limits & Quotas

### Plan Limits

| Feature | Free | Starter | Pro | Enterprise |
|---------|------|---------|-----|------------|
| **Subscriptions** | 10 | 100 | 1,000 | Unlimited |
| **Webhooks/month** | 1,000 | 10,000 | 100,000 | Unlimited |
| **API requests/minute** | 10 | 100 | 1,000 | 10,000 |
| **Mainnet access** | ❌ | ✅ | ✅ | ✅ |
| **Testnet access** | ✅ | ✅ | ✅ | ✅ |
| **Filters/subscription** | 3 | 5 | 10 | 20 |

### Rate Limit Headers

Responses include:
- `X-RateLimit-Limit`: Max requests per minute
- `X-RateLimit-Remaining`: Remaining requests
- `X-RateLimit-Reset`: Unix timestamp when limit resets

### Quota Errors

```json
{
  "error": "Subscription limit reached",
  "details": "Your free plan allows 10 subscriptions. Please upgrade or remove inactive subscriptions."
}
```

**Status Code**: `400 Bad Request` or `429 Too Many Requests`

---

## Plans & Pricing

### Free Plan
- **$0/month**
- Perfect for testing and development
- Testnet only
- Community support

### Starter Plan
- **$29/month**
- For small production apps
- Mainnet + Testnet
- Email support

### Pro Plan
- **$99/month**
- For growing businesses
- Higher limits
- Priority support
- Custom retention

### Enterprise Plan
- **Custom pricing**
- Unlimited everything
- Dedicated support
- SLA guarantee
- On-premise option

---

## Endpoints

### Authentication

#### Register
```http
POST /auth/register
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "securepass123",
  "full_name": "John Doe",
  "organization_name": "My Company",
  "organization_slug": "my-company"
}
```

**Response** (201 Created):
```json
{
  "user": { "id": "...", "email": "..." },
  "organization": { "id": "...", "name": "..." },
  "api_key": {
    "key": "tempo_live_abc123...",
    "key_prefix": "tempo_live_abc"
  }
}
```

#### Login
```http
POST /auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "securepass123"
}
```

---

### Subscriptions

#### Create Subscription
```http
POST /api/v1/subscriptions
X-API-Key: tempo_live_xxx
Content-Type: application/json

{
  "type": "TRANSFER",
  "network": "mainnet",
  "address": "0x20c0000000000000000000000000000000000001",
  "webhook_url": "https://myapp.com/webhooks",
  "filters": [
    {"type": "amount_min", "value": "1000000"},
    {"type": "from_address", "value": "0x..."}
  ]
}
```

**Validations**:
- ✅ Checks subscription quota for your plan
- ✅ Validates network access (mainnet requires paid plan)
- ✅ Checks filter count against plan limits
- ✅ Validates Ethereum addresses
- ✅ Validates webhook URL format

**Response** (201 Created):
```json
{
  "id": "sub_abc123",
  "type": "TRANSFER",
  "network": "mainnet",
  "active": true,
  "filters": [...]
}
```

**Errors**:
- `400`: Quota exceeded, invalid input, mainnet not allowed
- `429`: Rate limit exceeded

#### List Subscriptions
```http
GET /api/v1/subscriptions?limit=50&offset=0
X-API-Key: tempo_live_xxx
```

#### Update Subscription
```http
PATCH /api/v1/subscriptions/{id}
X-API-Key: tempo_live_xxx

{
  "active": false
}
```

#### Delete Subscription
```http
DELETE /api/v1/subscriptions/{id}
X-API-Key: tempo_live_xxx
```

---

### Webhooks & Usage

#### List Webhook Logs
```http
GET /api/v1/webhooks/logs?subscription_id=sub_xxx&limit=100
X-API-Key: tempo_live_xxx
```

**Response**:
```json
{
  "logs": [
    {
      "id": "log_xxx",
      "status": "delivered",
      "http_status_code": 200,
      "attempt_count": 1,
      "latency_ms": 145,
      "delivered_at": "2026-02-13T10:30:00Z"
    }
  ],
  "total": 1243,
  "has_more": true
}
```

#### Get Usage Statistics
```http
GET /api/v1/usage?start_date=2026-02-01&end_date=2026-02-28
X-API-Key: tempo_live_xxx
```

**Response**:
```json
{
  "period": {
    "start": "2026-02-01",
    "end": "2026-02-28"
  },
  "usage": {
    "webhook_deliveries": 5432,
    "api_requests": 1234,
    "active_subscriptions": 23
  },
  "quota": {
    "webhook_deliveries": 10000,
    "overage": 0,
    "overage_cost": 0.00
  },
  "plan": "starter"
}
```

---

### Plans & Quotas

#### Get Plan Info
```http
GET /api/v1/plan
X-API-Key: tempo_live_xxx
```

**Response**:
```json
{
  "current_plan": {
    "tier": "starter",
    "status": "active",
    "price_usd": 29.0
  },
  "usage": {
    "active_subscriptions": 23,
    "webhook_deliveries_this_month": 5432,
    "api_requests_this_minute": 12
  },
  "limits": {
    "max_subscriptions": 100,
    "max_webhook_deliveries": 10000,
    "max_api_requests_per_minute": 100,
    "can_use_mainnet": true
  },
  "available_plans": [...]
}
```

#### Check Quota Warnings
```http
GET /api/v1/quota/warnings
X-API-Key: tempo_live_xxx
```

**Response**:
```json
[
  {
    "warning_type": "webhooks",
    "message": "You're using 85% of your monthly webhook quota",
    "current_usage": 8500,
    "limit": 10000,
    "percentage_used": 85.0
  }
]
```

---

### API Keys

#### Create API Key
```http
POST /api/v1/api-keys
X-API-Key: tempo_live_xxx

{
  "name": "Production Key",
  "network": "both",
  "expires_in_days": 365
}
```

#### List API Keys
```http
GET /api/v1/api-keys
X-API-Key: tempo_live_xxx
```

#### Delete API Key
```http
DELETE /api/v1/api-keys/{id}
X-API-Key: tempo_live_xxx
```

---

## Error Codes

| Code | Meaning | Action |
|------|---------|--------|
| `400` | Bad Request | Check request format, quota limits |
| `401` | Unauthorized | Check API key, key expiration |
| `404` | Not Found | Resource doesn't exist |
| `429` | Too Many Requests | Rate limit exceeded, wait and retry |
| `500` | Internal Server Error | Contact support |

### Common Error Responses

**Quota Exceeded**:
```json
{
  "error": "Subscription limit reached",
  "details": "Your free plan allows 10 subscriptions."
}
```

**Mainnet Not Allowed**:
```json
{
  "error": "Bad Request",
  "details": "Your plan does not include mainnet access. Please upgrade to Starter or higher."
}
```

**Rate Limit Exceeded**:
```json
{
  "error": "Rate limit exceeded",
  "details": "Too many requests"
}
```

**Invalid API Key**:
```json
{
  "error": "Unauthorized",
  "details": "Invalid API key"
}
```

---

## Webhook Verification

### Verify Signatures

All webhooks include an HMAC signature in the `X-Tempo-Signature` header:

```
X-Tempo-Signature: t=1708876543,v1=5257a869e7ecebeda32affa62cdca3fa...
```

#### Node.js Example

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

---

## Best Practices

### 1. Monitor Your Quotas
```javascript
// Check quota warnings periodically
const warnings = await fetch('/api/v1/quota/warnings', {
  headers: { 'X-API-Key': apiKey }
});

if (warnings.length > 0) {
  console.warn('Approaching quota limits:', warnings);
  // Notify admin, consider upgrade
}
```

### 2. Handle Rate Limits
```javascript
async function makeRequest(url, options) {
  const response = await fetch(url, options);
  
  if (response.status === 429) {
    const resetTime = response.headers.get('X-RateLimit-Reset');
    const waitTime = resetTime - Date.now() / 1000;
    
    await new Promise(resolve => setTimeout(resolve, waitTime * 1000));
    return makeRequest(url, options); // Retry
  }
  
  return response;
}
```

### 3. Implement Idempotency
```javascript
// Store webhook IDs to prevent duplicate processing
const processedWebhooks = new Set();

app.post('/webhooks/tempo', (req, res) => {
  const webhookId = req.body.transactionHash;
  
  if (processedWebhooks.has(webhookId)) {
    return res.status(200).send('Already processed');
  }
  
  processedWebhooks.add(webhookId);
  // Process webhook...
});
```

---

## Support

- **Documentation**: https://docs.tempohooks.com
- **API Status**: https://status.tempohooks.com
- **Email**: support@tempohooks.com
- **Discord**: https://discord.gg/tempohooks

---

**Last Updated**: February 13, 2026
**API Version**: v1
