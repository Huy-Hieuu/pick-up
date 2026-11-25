$ErrorActionPreference = 'Stop'
$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$api = 'http://127.0.0.1:8080'
$redisCli = "$repo/artifacts/verification-services/redis/Redis-7.4.11-Windows-x64-msys2/redis-cli.exe"
$psql = "$repo/artifacts/verification-services/pgsql/bin/psql.exe"
function Register-Preview($email, $phone, $name) {
  $null = Invoke-RestMethod "$api/auth/register" -Method Post -ContentType 'application/json' -Body (@{email=$email} | ConvertTo-Json)
  $code = (& $redisCli -h 127.0.0.1 -p 56379 -n 1 GET "otp:registration:$email").Trim()
  Invoke-RestMethod "$api/auth/verify-registration" -Method Post -ContentType 'application/json' -Body (@{email=$email;code=$code;password='Pickup-test-2026';phone=$phone;display_name=$name} | ConvertTo-Json)
}
$preview = Register-Preview 'preview@example.com' '0901234567' 'Minh Nguyen'
$owner = Register-Preview 'owner-preview@example.com' '0902345678' 'Alex Tran'
& $psql -h 127.0.0.1 -p 55432 -U pickup -d pickup_preview -v "owner_id=$($owner.user.id)" -f "$PSScriptRoot/seed-courts.sql"
if ($LASTEXITCODE -ne 0) { throw 'Fixture SQL failed' }
$headers = @{Authorization="Bearer $($owner.access_token)"}
$day = [DateTimeOffset]::UtcNow.ToOffset([TimeSpan]::FromHours(7)).Date.AddDays(1).ToString('yyyy-MM-dd')
$court = '10000000-0000-4000-8000-000000000001'
$slots = Invoke-RestMethod "$api/courts/$court/slots?date=$day"
$slot = $slots.slots[0].id
$null = Invoke-RestMethod "$api/courts/$court/slots/$slot/book" -Method Post -Headers $headers -ContentType 'application/json' -Body '{}'
$game = Invoke-RestMethod "$api/games" -Method Post -Headers $headers -ContentType 'application/json' -Body (@{court_slot_id=$slot;sport_type='pickleball';max_players=6;description='Friendly evening doubles. All levels welcome. Bring a paddle and meet the community.'} | ConvertTo-Json)
$courts = Invoke-RestMethod "$api/courts"
$games = Invoke-RestMethod "$api/games" -Headers @{Authorization="Bearer $($preview.access_token)"}
$me = Invoke-RestMethod "$api/auth/me" -Headers @{Authorization="Bearer $($preview.access_token)"}
@{preview_user_id=$preview.user.id;owner_user_id=$owner.user.id;game_id=$game.id;slot_id=$slot;court_ids=@($courts.items | ForEach-Object id);game_count=$games.total;court_count=$courts.total;profile_name=$me.display_name} | ConvertTo-Json | Tee-Object -FilePath "$PSScriptRoot/fixture-evidence.json"
