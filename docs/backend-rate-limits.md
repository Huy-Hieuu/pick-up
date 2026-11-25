# API request limits and proxy trust

By default, the API allows **300 requests per client IP per fixed minute** on
all non-health HTTP routes, including WebSocket handshakes. Redis atomically
enforces the window across API instances. The counter key is an HMAC of the IP,
not the raw IP. `GET /health` and `/health/ready` are exempt. Excess requests
receive HTTP 429 and `Retry-After: 60`.

## Reverse proxies

By default, the API keys limits from the TCP peer address and ignores
`X-Forwarded-For`. To recover visitor addresses behind an ingress, set
`TRUSTED_PROXY_CIDRS` to the exact network ranges of proxies that strip or
overwrite any client-supplied forwarding header and append the actual peer IP.
The API reads the chain from the socket-facing end, skips only configured proxy
ranges, and uses the first untrusted address. A malformed header from a trusted
peer receives 400. Requests from non-trusted peers cannot spoof their rate-limit
key with forwarding headers.

Set `APP_REQUESTS_PER_MINUTE` to an integer from 1 to 100,000. The Compose
development stack defaults to 300. If the app is reachable directly as well as
through a proxy, allow only the proxy ranges in the firewall; otherwise a client
can bypass the proxy's address by making a direct request. Limits are per IP,
so carrier NATs, offices, and shared Wi-Fi users share a quota. Choose a limit
that fits measured launch traffic. Login also has a separate per-account
attempt counter and WebSocket subscriptions have per-user/process limits.

This is an application quota, not volumetric DDoS protection. Configure upstream
load-balancer/WAF protections before making the service public. The forwarded IP
is sensitive operational data; the application keeps only a keyed hash in
Redis. Proxy retention/access policy remains the deployer's responsibility.

The forwarded-chain trust policy follows IETF guidance that forwarding
information can be modified by clients and is trustworthy only across verified
proxies ([RFC 7239, security considerations](https://www.rfc-editor.org/rfc/rfc7239.html#section-8.1)).
The rate-limit response follows HTTP 429 semantics ([RFC 6585](https://www.rfc-editor.org/rfc/rfc6585.html#section-4)).
