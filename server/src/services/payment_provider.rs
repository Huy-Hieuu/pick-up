//! Signed MoMo and ZaloPay requests. Secrets and callback contents never enter logs.
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    config::settings::Settings,
    error::{AppError, AppResult},
    models::payment::{PaymentProvider, PaymentStatus},
};

type HmacSha256 = Hmac<Sha256>;
const ORDER_TTL_SECONDS: i64 = 900;

#[derive(Debug, Clone, sqlx::FromRow)]
pub(super) struct Reservation {
    pub id: uuid::Uuid,
    pub amount: i32,
    pub provider: PaymentProvider,
    pub status: PaymentStatus,
    pub provider_txn_id: Option<String>,
    pub provider_order_id: String,
    pub provider_payment_url: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub provider_create_claim: Option<uuid::Uuid>,
    pub provider_create_claim_until: Option<chrono::DateTime<chrono::Utc>>,
    pub dispute_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ProviderCredentials {
    pub endpoint: String,
    pub key: String,
    pub app_id: String,
    pub partner_code: String,
    pub redirect_url: String,
    pub callback_url: String,
}

pub(super) fn credentials(
    settings: &Settings,
    provider: PaymentProvider,
) -> AppResult<ProviderCredentials> {
    let (endpoint, key, app_id, partner_code) = match provider {
        PaymentProvider::Momo => (
            &settings.momo.endpoint,
            &settings.momo.secret_key,
            String::new(),
            settings.momo.partner_code.clone(),
        ),
        PaymentProvider::Zalopay => (
            &settings.zalopay.endpoint,
            &settings.zalopay.key1,
            settings.zalopay.app_id.clone(),
            String::new(),
        ),
    };
    if key.trim().is_empty()
        || match provider {
            PaymentProvider::Momo => {
                settings.momo.access_key.trim().is_empty() || partner_code.trim().is_empty()
            }
            PaymentProvider::Zalopay => {
                settings.zalopay.key2.trim().is_empty() || app_id.trim().is_empty()
            }
        }
    {
        return Err(AppError::Unimplemented(
            "Payment provider credentials are not configured".into(),
        ));
    }
    validate_endpoint(endpoint)?;
    Ok(ProviderCredentials {
        endpoint: endpoint.clone(),
        key: key.clone(),
        app_id,
        partner_code,
        redirect_url: format!("{}/payment-result", settings.app.public_app_url),
        callback_url: format!(
            "{}/webhooks/{}",
            settings.app.public_api_url,
            match provider {
                PaymentProvider::Momo => "momo",
                PaymentProvider::Zalopay => "zalopay",
            }
        ),
    })
}

fn validate_endpoint(value: &str) -> AppResult<url::Url> {
    let url = url::Url::parse(value).map_err(|_| AppError::PaymentGateway)?;
    let local_http =
        url.scheme() == "http" && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if (url.scheme() != "https" && !local_http)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::PaymentGateway);
    }
    Ok(url)
}

