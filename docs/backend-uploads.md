# Avatar upload contract

1. Request `POST /upload/avatar` with `content_type` (JPEG, PNG, WebP) and an
   exact `content_length` from 1 byte to 8 MiB. The response includes `upload_id`,
   `object_key`, `upload_url`, size limits and a 15-minute expiry. There is no
   public URL yet. The destination is the private `quarantine/` prefix.
2. PUT those bytes using the exact signed `Content-Type` and `Content-Length`.
   Do not use chunked transfer encoding. URLs are bearer capabilities until expiry.
3. Call authenticated `POST /upload/avatar/complete` with `{"upload_id":"..."}`.
   The server checks ownership and actual length, decodes the image with maximum
   dimensions 4096 × 4096 and a 64 MiB decoder allocation limit, resizes to at
   most 1024 × 1024, and re-encodes pixels as PNG. Declared MIME must match the
   detected file format. Malformed images are rejected. Original metadata,
   animation and trailing bytes are not copied.
4. Completion returns `public_url`. Assign it using `PATCH /auth/me` with
   `avatar_url`; only URLs approved for this user are accepted. Registration
   cannot accept avatar URLs. Setting `avatar_url` to null removes the avatar.

Completion is idempotent. Two images at most are decoded concurrently per API
process, on blocking workers. Downloads and output writes have a total
20-second deadline; processing claims are recoverable after two minutes if a
worker stops. Failed attempts have a 10-second backoff. The existing per-user
5/min upload URL quota now uses atomic increment/expiry and returns 429.

## Storage rollout

Apply migration `20260923000007_avatar_quarantine.sql`. Existing user avatar
references are copied into `legacy_avatar_references` and cleared from public
profiles; operators can revalidate the original files, and the migration does
not delete storage objects. Do not roll back to serving old unvalidated URLs.

Apply `scripts/media-public-policy.json` to your S3-compatible bucket (adjust
the bucket ARN if needed). Anonymous reads must be allowed **only** under
`avatars/sanitized/`; quarantine and old avatar paths must remain private.
The Compose initialization service applies this policy automatically. Re-run
that service on existing local buckets. Verify anonymous quarantine GETs fail
against the actual storage provider before enabling uploads publicly.

Configure a one-day lifecycle for `quarantine/` objects, including unfinished
uploads; a still-valid PUT may recreate a raw object after completion. Clean up
expired unfinished `avatar_uploads` rows after that lifecycle window. Approved
records must remain while their URLs can be used in profiles. Serve the media
origin with HTTPS, `X-Content-Type-Options: nosniff`, and a restrictive CSP.
The server sets PNG content type and immutable caching on sanitized objects.

`S3_ENDPOINT` is the client-reachable signing endpoint; `S3_INTERNAL_ENDPOINT`
is used for server reads/writes and defaults to the signing endpoint. Docker
Compose uses `http://minio:9000` internally and `http://localhost:9000` for local
browser PUTs. Set `S3_REGION` to the provider's signing region (default
`us-east-1`; use `auto` for providers that require it). For phones on a LAN,
configure a client-reachable signing endpoint and public media URL. Production
client URLs must use HTTPS.

Local integration tests verify ownership, bounded download length, completion
idempotency, pixel re-encoding, invalid content rejection, and profile assignment.
They do not validate the deployed bucket/CDN policy; that is a rollout gate.
