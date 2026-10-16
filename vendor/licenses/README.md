# vendor/licenses

`scripts/generate-dependency-licenses.ps1` writes `THIRD_PARTY_DEPENDENCIES.md`
here: the license notices of the Rust crates and npm packages compiled into the
app. `scripts/build-release.ps1` runs it before bundling, and the app ships this
folder. The generated file is not committed.
