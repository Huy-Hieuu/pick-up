# Phase 1 — External Integrations

Third-party services used in P1: SMS for OTP, Momo/ZaloPay for payments, Google Maps for court display, Zalo for game sharing.

---

## SMS Provider (OTP Delivery)

### Provider Options for Vietnam

| Provider | Pros | Cons |
|----------|------|------|
| **eSMS** | Vietnamese company, cheap, good local coverage | Limited docs in English |
| **SpeedSMS** | Simple API, competitive pricing | Smaller company |
| **Twilio** | Excellent docs, reliable, global | More expensive for VN numbers |

Recommendation: Start with eSMS for cost, abstract behind a trait to swap later.

### Trait Abstraction

```rust
// server/src/services/sms.rs
#[async_trait]
pub trait SmsProvider: Send + Sync {
    async fn send_otp(&self, phone: &str, code: &str) -> Result<(), AppError>;
}
```

### Console Provider (Development)

```rust
pub struct ConsoleSmsProvider;

impl SmsProvider for ConsoleSmsProvider {
    async fn send_otp(&self, phone: &str, code: &str) -> Result<(), AppError> {
        tracing::info!(phone, code, "OTP code (dev mode)");
        Ok(())
    }
}
```

Selected when `SMS_PROVIDER=console` in `.env`.

### eSMS Integration

```
POST https://rest.esms.vn/MainService.svc/json/SendMultipleMessage_V4_post_json/

{
  "ApiKey": "<api_key>",
  "Content": "Your PickUp code is {code}. Valid for 5 minutes.",
  "Phone": "0901234567",   // local format, no +84
  "SecretKey": "<secret>",
  "SmsType": "2",           // OTP type
  "Brandname": "PickUp"
}
```

**Config:**
```
SMS_PROVIDER=esms
SMS_API_KEY=<key>
SMS_SECRET_KEY=<secret>
SMS_BRANDNAME=PickUp
```

**Phone format:** Convert `+84901234567` → `0901234567` before sending to eSMS.

**Cost:** ~350–500 VND per SMS. Budget: ~500 OTPs/day at launch = ~200,000 VND/day.

---

## Momo Integration

### Overview

Momo is Vietnam's largest e-wallet. Integration uses their Payment Gateway API v2.

### Sandbox Setup

