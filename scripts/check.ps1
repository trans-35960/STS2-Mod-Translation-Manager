param(
    [switch]$NoIncremental,
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repoRoot

$previousIncremental = $env:CARGO_INCREMENTAL
if ($NoIncremental) {
    $env:CARGO_INCREMENTAL = "0"
}

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Label,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Command
    )

    Write-Host ""
    Write-Host "==> $Label"
    $global:LASTEXITCODE = 0
    & $Command
    # Native tools (cargo, node) report failure only through the exit code.
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed (exit code $LASTEXITCODE)"
    }
}

function Resolve-Node {
    $nodeCommand = Get-Command node -ErrorAction SilentlyContinue
    if ($nodeCommand) {
        return $nodeCommand.Source
    }

    $bundledNode = "C:\Program Files\cursor\resources\app\resources\helpers\node.exe"
    if (Test-Path $bundledNode) {
        return $bundledNode
    }

    throw "Node.js executable was not found on PATH or at the Cursor bundled runtime path."
}

try {
    Invoke-Step "cargo test" {
        cargo test
    }
    Invoke-Step "cargo clippy" {
        cargo clippy --all-targets -- -D warnings
    }
    Invoke-Step "src-tauri cargo test" {
        cargo test --manifest-path src-tauri\Cargo.toml
    }
    Invoke-Step "src-tauri cargo clippy" {
        cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
    }
    Invoke-Step "Tauri command contract" {
        & (Join-Path $PSScriptRoot "check-tauri-contract.ps1")
    }

    if (-not $SkipFrontend) {
        $node = Resolve-Node
        Invoke-Step "tsc" {
            & $node .\node_modules\typescript\bin\tsc
        }
        Invoke-Step "vite build" {
            & $node .\node_modules\vite\bin\vite.js build
        }
    }
} finally {
    if ($null -eq $previousIncremental) {
        Remove-Item Env:CARGO_INCREMENTAL -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_INCREMENTAL = $previousIncremental
    }
}
