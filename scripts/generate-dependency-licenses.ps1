param(
    [string]$OutputPath
)

# Collects license notices for the Rust crates and npm packages compiled into
# the release binaries and writes vendor/licenses/THIRD_PARTY_DEPENDENCIES.md.
# Dev/build dependencies and proc-macro crates only run at build time, so they
# are left out. Identical license texts are written once.

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if (-not $OutputPath) {
    $OutputPath = Join-Path $repoRoot "vendor\licenses\THIRD_PARTY_DEPENDENCIES.md"
}

$licenseFilePattern = '^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)'

function Get-LicenseFiles {
    param([Parameter(Mandatory = $true)][string]$Directory)

    if (-not (Test-Path -LiteralPath $Directory -PathType Container)) {
        return
    }
    Get-ChildItem -LiteralPath $Directory -File |
        Where-Object { $_.Name -match $licenseFilePattern } |
        Sort-Object Name
}

function Get-CargoPackages {
    param([Parameter(Mandatory = $true)][string]$ManifestPath)

    $json = cargo metadata --format-version 1 --locked --manifest-path $ManifestPath --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) {
        throw "cargo metadata failed for $ManifestPath (exit code $LASTEXITCODE)"
    }
    $metadata = $json | ConvertFrom-Json

    $packagesById = @{}
    foreach ($package in $metadata.packages) {
        $packagesById[$package.id] = $package
    }
    $nodesById = @{}
    foreach ($node in $metadata.resolve.nodes) {
        $nodesById[$node.id] = $node
    }
    $workspaceIds = @($metadata.workspace_members)

    $visited = @{}
    $queue = [System.Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($metadata.resolve.root)
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($visited.ContainsKey($id)) {
            continue
        }
        $visited[$id] = $true

        $package = $packagesById[$id]
        $isProcMacro = @($package.targets | Where-Object { $_.kind -contains "proc-macro" }).Count -gt 0
        if ($isProcMacro) {
            continue
        }

        foreach ($dep in $nodesById[$id].deps) {
            $isNormal = @($dep.dep_kinds | Where-Object { $null -eq $_.kind }).Count -gt 0
            if ($isNormal) {
                $queue.Enqueue($dep.pkg)
            }
        }
    }

    foreach ($id in $visited.Keys) {
        if ($workspaceIds -contains $id) {
            continue
        }
        $package = $packagesById[$id]
        $isProcMacro = @($package.targets | Where-Object { $_.kind -contains "proc-macro" }).Count -gt 0
        if ($isProcMacro) {
            continue
        }
        [pscustomobject]@{
            Name       = [string]$package.name
            Version    = [string]$package.version
            License    = [string]$package.license
            Repository = [string]$package.repository
            Directory  = Split-Path -Parent $package.manifest_path
        }
    }
}

function Get-NpmPackages {
    $lockPath = Join-Path $repoRoot "package-lock.json"
    # The root package's key is "", which only -AsHashtable accepts.
    $lock = Get-Content -LiteralPath $lockPath -Raw | ConvertFrom-Json -AsHashtable
    foreach ($key in $lock["packages"].Keys) {
        if (-not $key) {
            continue
        }
        $info = $lock["packages"][$key]
        if ($info["dev"]) {
            continue
        }
        $directory = Join-Path $repoRoot ($key -replace '/', '\')
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
            throw "npm package is not installed: $key. Run npm ci first."
        }
        $name = $key -replace '^.*node_modules/', ''
        $repository = ""
        $manifest = Get-Content -LiteralPath (Join-Path $directory "package.json") -Raw | ConvertFrom-Json
        if ($manifest.PSObject.Properties["repository"]) {
            $repository = if ($manifest.repository -is [string]) { $manifest.repository } else { [string]$manifest.repository.url }
        }
        [pscustomobject]@{
            Name       = $name
            Version    = [string]$info["version"]
            License    = [string]$info["license"]
            Repository = $repository
            Directory  = $directory
        }
    }
}

function Get-NormalizedText {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = [System.IO.File]::ReadAllText($Path)
    return ($text -replace "`r`n", "`n").Trim()
}

$sections = @(
    @{ Title = "Rust crates"; Packages = @(
            @(Get-CargoPackages -ManifestPath (Join-Path $repoRoot "Cargo.toml")) +
            @(Get-CargoPackages -ManifestPath (Join-Path $repoRoot "src-tauri\Cargo.toml")) |
            Sort-Object Name, Version -Unique)
    },
    @{ Title = "npm packages"; Packages = @(Get-NpmPackages | Sort-Object Name, Version -Unique) }
)

# License text -> list of "name version" that ship it.
$textOrder = [System.Collections.Generic.List[string]]::new()
$textUsers = @{}
$missing = [System.Collections.Generic.List[string]]::new()

$builder = [System.Text.StringBuilder]::new()
[void]$builder.AppendLine("# Third Party Dependencies")
[void]$builder.AppendLine()
[void]$builder.AppendLine("Generated by ``scripts/generate-dependency-licenses.ps1``. Lists the Rust")
[void]$builder.AppendLine("crates and npm packages compiled into this application, followed by their")
[void]$builder.AppendLine("license texts. Bundled command-line tools are listed in ``THIRD_PARTY_NOTICES.md``.")

foreach ($section in $sections) {
    [void]$builder.AppendLine()
    [void]$builder.AppendLine("## $($section.Title)")
    [void]$builder.AppendLine()
    [void]$builder.AppendLine("| Package | Version | License | Repository |")
    [void]$builder.AppendLine("|---|---|---|---|")
    foreach ($package in $section.Packages) {
        [void]$builder.AppendLine("| $($package.Name) | $($package.Version) | $($package.License) | $($package.Repository) |")

        $label = "$($package.Name) $($package.Version)"
        $files = @(Get-LicenseFiles -Directory $package.Directory)
        if ($files.Count -eq 0) {
            $missing.Add($label)
            continue
        }
        foreach ($file in $files) {
            $text = Get-NormalizedText -Path $file.FullName
            if (-not $text) {
                continue
            }
            if (-not $textUsers.ContainsKey($text)) {
                $textUsers[$text] = [System.Collections.Generic.List[string]]::new()
                $textOrder.Add($text)
            }
            if (-not $textUsers[$text].Contains($label)) {
                $textUsers[$text].Add($label)
            }
        }
    }
}

[void]$builder.AppendLine()
[void]$builder.AppendLine("## License texts")

$fence = '````'
foreach ($text in $textOrder) {
    [void]$builder.AppendLine()
    [void]$builder.AppendLine("Used by: $($textUsers[$text] -join ', ')")
    [void]$builder.AppendLine()
    [void]$builder.AppendLine($fence)
    [void]$builder.AppendLine($text)
    [void]$builder.AppendLine($fence)
}

if ($missing.Count -gt 0) {
    [void]$builder.AppendLine()
    [void]$builder.AppendLine("## Packages without a bundled license file")
    [void]$builder.AppendLine()
    [void]$builder.AppendLine("These packages ship no license file; the license named in the table above applies.")
    [void]$builder.AppendLine()
    foreach ($label in $missing) {
        [void]$builder.AppendLine("- $label")
    }
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) | Out-Null
[System.IO.File]::WriteAllText($OutputPath, $builder.ToString(), [System.Text.UTF8Encoding]::new($false))

$packageCount = ($sections | ForEach-Object { $_.Packages.Count } | Measure-Object -Sum).Sum
Write-Host "Dependency licenses written: $OutputPath ($packageCount packages, $($textOrder.Count) license texts, $($missing.Count) without license file)"