fn hmac_hex(key: &str, message: &str) -> AppResult<String> {
    let mut mac =
        HmacSha256::new_from_slice(key.as_bytes()).map_err(|_| AppError::PaymentGateway)?;
    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

pub(crate) fn verify_hmac_hex(key: &str, message: &str, received: &str) -> bool {
    let Ok(signature) = hex::decode(received) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(key.as_bytes()) else {
        return false;
    };
    mac.update(message.as_bytes());
    mac.verify_slice(&signature).is_ok()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MomoCreateRequest<'a> {
    partner_code: &'a str,
    partner_name: &'static str,
    store_id: &'static str,
    request_id: &'a str,
    amount: i64,
    order_id: &'a str,
    order_info: &'static str,
    redirect_url: &'a str,
    ipn_url: &'a str,
    request_type: &'static str,
    extra_data: &'static str,
    order_expire_time: i32,
    auto_capture: bool,
    lang: &'static str,
    signature: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MomoCreateResponse {
    partner_code: String,
    request_id: String,
    order_id: String,
    amount: i64,
    result_code: i32,
    pay_url: Option<String>,
}

fn momo_result_is_uncertain(result_code: i32) -> bool {
    // MoMo documents these as in-progress, duplicate, or otherwise unresolved
    // outcomes. Retrying with a new order can charge the customer twice.
    result_code != 0
        && !matches!(
            result_code,
            20 | 21 | 22 | 98 | 1001..=1007 | 1017 | 1026 | 4001 | 4002 | 4100
        )
}

pub(super) async fn create_momo_order(
    settings: &Settings,
    reservation: &Reservation,
) -> AppResult<String> {
    let cfg = credentials(settings, PaymentProvider::Momo)?;
    let request_id = reservation.id.simple().to_string();
    let amount = i64::from(reservation.amount);
    let order_id = reservation.provider_order_id.as_str();
    let order_info = "PickUp game bill";
    let raw = format!(
        "accessKey={}&amount={amount}&extraData=&ipnUrl={}&orderId={order_id}&orderInfo={order_info}&partnerCode={}&redirectUrl={}&requestId={request_id}&requestType=payWithMethod",
        settings.momo.access_key, cfg.callback_url, cfg.partner_code, cfg.redirect_url,
    );
    let body = MomoCreateRequest {
        partner_code: &cfg.partner_code,
        partner_name: "PickUp",
        store_id: "pickup",
        request_id: &request_id,
        amount,
        order_id,
        order_info,
        redirect_url: &cfg.redirect_url,
        ipn_url: &cfg.callback_url,
        request_type: "payWithMethod",
        extra_data: "",
        order_expire_time: (ORDER_TTL_SECONDS / 60) as i32,
        auto_capture: true,
        lang: "vi",
        signature: hmac_hex(&cfg.key, &raw)?,
    };
    let response: MomoCreateResponse = post_json(&cfg.endpoint, &body).await?;
    if response.result_code != 0 {
        if momo_result_is_uncertain(response.result_code) {
            return Err(AppError::PaymentOrderUncertain);
        }
        return Err(AppError::PaymentRejected);
    }
    if response.partner_code != cfg.partner_code
        || response.request_id != request_id
        || response.order_id != order_id
        || response.amount != amount
    {
        return Err(AppError::PaymentGateway);
    }
    validate_payment_url(
        response
            .pay_url
            .as_deref()
            .ok_or(AppError::PaymentGateway)?,
        "momo.vn",
    )
}

#[derive(Serialize)]
struct ZaloCreateRequest<'a> {
    app_id: i64,
    app_user: &'a str,
    app_time: i64,
    app_trans_id: &'a str,
    amount: i64,
    expire_duration_seconds: i64,
    item: &'static str,
    description: &'static str,
    embed_data: &'a str,
    bank_code: &'static str,
    callback_url: &'a str,
    mac: String,
}

#[derive(Deserialize)]
struct ZaloCreateResponse {
    return_code: i32,
    #[serde(default)]
    sub_return_code: Option<i32>,
    order_url: Option<String>,
}

fn zalopay_result_is_uncertain(response: &ZaloCreateResponse) -> bool {
    response.return_code != 1
        && !(response.return_code == 2
            && matches!(
                response.sub_return_code,
                Some(-2 | -53 | -401 | -402 | -403 | -405)
            ))
}

pub(super) async fn create_zalopay_order(
    settings: &Settings,
    reservation: &Reservation,
    user_id: uuid::Uuid,
) -> AppResult<String> {
    let cfg = credentials(settings, PaymentProvider::Zalopay)?;
    let app_id = cfg
        .app_id
        .parse::<i64>()
        .map_err(|_| AppError::PaymentGateway)?;
    let app_user = user_id.to_string();
    let app_time = chrono::Utc::now().timestamp_millis();
    let app_trans_id = reservation.provider_order_id.as_str();
    let amount = i64::from(reservation.amount);
    let item = "[]";
    let embed_data = serde_json::to_string(&serde_json::json!({"redirecturl": cfg.redirect_url}))
        .map_err(|_| AppError::PaymentGateway)?;
    let raw = format!("{app_id}|{app_trans_id}|{app_user}|{amount}|{app_time}|{embed_data}|{item}");
    let body = ZaloCreateRequest {
        app_id,
        app_user: &app_user,
        app_time,
        app_trans_id,
        amount,
        expire_duration_seconds: ORDER_TTL_SECONDS,
        item,
        description: "PickUp game bill",
        embed_data: &embed_data,
        bank_code: "",
        callback_url: &cfg.callback_url,
        mac: hmac_hex(&cfg.key, &raw)?,
    };
    let response: ZaloCreateResponse = post_json(&cfg.endpoint, &body).await?;
    if response.return_code != 1 {
        if zalopay_result_is_uncertain(&response) {
            return Err(AppError::PaymentOrderUncertain);
        }
        return Err(AppError::PaymentRejected);
    }
    validate_payment_url(
        response
            .order_url
            .as_deref()
            .ok_or(AppError::PaymentGateway)?,
        "zalopay.vn",
    )
}

async fn post_json<T: Serialize, R: for<'de> Deserialize<'de>>(
    endpoint: &str,
    body: &T,
) -> AppResult<R> {
    post_json_with_timeout(endpoint, body, 10).await
}

async fn post_json_with_timeout<T: Serialize, R: for<'de> Deserialize<'de>>(
    endpoint: &str,
    body: &T,
    timeout_seconds: u64,
) -> AppResult<R> {
    let endpoint = validate_endpoint(endpoint)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::PaymentGateway)?;
    let response = client
        .post(endpoint)
        .json(body)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Payment provider request failed");
            AppError::PaymentGateway
        })?;
    if !response.status().is_success() {
        return Err(AppError::PaymentGateway);
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::PaymentGateway)?
    {
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err(AppError::PaymentGateway);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| AppError::PaymentGateway)
}

