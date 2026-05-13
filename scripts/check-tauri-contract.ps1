$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$commandsPath = Join-Path $repoRoot "src-tauri\src\commands\mod.rs"
$apiPath = Join-Path $repoRoot "web\api\tauri.ts"

function Get-RustCommandNames {
    param([string]$Path)

    $content = Get-Content $Path -Raw
    $matches = [regex]::Matches(
        $content,
        '#\[tauri::command\]\s+pub\(crate\)\s+fn\s+([a-zA-Z0-9_]+)',
        [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    $matches | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
}

function Get-TypeKeys {
    param(
        [string]$Path,
        [string]$TypeName
    )

    $content = Get-Content $Path -Raw
    $typeMatch = [regex]::Match($content, "export\s+type\s+$([regex]::Escape($TypeName))\s*=")
    if (-not $typeMatch.Success) {
        throw "Could not find exported type $TypeName in $Path"
    }

    $bodyStart = $content.IndexOf("{", $typeMatch.Index + $typeMatch.Length)
    if ($bodyStart -lt 0) {
        throw "Could not find object body for exported type $TypeName in $Path"
    }

    $depth = 0
    $bodyEnd = -1
    for ($i = $bodyStart; $i -lt $content.Length; $i++) {
        if ($content[$i] -eq "{") {
            $depth++
        } elseif ($content[$i] -eq "}") {
            $depth--
            if ($depth -eq 0) {
                $bodyEnd = $i
                break
            }
        }
    }

    if ($bodyEnd -lt 0) {
        throw "Could not parse object body for exported type $TypeName in $Path"
    }

    $body = $content.Substring($bodyStart + 1, $bodyEnd - $bodyStart - 1)
    [regex]::Matches($body, "(?m)^  ([a-zA-Z0-9_]+):") |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object -Unique
}

function Compare-ContractSet {
    param(
        [string[]]$Expected,
        [string[]]$Actual,
        [string]$ActualName
    )

    $missing = @(Compare-Object -ReferenceObject $Expected -DifferenceObject $Actual |
        Where-Object SideIndicator -eq "<=" |
        ForEach-Object InputObject)
    $extra = @(Compare-Object -ReferenceObject $Expected -DifferenceObject $Actual |
        Where-Object SideIndicator -eq "=>" |
        ForEach-Object InputObject)

    if ($missing.Count -gt 0 -or $extra.Count -gt 0) {
        if ($missing.Count -gt 0) {
            Write-Error "$ActualName is missing command keys: $($missing -join ', ')"
        }
        if ($extra.Count -gt 0) {
            Write-Error "$ActualName has extra command keys: $($extra -join ', ')"
        }
        throw "$ActualName does not match Rust Tauri command names."
    }
}

$rustCommands = @(Get-RustCommandNames -Path $commandsPath)
$commandArgs = @(Get-TypeKeys -Path $apiPath -TypeName "CommandArgs")
$commandResult = @(Get-TypeKeys -Path $apiPath -TypeName "CommandResult")

Compare-ContractSet -Expected $rustCommands -Actual $commandArgs -ActualName "CommandArgs"
Compare-ContractSet -Expected $rustCommands -Actual $commandResult -ActualName "CommandResult"

Write-Host "Tauri contract check passed: $($rustCommands.Count) commands."
