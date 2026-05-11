$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot\..
try {
    $exe = ".\src-tauri\target\release\sts2_mod_manager_tauri.exe"
    if (-not (Test-Path $exe)) {
        npm run tauri build
    }

    $env:STS2_E2E_NO_FOCUS = "1"
    $process = Start-Process `
        -FilePath $exe `
        -WorkingDirectory . `
        -WindowStyle Hidden `
        -PassThru

    Start-Sleep -Seconds 3
    if ($process.HasExited) {
        throw "Tauri app exited during E2E smoke test with code $($process.ExitCode)."
    }

    Stop-Process -Id $process.Id -Force
    "e2e smoke: launched hidden and stopped"
}
finally {
    Remove-Item Env:\STS2_E2E_NO_FOCUS -ErrorAction SilentlyContinue
    Pop-Location
}
