# Avatar upload contract

`POST /upload/avatar` accepts only JPEG, PNG or WebP and requires an exact
`content_length` from 1 byte to 8 MiB. It returns a 15-minute presigned S3 PUT
and repeats `content_length` and `max_upload_bytes` in the response. The
signature covers the request's `Content-Type` and `Content-Length`; the client
must send both exactly as signed or storage rejects the PUT. Do not retry a
large upload with chunked transfer encoding; use an exact-length body.

The upload URL is rate-limited to 5 per user per minute. A signed URL is a
bearer capability and can be used by anyone who obtains it until expiry, for the
single user-owned avatar key and exact length/type. Presigned PUT itself is
single-object constrained, but it does not prove that the bytes match the
declared image MIME type or are safe to render. Avatar URLs should be served from
an isolated media origin with restrictive response headers. Before public
launch, validate/scan uploaded image bytes and decide how to prevent script or
decompression-bomb content from becoming a profile image.

The mobile client should inspect the local image first, include its byte length
and MIME type when asking for the URL, then PUT those exact bytes with matching
`Content-Type` and `Content-Length`. Requests larger than 8 MiB return 422. A
local signed-URL integration test verifies both headers are signed and oversize
requests are rejected.
