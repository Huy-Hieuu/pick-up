param(
    [Parameter(Mandatory = $true)][Uri]$ApiOrigin,
    [ValidateRange(1, 12)][int]$Attempts = 6
)
$ErrorActionPreference = 'Stop'
if ($ApiOrigin.UserInfo -or $ApiOrigin.Query -or $ApiOrigin.Fragment) {
    throw 'Use an API origin without credentials, query, or fragment.'
}
if ($ApiOrigin.Scheme -ne 'https' -and -not ($ApiOrigin.Scheme -eq 'http' -and $ApiOrigin.IsLoopback)) {
    throw 'Use HTTPS for staging; HTTP is permitted only for loopback checks.'
}
$endpoint = $ApiOrigin.AbsoluteUri.TrimEnd('/') + '/health/ready'
for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
    try {
        $response = Invoke-WebRequest -Uri $endpoint -TimeoutSec 3
        if ($response.StatusCode -eq 200) {
            Write-Output 'PASS: API readiness returned 200 (PostgreSQL and Redis only).'
            exit 0
        }
    } catch {
        # Do not echo endpoint credentials, response bodies, or exception details.
    }
    if ($attempt -lt $Attempts) { Start-Sleep -Seconds 5 }
}
Write-Error 'FAIL: API readiness did not return 200 within the bounded retries.'
exit 1
