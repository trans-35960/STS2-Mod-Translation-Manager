param(
    [switch]$SkipBuild,
    [switch]$SkipSmoke
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.Path]::GetFullPath($Path)
}

function Assert-PathUnderRoot {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Root
    )

    $fullPath = Get-FullPath $Path
    $trimChars = [char[]]@([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    $fullRoot = (Get-FullPath $Root).TrimEnd($trimChars)
    $rootPrefix = "$fullRoot$([System.IO.Path]::DirectorySeparatorChar)"
    $isRoot = [string]::Equals($fullPath, $fullRoot, [System.StringComparison]::OrdinalIgnoreCase)
    $isUnderRoot = $fullPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)
    if (-not ($isRoot -or $isUnderRoot)) {
        throw "Refusing to operate outside repository root: $fullPath"
    }
}

function Reset-Directory {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Root
    )

    Assert-PathUnderRoot -Path $Path -Root $Root
    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
}

function Copy-RequiredItem {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Required release item is missing: $Source"
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force
}

function Assert-ExecutableNotRunning {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
        return
    }

    $fullPath = Get-FullPath $Path
    $running = Get-Process | Where-Object {
        try {
            $_.Path -and [string]::Equals((Get-FullPath $_.Path), $fullPath, [System.StringComparison]::OrdinalIgnoreCase)
        }
        catch {
            $false
        }
    } | Select-Object -First 1

    if ($null -ne $running) {
        throw "The Tauri release executable is running and cannot be overwritten. Close $($running.ProcessName) (PID $($running.Id)) and run the release script again."
    }
}

function Invoke-Npm {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

    $npmCommand = Get-Command npm.cmd -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $npmCommand) {
        $npmCommand = Get-Command npm -ErrorAction SilentlyContinue | Select-Object -First 1
    }
    if ($null -ne $npmCommand) {
        $npmPath = $npmCommand.Source
    }
    else {
        $npmPath = @(
            "C:\nvm4w\nodejs\npm.cmd",
            (Join-Path $env:ProgramFiles "nodejs\npm.cmd"),
            (Join-Path ${env:ProgramFiles(x86)} "nodejs\npm.cmd"),
            (Join-Path $env:APPDATA "npm\npm.cmd")
        ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf -ErrorAction SilentlyContinue) } | Select-Object -First 1
    }
    if (-not $npmPath) {
        throw "npm was not found on PATH. Install Node.js or add npm.cmd to PATH before building the Tauri desktop app."
    }

    & $npmPath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "npm $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function New-PortablePackage {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $tauriConfigPath = Join-Path $RepoRoot "src-tauri\tauri.conf.json"
    $tauriConfig = Get-Content -LiteralPath $tauriConfigPath -Raw | ConvertFrom-Json
    $version = [string]$tauriConfig.version

    $portableRoot = Join-Path $RepoRoot "target\release\portable"
    $packageName = "sts2-mod-manager-$version-windows-x64-portable"
    $stagingDir = Join-Path $portableRoot $packageName
    $zipPath = Join-Path $portableRoot "$packageName.zip"

    New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null
    Reset-Directory -Path $stagingDir -Root $RepoRoot

    $guiExe = Join-Path $RepoRoot "src-tauri\target\release\sts2_mod_manager_tauri.exe"
    $cliExe = Join-Path $RepoRoot "target\release\sts2_mod_manager.exe"
    $vendorDir = Join-Path $RepoRoot "vendor"
    $thirdPartyNotices = Join-Path $RepoRoot "THIRD_PARTY_NOTICES.md"

    Copy-RequiredItem -Source $guiExe -Destination (Join-Path $stagingDir "STS2 Mod Manager.exe")
    Copy-RequiredItem -Source $cliExe -Destination (Join-Path $stagingDir "sts2_mod_manager.exe")
    Copy-RequiredItem -Source $vendorDir -Destination (Join-Path $stagingDir "vendor")
    Copy-RequiredItem -Source $thirdPartyNotices -Destination (Join-Path $stagingDir "THIRD_PARTY_NOTICES.md")

    New-Item -ItemType File -Force -Path (Join-Path $stagingDir ".sts2-mod-manager-portable") | Out-Null

    @"
Slay the Spire 2 Mod Manager Portable

Run "STS2 Mod Manager.exe" to start the desktop app.

Keep the vendor folder next to the executable. The app uses this portable folder for state, logs, backups, presets, vault, and translation_work.

The bundled sts2_mod_manager.exe is the CLI version and is optional for normal desktop use.
"@ | Set-Content -LiteralPath (Join-Path $stagingDir "README-portable.txt") -Encoding UTF8

    Assert-PathUnderRoot -Path $zipPath -Root $RepoRoot
    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    Compress-Archive -Path (Join-Path $stagingDir "*") -DestinationPath $zipPath -Force
    Write-Host "Portable package created: $zipPath"
}

$repoRoot = Get-FullPath (Join-Path $PSScriptRoot "..")

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        cargo fmt --check
        cargo test
        cargo build --release
        Assert-ExecutableNotRunning -Path (Join-Path $repoRoot "src-tauri\target\release\sts2_mod_manager_tauri.exe")
        Invoke-Npm run tauri build
    }

    if (-not $SkipSmoke) {
        .\target\release\sts2_mod_manager.exe scan
        .\target\release\sts2_mod_manager.exe launch status
        .\target\release\sts2_mod_manager.exe tools status
        "q" | .\target\release\sts2_mod_manager.exe ui
        .\target\release\sts2_mod_manager.exe help
    }

    New-PortablePackage -RepoRoot $repoRoot
}
finally {
    Pop-Location
}
