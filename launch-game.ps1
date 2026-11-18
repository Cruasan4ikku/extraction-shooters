$ErrorActionPreference = 'Stop'

$packageRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$backendExecutable = Join-Path $packageRoot 'eft-backend.exe'
$clientExecutable = Join-Path $packageRoot 'eft-client.exe'
$releaseRoot = Join-Path $packageRoot 'target\release'
if (-not ((Test-Path -LiteralPath $backendExecutable) -and
        (Test-Path -LiteralPath $clientExecutable))) {
    $releaseBackend = Join-Path $releaseRoot 'eft-backend.exe'
    $releaseClient = Join-Path $releaseRoot 'eft-client.exe'
    if ((Test-Path -LiteralPath $releaseBackend) -and
        (Test-Path -LiteralPath $releaseClient)) {
        $backendExecutable = $releaseBackend
        $clientExecutable = $releaseClient
    } elseif ((Test-Path -LiteralPath (Join-Path $packageRoot 'Cargo.toml')) -and
        (Test-Path -LiteralPath (Join-Path $packageRoot 'run-game.ps1'))) {
        & (Join-Path $packageRoot 'run-game.ps1')
        exit $LASTEXITCODE
    }
}

$previousBindAddress = $env:BIND_ADDR
$previousBackendUrl = $env:EFT_BACKEND_URL
$backendProcess = $null
$locationPushed = $false
$clientExitCode = 0

try {
    if (-not (Test-Path -LiteralPath $backendExecutable)) {
        throw "Backend executable was not found: $backendExecutable"
    }
    if (-not (Test-Path -LiteralPath $clientExecutable)) {
        throw "Client executable was not found: $clientExecutable"
    }

    $env:BIND_ADDR = '127.0.0.1:3000'
    $env:EFT_BACKEND_URL = 'http://127.0.0.1:3000'
    $backendProcess = Start-Process `
        -FilePath $backendExecutable `
        -WorkingDirectory $packageRoot `
        -PassThru `
        -NoNewWindow

    $backendReady = $false
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        $backendProcess.Refresh()
        if ($backendProcess.HasExited) {
            throw "Backend exited before becoming ready (exit code $($backendProcess.ExitCode))."
        }

        try {
            $health = Invoke-RestMethod -Uri 'http://127.0.0.1:3000/health' -TimeoutSec 2
            if ($health.status -eq 'ok') {
                $backendReady = $true
                break
            }
        } catch [System.Net.WebException] {
            # The backend may not have bound its port yet.
        }

        Start-Sleep -Seconds 1
    }

    if (-not $backendReady) {
        throw 'Backend did not become ready at http://127.0.0.1:3000 within 30 seconds.'
    }

    Push-Location $packageRoot
    $locationPushed = $true
    & $clientExecutable
    $clientExitCode = $LASTEXITCODE
} finally {
    if ($null -ne $backendProcess) {
        $backendProcess.Refresh()
        if (-not $backendProcess.HasExited) {
            Stop-Process -Id $backendProcess.Id
        }
    }

    if ($locationPushed) {
        Pop-Location
    }

    $env:BIND_ADDR = $previousBindAddress
    $env:EFT_BACKEND_URL = $previousBackendUrl
}

exit $clientExitCode
