$ErrorActionPreference = 'Stop'
$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$api = 'http://127.0.0.1:8080'
$email = "smoke-$([guid]::NewGuid().ToString('N'))@example.com"
$null = Invoke-RestMethod "$api/auth/register" -Method Post -ContentType 'application/json' -Body (@{email=$email} | ConvertTo-Json)
$code = (& "$repo/artifacts/verification-services/redis/Redis-7.4.11-Windows-x64-msys2/redis-cli.exe" -h 127.0.0.1 -p 56379 -n 1 GET "otp:registration:$email").Trim()
$auth = Invoke-RestMethod "$api/auth/verify-registration" -Method Post -ContentType 'application/json' -Body (@{email=$email;code=$code;password='Pickup-test-2026';phone='0903456789';display_name='Disposable API Smoke'} | ConvertTo-Json)
$headers = @{Authorization="Bearer $($auth.access_token)"}
$fixture = Get-Content "$PSScriptRoot/fixture-evidence.json" -Raw | ConvertFrom-Json
$game = $fixture.game_id
$joined = Invoke-RestMethod "$api/games/$game/join" -Method Post -Headers $headers -ContentType 'application/json' -Body '{}'
$personal = Invoke-RestMethod "$api/auth/me/games" -Headers $headers
$split = Invoke-RestMethod "$api/games/$game/split" -Headers $headers
$payment = Invoke-WebRequest "$api/games/$game/payments/initiate" -Method Post -Headers $headers -ContentType 'application/json' -Body '{"provider":"momo"}' -SkipHttpErrorCheck
if ($payment.StatusCode -ne 501) { throw "Unconfigured preview PSP unexpected status $($payment.StatusCode)" }
$null = Invoke-RestMethod "$api/games/$game/leave" -Method Post -Headers $headers -ContentType 'application/json' -Body '{}'
$court = '10000000-0000-4000-8000-000000000003'
$day = [DateTimeOffset]::UtcNow.ToOffset([TimeSpan]::FromHours(7)).Date.ToString('yyyy-MM-dd')
$slots = Invoke-RestMethod "$api/courts/$court/slots?date=$day"
$slot = $slots.slots[0].id
$booking = Invoke-RestMethod "$api/courts/$court/slots/$slot/book" -Method Post -Headers $headers -ContentType 'application/json' -Body '{}'
$bookings = Invoke-RestMethod "$api/auth/me/bookings" -Headers $headers
$cancel = Invoke-RestMethod "$api/courts/$court/slots/$slot/cancel" -Method Post -Headers $headers -ContentType 'application/json' -Body '{}'
$logout = Invoke-WebRequest "$api/auth/logout" -Method Post -Headers $headers -ContentType 'application/json' -Body '{}'
$revoked = Invoke-WebRequest "$api/auth/me" -Headers $headers -SkipHttpErrorCheck
$refresh = Invoke-WebRequest "$api/auth/refresh" -Method Post -ContentType 'application/json' -Body (@{refresh_token=$auth.refresh_token} | ConvertTo-Json) -SkipHttpErrorCheck
@{registered_verified=$true;joined_player_count=$joined.players.Count;personal_game_count=$personal.total;split_players=$split.player_count;split_total=$split.total_amount;payment_unconfigured_status=$payment.StatusCode;booked_status=$booking.status;personal_bookings_count=$bookings.total;cancelled_status=$cancel.status;logout_status=$logout.StatusCode;revoked_access_status=$revoked.StatusCode;revoked_refresh_status=$refresh.StatusCode} | ConvertTo-Json | Tee-Object -FilePath "$PSScriptRoot/api-smoke-evidence.json"
