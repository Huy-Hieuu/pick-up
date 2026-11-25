$ErrorActionPreference = 'Stop'
$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$env:DATABASE_URL = 'postgres://pickup@127.0.0.1:55432/pickup_preview'
$env:REDIS_URL = 'redis://127.0.0.1:56379/1'
$env:APP_HOST = '127.0.0.1'
$env:APP_PORT = '8080'
$env:CORS_ORIGINS = 'http://localhost:8081,http://127.0.0.1:8081'
$env:APP_PUBLIC_URL = 'https://preview.pickup.example'
$env:APP_PUBLIC_API_URL = 'https://preview-api.pickup.example'
$env:APP_REQUESTS_PER_MINUTE = '10000'
$env:TRUSTED_PROXY_CIDRS = ''
$env:DB_RUN_MIGRATIONS = 'true'
$env:DB_MAX_CONNECTIONS = '10'
$env:SMTP_HOST = '127.0.0.1'
$env:SMTP_PORT = '1025'
$env:SMTP_USE_TLS = 'false'
$env:SMTP_MAIL_FROM = 'preview@pickup.example'
$env:SMTP_MAIL_FROM_NAME = 'PickUp Preview'
$env:SMTP_USERNAME = ''
$env:SMTP_PASSWORD = ''
$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$secretBytes = New-Object byte[] 48
$rng.GetBytes($secretBytes)
$env:JWT_SECRET = [Convert]::ToHexString($secretBytes)
$rng.Dispose()
$env:JWT_ACCESS_TTL_MINUTES = '60'
$env:JWT_REFRESH_TTL_DAYS = '1'
$env:SMS_API_KEY = ''
$env:SMS_SECRET_KEY = ''
$env:MOMO_PARTNER_CODE = ''
$env:MOMO_ACCESS_KEY = ''
$env:MOMO_SECRET_KEY = ''
$env:ZALOPAY_APP_ID = ''
$env:ZALOPAY_KEY1 = ''
$env:ZALOPAY_KEY2 = ''
$env:MOMO_ENDPOINT = 'http://127.0.0.1:10999/momo'
$env:ZALOPAY_ENDPOINT = 'http://127.0.0.1:10999/zalopay'
$env:S3_ENDPOINT = 'http://127.0.0.1:10998'
$env:S3_INTERNAL_ENDPOINT = 'http://127.0.0.1:10998'
$env:S3_REGION = 'us-east-1'
$env:S3_ACCESS_KEY = 'preview-only'
$env:S3_SECRET_KEY = 'preview-only-local-storage-unavailable'
$env:S3_BUCKET = 'pickup-preview'
$env:S3_PUBLIC_URL = 'http://127.0.0.1:10998/pickup-preview'
$env:RUST_LOG = 'info'
Set-Location $PSScriptRoot
& "$repo/server/target/debug/pickup-server.exe"
exit $LASTEXITCODE
