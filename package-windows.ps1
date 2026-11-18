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

$distributionRoot = Join-Path $projectRoot 'dist'
$packageName = 'EFT-Rust-First-Build'
$packageRoot = Join-Path $distributionRoot $packageName
$archivePath = Join-Path $distributionRoot "$packageName.zip"
if (Test-Path -LiteralPath $packageRoot) {
    throw "Package directory already exists; move it before packaging again: $packageRoot"
}
if (Test-Path -LiteralPath $archivePath) {
    throw "Package archive already exists; move it before packaging again: $archivePath"
}

$previousPath = $env:PATH
$locationPushed = $false
try {
    $gnuTools = 'C:\cygwin64\bin'
    if (Test-Path -LiteralPath $gnuTools) {
        $env:PATH = "$gnuTools;$env:PATH"
    }

    Push-Location $projectRoot
    $locationPushed = $true
    & $cargoPath '+stable-x86_64-pc-windows-gnu' build --release -p eft-backend -p eft-client
    if ($LASTEXITCODE -ne 0) {
        throw "Release build failed with exit code $LASTEXITCODE."
    }

    $backendExecutable = Join-Path $projectRoot 'target\release\eft-backend.exe'
    $clientExecutable = Join-Path $projectRoot 'target\release\eft-client.exe'
    foreach ($executable in @($backendExecutable, $clientExecutable)) {
        if (-not (Test-Path -LiteralPath $executable)) {
            throw "Release executable was not created: $executable"
        }
    }

    New-Item -ItemType Directory -Path $distributionRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $packageRoot | Out-Null
    Copy-Item -LiteralPath $backendExecutable -Destination $packageRoot
    Copy-Item -LiteralPath $clientExecutable -Destination $packageRoot
    Copy-Item -LiteralPath (Join-Path $projectRoot 'assets') `
        -Destination $packageRoot -Recurse
    Copy-Item -LiteralPath (Join-Path $projectRoot 'launch-game.cmd') -Destination $packageRoot
    Copy-Item -LiteralPath (Join-Path $projectRoot 'launch-game.ps1') -Destination $packageRoot

    $packageReadme = @(
        'EFT Rust - First Windows Build'
        ''
        'START'
        '1. Extract the complete folder to a location you can write to.'
        '2. Double-click launch-game.cmd.'
        '3. Use SETTINGS from the stash or pause menu to change resolution, volume,'
        '   and mouse sensitivity.'
        ''
        'The launcher starts the local profile service and closes it when the game exits.'
        'No Rust installation, source code, or separate server launch is required.'
        ''
        'SAVE DATA'
        'The inventory profile and settings are stored in:'
        '%LOCALAPPDATA%\Escape From Tarkov Rust\'
        'The game package can be replaced without removing those files.'
        ''
        'CONTROLS'
        'WASD: move    Mouse: look/fire    Shift: sprint    Ctrl: crouch'
        'Esc: pause    I: inventory    R: reload / rotate item'
        'E: pick up loot    F: extract    U: use medkit in inventory'
        ''
        'REQUIREMENTS'
        'Windows 10 or later (64-bit), a GPU driver with a supported DirectX 12'
        'or Vulkan graphics backend, and audio output.'
        ''
        'AUDIO CREDITS AND LICENSES'
        'Gunshots: OpenGameArt.org, Gunshot Sounds, recorded by Vincent Sevedge.'
        'The included excerpts are covered by CC BY 3.0; source and credits:'
        'https://opengameart.org/content/gunshot-sounds'
        'Footsteps: Fantozzi''s Footsteps, CC0:'
        'https://opengameart.org/content/fantozzis-footsteps-grasssand-stone'
        'Wind: Wind Woosh Loop, CC0: https://opengameart.org/content/wind-whoosh-loop'
        'Impacts: Impact by Iwan "qubodup" Gabovitch, CC0:'
        'https://opengameart.org/content/impact'
        ''
        'The game contains original placeholder geometry and demonstration data.'
    )
    Set-Content -LiteralPath (Join-Path $packageRoot 'README.txt') `
        -Value $packageReadme -Encoding ASCII

    Add-Type -AssemblyName System.IO.Compression
    $archiveStream = [System.IO.File]::Open(
        $archivePath,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None
    )
    $archive = [System.IO.Compression.ZipArchive]::new(
        $archiveStream,
        [System.IO.Compression.ZipArchiveMode]::Create,
        $false
    )
    try {
        Get-ChildItem -LiteralPath $packageRoot -Recurse -File | ForEach-Object {
            $relativePath = $_.FullName.Substring($packageRoot.Length).TrimStart('\')
            $entry = $archive.CreateEntry(
                ($relativePath -replace '\\', '/'),
                [System.IO.Compression.CompressionLevel]::Optimal
            )
            $source = [System.IO.File]::OpenRead($_.FullName)
            $destination = $entry.Open()
            try {
                $source.CopyTo($destination)
            } finally {
                $destination.Dispose()
                $source.Dispose()
            }
        }
    } finally {
        $archive.Dispose()
        $archiveStream.Dispose()
    }

    Write-Output "Package directory: $packageRoot"
    Write-Output "Package archive: $archivePath"
} finally {
    if ($locationPushed) {
        Pop-Location
    }
    $env:PATH = $previousPath
}
