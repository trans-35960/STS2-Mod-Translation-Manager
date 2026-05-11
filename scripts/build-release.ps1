$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot\..
try {
    cargo fmt --check
    cargo test
    cargo build --release
    .\target\release\sts2_mod_manager.exe scan
    .\target\release\sts2_mod_manager.exe launch status
    .\target\release\sts2_mod_manager.exe tools status
    "q" | .\target\release\sts2_mod_manager.exe ui
    .\target\release\sts2_mod_manager.exe help
}
finally {
    Pop-Location
}
