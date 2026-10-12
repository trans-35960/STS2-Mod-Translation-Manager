param(
    # Folder with sts2.dll / 0Harmony.dll; found from Steam when omitted.
    [string]$Sts2DataDir = $env:STS2_DATA_DIR,
    # Exit 0 with a notice instead of failing when .NET or the game is missing.
    [switch]$SkipIfUnavailable
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$project = Join-Path $repoRoot "bridge\STS2ModManagerBridge.csproj"
$outputDir = Join-Path $repoRoot "vendor\bridge"
$modId = "STS2ModManagerBridge"

function Skip-OrFail([string]$Message) {
    if ($SkipIfUnavailable) {
        Write-Host "브리지 빌드 건너뜀: $Message"
        exit 0
    }
    throw $Message
}

function Find-Sts2DataDir {
    $steamRoots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Steam"),
        (Join-Path $env:ProgramFiles "Steam")
    ) | Where-Object { $_ -and (Test-Path (Join-Path $_ "steamapps\libraryfolders.vdf")) }
    $libraries = @()
    foreach ($steam in $steamRoots) {
        $libraries += $steam
        $vdf = Get-Content (Join-Path $steam "steamapps\libraryfolders.vdf") -Raw
        foreach ($match in [regex]::Matches($vdf, '"path"\s+"([^"]+)"')) {
            $libraries += $match.Groups[1].Value -replace '\\\\', '\'
        }
    }
    foreach ($library in ($libraries | Select-Object -Unique)) {
        $candidate = Join-Path $library "steamapps\common\Slay the Spire 2\data_sts2_windows_x86_64"
        if (Test-Path (Join-Path $candidate "sts2.dll")) {
            return $candidate
        }
    }
    return $null
}

if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
    Skip-OrFail ".NET SDK(dotnet)를 찾지 못했습니다."
}
if (-not $Sts2DataDir) {
    $Sts2DataDir = Find-Sts2DataDir
}
if (-not $Sts2DataDir -or -not (Test-Path (Join-Path $Sts2DataDir "sts2.dll"))) {
    Skip-OrFail "Slay the Spire 2 데이터 폴더(sts2.dll)를 찾지 못했습니다. STS2_DATA_DIR 환경 변수로 지정하세요."
}

# The bridge ships with the app, so it carries the app version.
$tauriConfig = Get-Content (Join-Path $repoRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$version = $tauriConfig.version

Write-Host "브리지 빌드: $version ($Sts2DataDir)"
dotnet build $project -c Release -v q -nologo "-p:Sts2DataDir=$Sts2DataDir" "-p:Version=$version"
if ($LASTEXITCODE -ne 0) {
    throw "브리지 빌드 실패 (exit code $LASTEXITCODE)"
}

# Godot.NET.Sdk writes the assembly under .godot/mono/temp.
$builtDll = Join-Path $repoRoot "bridge\.godot\mono\temp\bin\Release\$modId.dll"
if (-not (Test-Path $builtDll)) {
    throw "빌드 결과를 찾지 못했습니다: $builtDll"
}

New-Item -ItemType Directory -Force $outputDir | Out-Null
Copy-Item $builtDll (Join-Path $outputDir "$modId.dll") -Force
$manifest = Get-Content (Join-Path $repoRoot "bridge\$modId.json") -Raw | ConvertFrom-Json
$manifest.version = $version
$manifest | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $outputDir "$modId.json") -Encoding utf8NoBOM
Write-Host "브리지 출력: $outputDir"
