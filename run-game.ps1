$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($null -eq $cargoCommand) {
    $cargoPath = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (-not (Test-Path -LiteralPath $cargoPath)) {
        throw 'Cargo was not found. Install Rust or add cargo.exe to PATH.'
    }
} else {
    $cargoPath = $cargoCommand.Source
}

$previousPath = $env:PATH
$previousBindAddress = $env:BIND_ADDR
$previousBackendUrl = $env:EFT_BACKEND_URL
$backendProcess = $null
$locationPushed = $false
$clientExitCode = 0

try {
    $gnuTools = 'C:\cygwin64\bin'
    if (Test-Path -LiteralPath $gnuTools) {
        $env:PATH = "$gnuTools;$env:PATH"
    }

    $env:BIND_ADDR = '127.0.0.1:3000'
    $env:EFT_BACKEND_URL = 'http://127.0.0.1:3000'
    Push-Location $projectRoot
    $locationPushed = $true

    & $cargoPath '+stable-x86_64-pc-windows-gnu' build --release -p eft-backend -p eft-client
    if ($LASTEXITCODE -ne 0) {
        throw "Release build failed with exit code $LASTEXITCODE."
    }

    $backendExecutable = Join-Path $projectRoot 'target\release\eft-backend.exe'
    $clientExecutable = Join-Path $projectRoot 'target\release\eft-client.exe'
    if (-not (Test-Path -LiteralPath $backendExecutable)) {
        throw "Backend executable was not created: $backendExecutable"
    }
    if (-not (Test-Path -LiteralPath $clientExecutable)) {
        throw "Client executable was not created: $clientExecutable"
    }

    $backendProcess = Start-Process `
        -FilePath $backendExecutable `
        -WorkingDirectory $projectRoot `
        -PassThru

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

    $env:PATH = $previousPath
    $env:BIND_ADDR = $previousBindAddress
    $env:EFT_BACKEND_URL = $previousBackendUrl
}

exit $clientExitCode