pub(super) struct OrderStatus {
    pub amount: i64,
    pub transaction_id: Option<String>,
    pub outcome: &'static str,
}

pub(super) fn momo_terminal_failure(code: i32) -> bool {
    matches!(code, 1001..=1007 | 1017 | 1026 | 4001 | 4002 | 4100)
}

/// Query URLs are derived from the configured create endpoint on the same
/// trusted origin; defaults resolve to each provider's /query API.
pub(super) async fn query_order(
    settings: &Settings,
    payment: &Reservation,
) -> AppResult<OrderStatus> {
    let cfg = credentials(settings, payment.provider)?;
    let endpoint = validate_endpoint(&cfg.endpoint)?
        .join("query")
        .map_err(|_| AppError::PaymentGateway)?;
    match payment.provider {
        PaymentProvider::Momo => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct QueryResponse {
                partner_code: String,
                order_id: String,
                request_id: String,
                result_code: i32,
                amount: Option<i64>,
                trans_id: Option<i64>,
            }
            let request_id = uuid::Uuid::new_v4().simple().to_string();
            let raw = format!(
                "accessKey={}&orderId={}&partnerCode={}&requestId={}",
                settings.momo.access_key, payment.provider_order_id, cfg.partner_code, request_id
            );
            let body = serde_json::json!({"partnerCode": cfg.partner_code,
                "orderId": payment.provider_order_id, "requestId": request_id,
                "lang": "en", "signature": hmac_hex(&cfg.key, &raw)?});
            let response: QueryResponse =
                post_json_with_timeout(endpoint.as_str(), &body, 35).await?;
            if response.partner_code != cfg.partner_code
                || response.order_id != payment.provider_order_id
                || response.request_id != request_id
            {
                return Err(AppError::PaymentGateway);
            }
            let txn = response
                .trans_id
                .filter(|id| *id > 0)
                .map(|id| id.to_string());
            let outcome = if response.result_code == 0 && txn.is_some() && response.amount.is_some()
            {
                "paid"
            } else if momo_terminal_failure(response.result_code) {
                "failed"
            } else {
                "processing"
            };
            Ok(OrderStatus {
                amount: response.amount.unwrap_or(i64::from(payment.amount)),
                transaction_id: txn,
                outcome,
            })
        }
        PaymentProvider::Zalopay => {
            #[derive(Deserialize)]
            struct QueryResponse {
                return_code: i32,
                sub_return_code: Option<i32>,
                is_processing: Option<bool>,
                amount: Option<i64>,
                zp_trans_id: Option<i64>,
            }
            let app_id = cfg
                .app_id
                .parse::<i64>()
                .map_err(|_| AppError::PaymentGateway)?;
            let raw = format!("{}|{}|{}", app_id, payment.provider_order_id, cfg.key);
            let body = serde_json::json!({"app_id": app_id, "app_trans_id": payment.provider_order_id,
                "mac": hmac_hex(&cfg.key, &raw)?});
            let response: QueryResponse =
                post_json_with_timeout(endpoint.as_str(), &body, 35).await?;
            let txn = response
                .zp_trans_id
                .filter(|id| *id > 0)
                .map(|id| id.to_string());
            let outcome = if response.return_code == 1
                && response.is_processing == Some(false)
                && txn.is_some()
                && response.amount.is_some()
            {
                "paid"
            } else if response.return_code == 2
                && response.sub_return_code == Some(-54)
                && response.is_processing == Some(false)
            {
                "failed"
            } else {
                "processing"
            };
            Ok(OrderStatus {
                amount: response.amount.unwrap_or(i64::from(payment.amount)),
                transaction_id: txn,
                outcome,
            })
        }
    }
}

