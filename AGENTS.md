# 에이전트 작업 규칙

## 프로젝트 구조

- 루트 Rust 크레이트(`src/`, `Cargo.toml`): 핵심 로직과 CLI
- `src-tauri/`: Tauri 데스크톱 앱 (별도 크레이트, 별도 `Cargo.toml`/`Cargo.lock`)
- `web/`: React 프론트엔드 (`npm run dev`로 개발 서버 실행)

## 검증

- 일반 변경 후에는 `scripts/check.ps1`을 실행합니다. 루트/`src-tauri` 두 크레이트의 `cargo test`와 `cargo clippy -D warnings`, Tauri 명령 계약 검사, `tsc`, `vite build`를 포함합니다.
- 루트에서 `cargo test`만 실행하면 `src-tauri` 테스트는 돌지 않으니 주의합니다.
- 실행하지 못했거나 건너뛴 검사는 결과 보고에 명시합니다.

## 빌드와 릴리즈

- 이 저장소의 GitHub Release 배포 대상은 `trans-35960/STS2-Mod-Translation-Manager`입니다.
- `git push`는 SSH 별칭(`github-trans-35960`)을 사용하므로 `gh` 활성 계정과 무관합니다. 계정이 문제가 되는 것은 `gh release` 등 `gh` 명령뿐입니다.
- `gh`의 활성 계정이 다른 계정이어도 전역 계정을 함부로 바꾸지 않습니다. 릴리즈가 필요하면 먼저 `gh auth status`로 계정을 확인하고, 필요 시 사용자에게 확인한 뒤 배포용 계정 또는 `GH_TOKEN` 방식으로 진행합니다.
- 릴리즈 전에는 `scripts/check.ps1` → `scripts/build-release.ps1` 순서로 실행합니다. `build-release.ps1`은 `cargo fmt --check`, `cargo test`, `cargo build --release`, `npm run tauri build`와 smoke check를 포함합니다. 필요하면 `npm run e2e:smoke`도 실행합니다.
- 버전을 올릴 때는 아래 파일을 모두 같은 버전으로 맞춥니다.
  - `package.json`, `package-lock.json`
  - `Cargo.toml`, `Cargo.lock`
  - `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
  - `src-tauri/tauri.conf.json` (`build-release.ps1`이 이 파일에서 버전을 읽음)
- 태그와 릴리즈 이름은 `vX.Y.Z` 형식을 사용합니다.
- 버전 릴리즈 커밋에는 위 버전 변경만 포함하고, 빌드 산출물은 GitHub Release asset으로만 업로드합니다.

## 커밋

- 커밋 메시지는 Conventional Commits 접두어 + 한국어 설명 형식을 따릅니다. 예: `feat: 숫자 불일치 경고 추가`, `fix: ...`, `chore: ...`
- 커밋 제외 대상은 `.gitignore`에서 관리합니다(빌드 산출물, `logs/`, `state/`, `translation_work/`, `backups/`, `presets/`, `vault/`, 로컬 도구 디렉터리 등). `.gitignore`에 등록된 경로는 `git add -f`로 강제 추가하지 않습니다.
- 커밋 전에는 항상 `git status --short`로 의도하지 않은 파일이 포함되지 않았는지 확인하고, `git add -A` 대신 파일을 지정해 스테이징합니다.
