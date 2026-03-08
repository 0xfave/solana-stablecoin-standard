# Backend API Reference

This document provides a complete reference for the SSS Backend API.

---

## Table of Contents

- [Base URL](#base-url)
- [Response Format](#response-format)
- [Health](#health)
- [Info](#info)
- [Mint](#mint)
- [Burn](#burn)
- [Events](#events)
- [Compliance](#compliance)
- [Webhooks](#webhooks)
- [Error Codes](#error-codes)

---

## Base URL

```
http://localhost:3000
```

---

## Response Format

All responses follow this format:

```json
{
  "success": true,
  "data": { ... }
}
```

Error response:

```json
{
  "success": false,
  "error": "Error message"
}
```

---

## Health

### GET /health

Health check endpoint.

**Response:**

```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "tier": "SSS-2",
    "program_id": "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw",
    "mint": "YourMintAddress123...",
    "rpc_url": "https://api.devnet.solana.com",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

---

## Info

### GET /api/info

Get program information.

**Response:**

```json
{
  "success": true,
  "data": {
    "program_id": "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw",
    "mint": "YourMintAddress123...",
    "tier": "SSS-2",
    "decimals": 6,
    "supply_cap": 1000000000000,
    "current_supply": 50000000000,
    "paused": false
  }
}
```

---

## Mint

### POST /api/mint

Create a mint request (fiat on-ramp).

**Request:**

```json
{
  "user_wallet": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "amount": 1000000,
  "fiat_tx_id": "fiat_123456",
  "custodian": "CustodianPubkey123..."
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "id": "mint_abc123",
    "user_wallet": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
    "amount": 1000000,
    "fiat_tx_id": "fiat_123456",
    "custodian": "CustodianPubkey123...",
    "requested_at": "2024-01-15T10:30:00Z",
    "status": "Pending",
    "signature": null,
    "confirmed_at": null,
    "error": null
  }
}
```

### GET /api/mint/:id

Get mint request by ID.

**Response:**

```json
{
  "success": true,
  "data": {
    "id": "mint_abc123",
    "user_wallet": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
    "amount": 1000000,
    "fiat_tx_id": "fiat_123456",
    "status": "Confirmed",
    "signature": "5abc123...",
    "confirmed_at": "2024-01-15T10:31:00Z"
  }
}
```

### GET /api/mint/wallet/:wallet

Get all mint requests for a wallet.

**Response:**

```json
{
  "success": true,
  "data": {
    "requests": [
      {
        "id": "mint_abc123",
        "amount": 1000000,
        "status": "Confirmed",
        "requested_at": "2024-01-15T10:30:00Z"
      }
    ]
  }
}
```

---

## Burn

### POST /api/burn

Create a burn request (fiat off-ramp).

**Request:**

```json
{
  "user_wallet": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "token_account": "TokenAccountAddress123...",
  "amount": 1000000,
  "fiat_destination": "bank_account_123",
  "custodian": "CustodianPubkey123..."
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "id": "burn_xyz789",
    "user_wallet": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
    "token_account": "TokenAccountAddress123...",
    "amount": 1000000,
    "fiat_destination": "bank_account_123",
    "status": "Pending"
  }
}
```

### GET /api/burn/:id

Get burn request by ID.

### GET /api/burn/wallet/:wallet

Get all burn requests for a wallet.

---

## Events

### GET /api/events

Query indexed events.

**Query Parameters:**

| Parameter | Type   | Description                |
| --------- | ------ | -------------------------- |
| `type`    | string | Filter by event type       |
| `from`    | string | Start date (ISO)           |
| `to`      | string | End date (ISO)             |
| `limit`   | number | Max results (default: 100) |
| `offset`  | number | Pagination offset          |

**Example:**

```
GET /api/events?type=TokensMinted&from=2024-01-01&limit=50
```

**Response:**

```json
{
  "success": true,
  "data": {
    "events": [
      {
        "id": "evt_123",
        "event_type": "TokensMinted",
        "signature": "5abc123...",
        "slot": 123456789,
        "timestamp": "2024-01-15T10:30:00Z",
        "data": {
          "amount": 1000000,
          "recipient": "7xKXtg2..."
        }
      }
    ],
    "total": 1
  }
}
```

### GET /api/events/:signature

Get events by transaction signature.

---

## Compliance

### GET /api/compliance/check/:address

Check an address against compliance rules.

**Response:**

```json
{
  "success": true,
  "data": {
    "address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
    "allowed": true,
    "reason": null,
    "rules_triggered": [],
    "risk_score": 0
  }
}
```

Blocked address:

```json
{
  "success": true,
  "data": {
    "address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
    "allowed": false,
    "reason": "Blacklisted: OFAC sanctions",
    "rules_triggered": ["blacklist"],
    "risk_score": 100
  }
}
```

### POST /api/compliance/check-tx

Check a transaction before submission.

**Request:**

```json
{
  "from": "SenderAddress...",
  "to": "ReceiverAddress...",
  "amount": 1000000
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "allowed": true,
    "reason": null
  }
}
```

### GET /api/compliance/blacklist

Get all blacklist entries.

**Response:**

```json
{
  "success": true,
  "data": {
    "entries": [
      {
        "address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
        "reason": "OFAC sanctions",
        "blacklister": "BlacklisterPubkey...",
        "timestamp": "2024-01-15T10:30:00Z",
        "status": "Active"
      }
    ]
  }
}
```

### POST /api/compliance/blacklist

Add address to blacklist.

**Request:**

```json
{
  "address": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "reason": "OFAC sanctions"
}
```

### DELETE /api/compliance/blacklist/:address

Remove address from blacklist.

### GET /api/compliance/rules

Get active compliance rules.

**Response:**

```json
{
  "success": true,
  "data": {
    "rules": [
      {
        "name": "blacklist",
        "enabled": true,
        "description": "Block blacklisted addresses"
      },
      {
        "name": "allowlist",
        "enabled": true,
        "description": "Require receiver on allowlist"
      }
    ]
  }
}
```

### GET /api/compliance/audit

Export audit trail.

**Query Parameters:**

| Parameter | Type   | Description       |
| --------- | ------ | ----------------- |
| `from`    | string | Start date (ISO)  |
| `to`      | string | End date (ISO)    |
| `type`    | string | Event type filter |

**Example:**

```
GET /api/compliance/audit?from=2024-01-01&to=2024-12-31
```

**Response:**

```json
{
  "success": true,
  "data": {
    "events": [
      {
        "event_type": "AddedToBlacklist",
        "signature": "5abc123...",
        "timestamp": "2024-01-15T10:30:00Z",
        "data": {
          "address": "7xKXtg2...",
          "reason": "OFAC sanctions"
        }
      }
    ],
    "total": 1,
    "from": "2024-01-01",
    "to": "2024-12-31"
  }
}
```

### GET /api/compliance/stats

Get compliance statistics.

**Response:**

```json
{
  "success": true,
  "data": {
    "total_events": 150,
    "total_mints": 50,
    "total_burns": 30,
    "total_blacklist": 5,
    "total_seized": 0,
    "paused_count": 0
  }
}
```

---

## Webhooks

### POST /webhook

Receive events from external systems (Helius, etc.).

**Headers:**

```
Content-Type: application/json
X-Webhook-Secret: your_webhook_secret
```

**Request:**

```json
{
  "type": "TRANSFER",
  "signature": "5abc123...",
  "slot": 123456789,
  "data": {
    "from": "SenderAddress...",
    "to": "ReceiverAddress...",
    "amount": 1000000
  }
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "processed": true
  }
}
```

---

## Error Codes

| Code                      | Description                   |
| ------------------------- | ----------------------------- |
| `INVALID_REQUEST`         | Malformed request body        |
| `UNAUTHORIZED`            | Invalid API key or signature  |
| `NOT_FOUND`               | Resource not found            |
| `COMPLIANCE_CHECK_FAILED` | Address failed compliance     |
| `TRANSACTION_FAILED`      | Blockchain transaction failed |
| `INTERNAL_ERROR`          | Server error                  |

**Error Response:**

```json
{
  "success": false,
  "error": "COMPLIANCE_CHECK_FAILED",
  "details": "Address is blacklisted"
}
```