fn validate_payment_url(value: &str, provider_domain: &str) -> AppResult<String> {
    let url = url::Url::parse(value).map_err(|_| AppError::PaymentGateway)?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == provider_domain || host.ends_with(&format!(".{provider_domain}")))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::PaymentGateway);
    }
    Ok(value.to_owned())
}

pub(super) fn momo_signature_message(
    body: &crate::models::payment::MomoWebhook,
    access_key: &str,
) -> String {
    format!(
        "accessKey={access_key}&amount={}&extraData={}&message={}&orderId={}&orderInfo={}&orderType={}&partnerCode={}&payType={}&requestId={}&responseTime={}&resultCode={}&transId={}",
        body.amount, body.extra_data, body.message, body.order_id, body.order_info, body.order_type,
        body.partner_code, body.pay_type, body.request_id, body.response_time, body.result_code, body.trans_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_provider_create_results_do_not_release_the_order() {
        for result_code in [40, 41, 43, 7000, 7002, 9000] {
            assert!(momo_result_is_uncertain(result_code));
        }
        assert!(!momo_result_is_uncertain(0));
        assert!(!momo_result_is_uncertain(20));
        assert!(momo_result_is_uncertain(7));

        assert!(zalopay_result_is_uncertain(&ZaloCreateResponse {
            return_code: 3,
            sub_return_code: None,
            order_url: None,
        }));
        assert!(zalopay_result_is_uncertain(&ZaloCreateResponse {
            return_code: 2,
            sub_return_code: Some(-68),
            order_url: None,
        }));
        assert!(!zalopay_result_is_uncertain(&ZaloCreateResponse {
            return_code: 2,
            sub_return_code: Some(-53),
            order_url: None,
        }));
    }

    #[test]
    fn hmac_sha256_matches_rfc4231_and_rejects_modified_signatures() {
        let expected = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
        let signature = hmac_hex("Jefe", "what do ya want for nothing?").unwrap();
        assert_eq!(signature, expected);
        assert!(verify_hmac_hex(
            "Jefe",
            "what do ya want for nothing?",
            &signature
        ));
        assert!(!verify_hmac_hex(
            "Jefe",
            "what do ya want for something?",
            &signature
        ));
        assert!(!verify_hmac_hex(
            "Jefe",
            "what do ya want for nothing?",
            "bad"
        ));
    }

    #[test]
    fn payment_urls_must_stay_on_https_provider_hosts() {
        assert!(
            validate_payment_url("https://test-payment.momo.vn/v2/gateway/pay?t=x", "momo.vn")
                .is_ok()
        );
        assert!(validate_payment_url("https://sbgateway.zalopay.vn/pay?t=x", "zalopay.vn").is_ok());
        assert!(validate_payment_url("http://test-payment.momo.vn/pay", "momo.vn").is_err());
        assert!(validate_payment_url("https://momo.vn.attacker.example/pay", "momo.vn").is_err());
    }
}
