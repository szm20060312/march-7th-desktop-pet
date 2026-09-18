Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$CodexRoot = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME ".codex" }
$PetsRoot = Join-Path $CodexRoot "pets"
$Destination = Join-Path $PetsRoot "march-7th"
$Stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$BackupRoot = Join-Path $PetsRoot "_backups"
$Backup = Join-Path $BackupRoot "march-7th-$Stamp"

$Expected = @{
    "pet.json" = "0f61a86adf8051b72600d4b7fad1db70dbf899f5c8e424976df838f2905b6c03"
    "spritesheet.webp" = "1223cabacfef28dbd40a46387ffff28ecf951460d47bec1defeebbadc2da8622"
}

foreach ($Name in $Expected.Keys) {
    $Source = Join-Path $ScriptDir $Name
    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Missing package file: $Name"
    }
    $Actual = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Actual -ne $Expected[$Name]) {
        throw "SHA-256 verification failed: $Name"
    }
    Write-Host "Verified: $Name"
}

New-Item -ItemType Directory -Force -Path $PetsRoot | Out-Null

if (Test-Path -LiteralPath $Destination -PathType Container) {
    New-Item -ItemType Directory -Force -Path $BackupRoot | Out-Null
    Copy-Item -LiteralPath $Destination -Destination $Backup -Recurse
    Write-Host "Existing version backed up to: $Backup"
}

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item -LiteralPath (Join-Path $ScriptDir "pet.json") -Destination (Join-Path $Destination "pet.json") -Force
Copy-Item -LiteralPath (Join-Path $ScriptDir "spritesheet.webp") -Destination (Join-Path $Destination "spritesheet.webp") -Force

Write-Host "March 7th v1.0.0 installed to: $Destination"
Write-Host "Fully quit and restart the Codex/ChatGPT desktop app."

