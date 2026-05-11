# Third Party Notices

This application bundles the following third-party command-line tools and
native libraries as application resources.

## 7-Zip

- Files: `vendor/7zip/7z.exe`, `vendor/7zip/7z.dll`
- Version in tree: 25.01, per `vendor/7zip/readme.txt`
- Copyright: Copyright (C) 1999-2025 Igor Pavlov
- License: GNU LGPL for most files, with BSD 2-clause/BSD 3-clause portions
  and the unRAR restriction for some code in `7z.dll`
- Full local license: `vendor/7zip/License.txt`
- Upstream: https://www.7-zip.org/

The bundled 7-Zip files are used for archive inspection and extraction. Binary
redistribution must preserve the related license information from
`vendor/7zip/License.txt`.

## Godot PCK Explorer

- Files: `vendor/godot-pck-explorer-dotnet-ui-console-win-linux-mac/GodotPCKExplorer.*`
- Version in tree: 1.6.0, per bundled `.deps.json` metadata
- Copyright: Copyright (c) 2024 DmitriySalnikov
- License: MIT
- Full local license:
  `vendor/godot-pck-explorer-dotnet-ui-console-win-linux-mac/LICENSE`
- Upstream: https://github.com/DmitriySalnikov/GodotPCKExplorer

The bundled console executable is used for Godot PCK extraction and repacking.

## mbedTLS_AES / Mbed TLS

- Files:
  `vendor/godot-pck-explorer-dotnet-ui-console-win-linux-mac/mbedTLS/**/*`
- License notice: Mbed TLS files are generally provided under a dual
  Apache-2.0 OR GPL-2.0-or-later license unless specifically indicated
  otherwise by upstream files.
- Local Apache-2.0 license text:
  `vendor/godot-pck-explorer-dotnet-ui-console-win-linux-mac/mbedTLS/LICENSE_APACHE-2.0.txt`
- Upstream: https://github.com/Mbed-TLS/mbedtls

These native AES libraries are included with the Godot PCK Explorer binary
distribution and are kept with it so encrypted Godot PCK operations can run.
