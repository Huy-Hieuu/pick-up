//! Shared per-client fixed-window limits for API requests.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{error::AppError, state::AppState};

type ClientHash = Hmac<Sha256>;
const WINDOW_SECONDS: i64 = 60;
const REDIS_KEY_TTL_SECONDS: i64 = 120;

pub async fn limit_requests(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    // Health probes are intentionally independent of public API quotas.
    if path == "/health" || path == "/health/ready" {
        return next.run(request).await;
    }
    let client = match resolve_client_ip(&request, &state.settings.app.trusted_proxy_cidrs) {
        Ok(client) => client,
        Err(error) => return error.into_response(),
    };
    let digest = match ClientHash::new_from_slice(state.settings.jwt.secret.as_bytes()) {
        Ok(mut mac) => {
            mac.update(client.to_string().as_bytes());
            hex::encode(mac.finalize().into_bytes())
        }
        Err(_) => {
            return AppError::Internal(anyhow::anyhow!(
                "Client rate-limit key initialization failed"
            ))
            .into_response()
        }
    };
    let window = chrono::Utc::now().timestamp() / WINDOW_SECONDS;
    let key = format!("http-rate:{window}:{digest}");
    let count: i64 = redis::Script::new(
        r#"
        local n = redis.call('INCR', KEYS[1])
        if n == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
        return n
    "#,
    )
    .key(key)
    .arg(REDIS_KEY_TTL_SECONDS)
    .invoke_async(&mut state.redis.clone())
    .await
    .unwrap_or_else(|error| {
        tracing::error!(%error, "Rate-limit store unavailable; rejecting request");
        i64::MAX
    });
    if count > i64::from(state.settings.app.requests_per_minute) {
        return AppError::TooManyRequests(
            "Rate limit exceeded; retry after the current minute".into(),
        )
        .into_response();
    }
    next.run(request).await
}

fn resolve_client_ip(
    request: &Request,
    trusted_proxies: &[ipnet::IpNet],
) -> Result<IpAddr, AppError> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    if !trusted_proxies.iter().any(|range| range.contains(&peer)) {
        // X-Forwarded-For supplied directly by an untrusted peer is spoofable.
        return Ok(peer);
    }
    let Some(forwarded) = request.headers().get("x-forwarded-for") else {
        return Ok(peer);
    };
    let forwarded = forwarded
        .to_str()
        .map_err(|_| AppError::BadRequest("Invalid forwarded client address".into()))?;
    let chain: Vec<IpAddr> = forwarded
        .split(',')
        .map(str::trim)
        .map(|value| {
            value
                .parse::<IpAddr>()
                .map_err(|_| AppError::BadRequest("Invalid forwarded client address".into()))
        })
        .collect::<Result<_, _>>()?;
    if chain.is_empty() {
        return Err(AppError::BadRequest(
            "Invalid forwarded client address".into(),
        ));
    }
    // Walk from the socket-facing side. Only discard hops inside configured
    // proxy ranges; everything left of the first untrusted address is ignored.
    Ok(chain
        .into_iter()
        .rev()
        .find(|address| !trusted_proxies.iter().any(|range| range.contains(address)))
        .unwrap_or(peer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(peer: &str, forwarded: Option<&str>) -> Request {
        let mut builder = axum::http::Request::builder().uri("/api");
        if let Some(forwarded) = forwarded {
            builder = builder.header("x-forwarded-for", forwarded);
        }
        let mut request = builder.body(axum::body::Body::empty()).unwrap();
        let peer: SocketAddr = format!("{peer}:8080").parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(peer));
        request
    }

    #[test]
    fn untrusted_forwarding_headers_are_ignored() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        assert_eq!(
            resolve_client_ip(&request("198.51.100.10", Some("192.0.2.1")), &trusted).unwrap(),
            "198.51.100.10".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn trusted_proxy_chain_uses_nearest_untrusted_hop() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let request = request("10.0.0.2", Some("203.0.113.99, 198.51.100.10, 10.0.0.1"));
        assert_eq!(
            resolve_client_ip(&request, &trusted).unwrap(),
            "198.51.100.10".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn malformed_forwarding_header_from_a_trusted_proxy_is_rejected() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        assert!(matches!(
            resolve_client_ip(&request("10.0.0.2", Some("spoof")), &trusted),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn untrusted_rightmost_proxy_and_spoofed_left_entries_are_ignored() {
        let trusted: Vec<ipnet::IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let request = request("10.0.0.2", Some("192.0.2.66, 10.0.0.1, 198.51.100.18"));
        assert_eq!(
            resolve_client_ip(&request, &trusted).unwrap(),
            "198.51.100.18".parse::<IpAddr>().unwrap()
        );
    }
}
