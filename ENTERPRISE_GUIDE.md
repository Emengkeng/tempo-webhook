# Enterprise Customer Management Guide

## Overview

For enterprise customers who need custom quotas beyond standard plans, admins can configure organization-specific limits that override plan defaults.

## Admin API Endpoints

### Create Custom Quota
```http
POST /admin/v1/quotas
{
  "organization_id": "uuid",
  "max_subscriptions": 10000,
  "max_webhook_deliveries": 1000000,
  "max_api_requests_per_minute": 5000,
  "notes": "Enterprise - Acme Corp"
}
```

### Update Custom Quota
```http
PUT /admin/v1/quotas/{org_id}
{
  "max_subscriptions": 20000
}
```

### List All Custom Quotas
```http
GET /admin/v1/quotas
```

### Delete Custom Quota
```http
DELETE /admin/v1/quotas/{org_id}
```

## How It Works

1. **Custom quotas override plan limits**
2. **Partial overrides supported** - only set limits you want to customize
3. **NULL values** use plan defaults
4. **Immediate effect** - no restart needed

## Example: Enterprise Customer Setup

```bash
# Create unlimited subscriptions for enterprise customer
POST /admin/v1/quotas
{
  "organization_id": "customer-uuid",
  "max_subscriptions": 100000,
  "max_webhook_deliveries": 10000000,
  "notes": "Enterprise contract #12345"
}
```

Customer immediately gets custom limits. System automatically enforces them.

See `API_DOCUMENTATION.md` for full details.