1. Register at [business.momo.vn](https://business.momo.vn)
2. Get sandbox credentials: `partner_code`, `access_key`, `secret_key`
3. Sandbox API: `https://test-payment.momo.vn/v2/gateway/api`
4. Production API: `https://payment.momo.vn/v2/gateway/api`

### Create Payment Order

```
POST https://test-payment.momo.vn/v2/gateway/api/create

{
  "partnerCode": "PICKUP",
  "partnerName": "PickUp",
  "storeId": "pickup_store",
  "requestId": "<unique-request-uuid>",
  "amount": 100000,
  "orderId": "<payment-uuid>",
  "orderInfo": "Game payment - Pickleball Thao Dien",
  "redirectUrl": "pickup://payment-result",
  "ipnUrl": "https://api.pickup.app/webhooks/momo",
  "lang": "vi",
  "requestType": "payWithMethod",
  "extraData": "<base64-encoded-game-id>",
  "signature": "<hmac-sha256>"
}
```

**Signature generation:**
```
raw = "accessKey={access_key}&amount={amount}&extraData={extra_data}&ipnUrl={ipn_url}&orderId={order_id}&orderInfo={order_info}&partnerCode={partner_code}&redirectUrl={redirect_url}&requestId={request_id}&requestType={request_type}"
signature = HMAC-SHA256(raw, secret_key)
```

**Response:**
```json
{
  "partnerCode": "PICKUP",
  "orderId": "payment-uuid",
  "requestId": "req-uuid",
  "amount": 100000,
  "responseTime": 1714560000000,
  "message": "Successful.",
  "resultCode": 0,
  "payUrl": "https://test-payment.momo.vn/..."
}
```

`payUrl` is what we return to the mobile app as `payment_url`.

### Webhook (IPN) Verification

```rust
fn verify_momo_signature(webhook: &MomoWebhook, secret_key: &str) -> bool {
    let raw = format!(
        "accessKey={}&amount={}&extraData={}&message={}&orderId={}&orderInfo={}&orderType={}&partnerCode={}&payType={}&requestId={}&responseTime={}&resultCode={}&transId={}",
        access_key, webhook.amount, webhook.extra_data, webhook.message,
        webhook.order_id, webhook.order_info, webhook.order_type,
        webhook.partner_code, webhook.pay_type, webhook.request_id,
        webhook.response_time, webhook.result_code, webhook.trans_id
    );
    let expected = hmac_sha256(raw.as_bytes(), secret_key.as_bytes());
    expected == webhook.signature
}
```

`resultCode == 0` means payment successful.

### Config

```
MOMO_PARTNER_CODE=PICKUP
MOMO_ACCESS_KEY=<key>
MOMO_SECRET_KEY=<key>
MOMO_API_URL=https://test-payment.momo.vn/v2/gateway/api
```

---

## ZaloPay Integration

### Overview

ZaloPay is Vietnam's second-largest e-wallet (by Zalo/VNG). Uses two secret keys: `key1` for creating orders, `key2` for verifying webhooks.

### Sandbox Setup

1. Register at [docs.zalopay.vn](https://docs.zalopay.vn)
2. Get sandbox credentials: `app_id`, `key1`, `key2`
3. Sandbox API: `https://sb-openapi.zalopay.vn/v2`
4. Production API: `https://openapi.zalopay.vn/v2`

### Create Payment Order

```
POST https://sb-openapi.zalopay.vn/v2/create

{
  "app_id": 1234,
  "app_user": "user-uuid",
  "app_time": 1714560000000,
  "app_trans_id": "260501_payment-uuid",
  "amount": 100000,
  "item": "[{\"name\":\"Game payment\"}]",
  "description": "PickUp - Pickleball Thao Dien",
  "embed_data": "{\"redirecturl\":\"pickup://payment-result\"}",
  "bank_code": "",
  "callback_url": "https://api.pickup.app/webhooks/zalopay",
  "mac": "<hmac-sha256>"
}
```

**MAC generation (using key1):**
```
raw = "{app_id}|{app_trans_id}|{app_user}|{amount}|{app_time}|{embed_data}|{item}"
mac = HMAC-SHA256(raw, key1)
```

**`app_trans_id` format:** `YYMMDD_<payment-uuid>` (ZaloPay requires date prefix).

**Response:**
```json
{
  "return_code": 1,
  "return_message": "Success",
  "order_url": "https://sb-openapi.zalopay.vn/...",
  "zp_trans_token": "token123"
}
```

`order_url` is what we return to the mobile app as `payment_url`.

### Webhook Verification

```rust
fn verify_zalopay_signature(webhook: &ZaloPayWebhook, key2: &str) -> bool {
    let expected = hmac_sha256(webhook.data.as_bytes(), key2.as_bytes());
    expected == webhook.mac
}
```

The `data` field is a JSON string that must be parsed after verification:
```json
{
  "app_id": 1234,
  "app_trans_id": "260501_payment-uuid",
  "app_time": 1714560000000,
  "amount": 100000,
  "zp_trans_id": 987654321,
  "server_time": 1714560001000,
  "channel": 38,
  "merchant_user_id": "",
  "user_fee_amount": 0,
  "discount_amount": 0
}
```

**Response to ZaloPay webhook:**
```json
{
  "return_code": 1,
  "return_message": "success"
}
```

### Config

```
ZALOPAY_APP_ID=1234
ZALOPAY_KEY1=<key1>
ZALOPAY_KEY2=<key2>
ZALOPAY_API_URL=https://sb-openapi.zalopay.vn/v2
```

---

## Google Maps (Client-Side)

### Purpose

Display court locations on a map in the Explore tab. All map rendering is client-side — the server only stores `lat`/`lng` coordinates.

### Expo Setup

Install the Google Maps package for Expo:

```bash
npx expo install react-native-maps
```

Configure in `app.json`:
```json
{
  "expo": {
    "plugins": [
      ["react-native-maps", {
        "googleMapsApiKey": "<GOOGLE_MAPS_API_KEY>"
      }]
    ]
  }
}
```

### API Key Restrictions

- Restrict to Android app (package name + SHA-1 fingerprint)
- Restrict to iOS app (bundle identifier)
- Enable only: Maps SDK for Android, Maps SDK for iOS
- Do NOT enable Geocoding API (court addresses are geocoded once during seeding)

### Usage in App

The `explore.tsx` screen uses `MapView` with `Marker` components for each court. Tapping a marker navigates to `court/[id].tsx`.

---

## Zalo Deeplink (Game Sharing)

### Purpose

Players share game invites via Zalo (Vietnam's dominant messaging app). Tapping the shared link opens the PickUp app directly to the game detail screen.

### URL Format

```
https://pickup.app/game/{game_id}
```

This is a universal link that:
1. If app is installed → opens `game/[id].tsx` via Expo Router deep linking
2. If app is not installed → redirects to app store

### Expo Router Deep Link Config

In `app.json`:
```json
{
  "expo": {
    "scheme": "pickup",
    "web": {
      "bundler": "metro"
    },
    "plugins": [
      ["expo-router", {
        "origin": "https://pickup.app"
      }]
    ]
  }
}
```

Expo Router automatically maps `https://pickup.app/game/{id}` to `app/game/[id].tsx`.

### Share Message

Generated by `GET /games/:id/share`:

```
Join my pickleball game at Pickleball Thao Dien on Thu May 1 at 6:00 PM! 🏓
3/6 spots left. Split: 100,000 VND each.

https://pickup.app/game/{id}
```

The mobile app uses `Share.share()` (React Native) to open the system share sheet, making it easy to send via Zalo, Messenger, iMessage, etc.

---

## Files to Implement

| File | Purpose |
|------|---------|
| `server/src/services/sms.rs` | SmsProvider trait + ConsoleSmsProvider + EsmsSmsProvider |
| `server/src/services/momo.rs` | Momo API client: create order, verify signature |
| `server/src/services/zalopay.rs` | ZaloPay API client: create order, verify signature |
| `mobile/app.json` | Google Maps plugin + deep link scheme config |

---

## Environment Variables Summary

```bash
# SMS
SMS_PROVIDER=console          # console | esms | speedsms
SMS_API_KEY=
SMS_SECRET_KEY=
SMS_BRANDNAME=PickUp

# Momo
MOMO_PARTNER_CODE=
MOMO_ACCESS_KEY=
MOMO_SECRET_KEY=
MOMO_API_URL=https://test-payment.momo.vn/v2/gateway/api

# ZaloPay
ZALOPAY_APP_ID=
ZALOPAY_KEY1=
ZALOPAY_KEY2=
ZALOPAY_API_URL=https://sb-openapi.zalopay.vn/v2

# Google Maps (mobile only, in app.json or .env for Expo)
GOOGLE_MAPS_API_KEY=

# App
APP_URL=https://pickup.app
```

---

## Related Docs

- [Auth Service](./p1-auth-service.md) — uses SMS provider for OTP
- [Payment Service](./p1-payment-service.md) — uses Momo/ZaloPay for payments and webhooks
- [Game Service](./p1-game-service.md) — uses Zalo deeplink for sharing
- [Mobile App](./p1-mobile-app.md) — uses Google Maps for court display
