# Vendor Tools

Place embedded helper tools here when the manager needs capabilities that are not implemented directly in Rust.

Expected layout:

```text
vendor/
  godot-pck-explorer-dotnet-ui-console-win-linux-mac/
    GodotPCKExplorer.Console.exe
    mbedTLS/
  GDRE_tools-v2.5.0-beta.5-windows/
    gdre_tools.exe
  7zip/
    7z.exe
    7z.dll
    License.txt
    readme.txt
  bridge/
    STS2ModManagerBridge.dll
    STS2ModManagerBridge.json
```

The 7-Zip CLI is embedded so `.7z` and `.rar` mod archives can be inspected and
language files can be extracted without a system-wide install. Godot PCK
Explorer is embedded so nested `.pck` payloads can be opened after archive
extraction. `bridge/` holds the in-game translation bridge built by
`scripts/build-bridge.ps1` (not committed; see `bridge/README.md`).
Third-party licenses are listed in `THIRD_PARTY_NOTICES.md` at the repo root.
