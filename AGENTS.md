# 에이전트 작업 규칙

## 빌드와 릴리즈

- 이 저장소의 GitHub Release 배포 대상은 `trans-35960/STS2-Mod-Translation-Manager`입니다.
- `gh`의 활성 계정이 다른 계정이어도 전역 계정을 함부로 바꾸지 않습니다. 릴리즈가 필요하면 먼저 `gh auth status`로 계정을 확인하고, 필요 시 사용자에게 확인한 뒤 배포용 계정 또는 `GH_TOKEN` 방식으로 진행합니다.
- 릴리즈 전에는 `cargo fmt --check`, `cargo test`, `npm run tauri build`, `scripts/build-release.ps1` 또는 동등한 smoke check를 가능한 범위에서 실행합니다.
- 버전 릴리즈를 만들 때는 소스/설정/락파일의 버전 변경만 커밋하고, 빌드 산출물은 GitHub Release asset으로만 업로드합니다.

## 커밋 제외

- 빌드/릴리즈 산출물과 로컬 작업 보조 파일은 커밋하지 않습니다.
- 특히 `.codex-local/`, `scripts/watch-dcinside-comments.ps1`, `target/`, `src-tauri/target/`, `dist/`, `logs/`, `state/`, `translation_work/`는 스테이징하지 않습니다.
- 커밋 전에는 항상 `git status --short`로 의도하지 않은 파일이 포함되지 않았는지 확인합니다.
