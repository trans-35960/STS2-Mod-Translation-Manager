import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const extract = JSON.parse(fs.readFileSync(path.join(root, ".understand-anything/tmp/ua-file-extract-results-3.json"), "utf8"));
const batches = JSON.parse(fs.readFileSync(path.join(root, ".understand-anything/intermediate/batches.json"), "utf8"));
const batch = batches.batches.find((item) => item.batchIndex === 3);
if (!batch || extract.scriptCompleted !== true || extract.filesAnalyzed !== batch.files.length) {
  throw new Error("batch 3 extraction metadata mismatch");
}

const fileMeta = {
  "web/components/Common.tsx": ["여러 화면에서 공유하는 출처 태그, 아이콘 버튼, 상태 배지와 pill 컴포넌트를 제공한다.", ["component", "shared-ui", "presentation"]],
  "web/features/mods/ExtractModal.tsx": ["모드 번역 리소스의 추출 경로와 파일 트리를 선택하고 전체 또는 개별 노드 추출을 실행하는 확인 모달을 구현한다.", ["component", "mod-management", "file-tree"]],
  "web/features/mods/ImportMenu.tsx": ["폴더, 압축 파일, Vortex 다운로드에서 모드를 가져오는 동작을 묶은 팝오버 메뉴를 제공한다.", ["component", "mod-import", "menu"]],
  "web/features/mods/ModBadges.tsx": ["모드의 대표 언어와 사용 가능한 언어 목록을 키 수 및 대상 언어 상태와 함께 배지로 표시한다.", ["component", "language", "mod-management"]],
  "web/features/mods/ModRows.tsx": ["상세·간단 보기의 모드 및 모드 그룹 행을 렌더링하고 선택, 활성화, 번역, 추출, 삭제 동작을 연결한다.", ["component", "mod-management", "table-row"]],
  "web/features/mods/ModTranslationActions.tsx": ["자동 감지 언어가 없는 모드에서 번역 리소스 경로를 파일 트리 또는 수동 입력으로 고르는 UI를 제공한다.", ["component", "translation", "file-tree"]],
  "web/features/mods/ModsPage.tsx": ["모드 검색·필터·정렬·그룹화·선택과 프리셋, 가져오기, 실행 및 열 너비 상태를 총괄하는 메인 관리 화면이다.", ["component", "mod-management", "state-management"]],
  "web/features/mods/PresetMenu.tsx": ["활성 모드 프리셋의 저장·선택·적용과 아카이브 가져오기·내보내기를 제어하는 메뉴를 구현한다.", ["component", "preset", "menu"]],
  "web/features/mods/modUtils.ts": ["모드 그룹화, 검색·필터·정렬, 의존성·버전 병합, 언어 추천, 날짜·경로 표시를 지원하는 도메인 유틸리티 모음이다.", ["utility", "mod-management", "normalization"]],
  "web/features/translation/TranslationActionsPanel.tsx": ["번역 작업 헤더와 프로젝트·언어·비교·내보내기·적용 작업을 제공하는 사이드 액션 패널을 구성한다.", ["component", "translation", "workflow"]],
  "web/features/translation/TranslationProjectTree.tsx": ["번역 프로젝트의 파일 트리를 탐색하고 노드 선택 및 JSON 복사 컨텍스트 메뉴를 제공한다.", ["component", "translation", "file-tree"]],
  "web/features/translation/TranslationSheetTable.tsx": ["번역 시트의 검색·치환·선택·검증 경고·붙여넣기 충돌과 행 편집기를 제공하는 대형 테이블 컴포넌트다.", ["component", "translation", "data-table"]],
  "web/features/translation/TranslationStartPage.tsx": ["번역 가능한 모드와 리소스 경로를 검색·그룹화하여 새 번역 작업을 시작하는 선택 화면을 구현한다.", ["component", "translation", "workflow"]],
  "web/features/translation/TranslationToolsPage.tsx": ["번역 작업 헤더, 액션 패널, 시트 테이블을 조합하고 번역 유틸리티의 공개 진입점을 재노출한다.", ["component", "translation", "entry-point"]],
  "web/features/translation/TranslationToolsTypes.ts": ["번역 도구 페이지의 props, 열 설정, 필터 및 표시 행 계약을 정의한다.", ["type-definition", "translation", "typescript"]],
  "web/features/translation/TranslationWidgets.tsx": ["적용 결과, 비교 값, 붙여넣기 후보, 프로젝트 요약, 크기 조절 헤더와 자동 높이 입력기를 제공한다.", ["component", "translation", "shared-ui"]],
  "web/features/translation/translationUtils.ts": ["번역 키·경로·언어 정규화, 프로젝트 트리, 구조화 붙여넣기, slot ID 및 비교 키를 처리하는 핵심 유틸리티다.", ["utility", "translation", "parsing"]],
  "web/features/translation/useTranslationToolsUiState.ts": ["번역 시트에서 언어 옵션, 통계, 프로젝트 트리, 필터·검색 결과와 열 크기 조절 상태를 파생하는 React hook이다.", ["hook", "translation", "state-management"]],
  "web/i18n.ts": ["모드 관리와 번역 도구 전반에서 사용하는 한국어·영어 UI 라벨 사전을 정의한다.", ["localization", "configuration", "ui-copy"]]
};

const descriptions = {
  SourceTags: "복합 출처 문자열을 제한된 수의 태그로 표시한다.", IconButton: "아이콘과 접근성 라벨을 갖춘 공용 동작 버튼을 렌더링한다.", StatusBadge: "상태 의미에 따른 색조 배지를 렌더링한다.", Pill: "짧은 상태 텍스트를 pill 형태로 표시한다.",
  ExtractConfirmModal: "추출 대상 모드와 출력 경로를 확인하고 전체 또는 선택 노드 추출을 전달한다.", FileTree: "추출 가능한 리소스 노드 목록을 재귀 트리로 렌더링한다.", FileTreeNode: "개별 추출 트리 노드의 펼침, 선택, 컨텍스트 동작을 처리한다.",
  ImportMenu: "지원되는 모드 가져오기 경로를 선택하는 메뉴의 열림 상태와 실행을 관리한다.", RepresentativeLanguageBadge: "대상 언어 또는 대표 원본 언어 한 개를 요약 배지로 표시한다.", LanguageBadges: "사용 가능한 언어를 키 수와 대상·대표 상태가 포함된 배지 목록으로 표시한다.",
  ModTableRow: "상세 모드 행에 메타데이터, 경고, 언어와 주요 관리 동작을 렌더링한다.", ModGroupTableRow: "상세 보기에서 여러 버전 모드 그룹의 요약과 그룹 동작을 렌더링한다.", SimpleModGroupRow: "간단 보기에서 모드 그룹의 핵심 상태와 펼침·토글 동작을 렌더링한다.", SimpleModRow: "간단 보기의 개별 모드 상태와 주요 동작을 렌더링한다.", ModSelectionCheckbox: "삭제 가능한 모드 또는 그룹의 선택 체크박스를 렌더링한다.", ChangeReasonList: "모드 변경 사유를 중복 제거된 배지 목록으로 표시한다.", DependencyWarningIcon: "비활성·누락·버전 불일치 의존성 경고를 툴팁 아이콘으로 표시한다.", isExpandableRowBlankClick: "그룹 행의 빈 영역 클릭인지 판별해 오동작을 막는다.", ToggleSwitch: "진행 상태와 잠금 상태를 반영하는 모드 활성화 스위치를 렌더링한다.", TranslationApplyCell: "번역 적용 여부와 적용 시각을 모드 행에 표시한다.", DependencyList: "모드 의존성의 활성·가용·버전 상태를 표시한다.",
  ModTranslationActions: "자동 언어 후보가 없는 모드의 수동 번역 경로 선택 UI를 관리한다.", LanguageCandidateTree: "번역 후보 리소스 목록을 재귀 트리의 루트로 렌더링한다.", LanguageCandidateNode: "번역 후보 트리 노드의 경로 계산, 펼침 및 선택을 처리한다.",
  ModsPage: "모드 목록의 필터, 정렬, 선택, 그룹, 열 설정과 전체 관리 동작을 조정한다.", ModResizableHead: "모드 테이블 열의 크기 조절 손잡이가 포함된 헤더를 렌더링한다.", startColumnResize: "포인터 이동 동안 CSS 열 너비를 갱신하고 완료 값을 상태에 반영한다.", detailColumnVar: "상세 보기 열을 대응하는 CSS 사용자 속성으로 변환한다.", simpleColumnVar: "간단 보기 열을 대응하는 CSS 사용자 속성으로 변환한다.", attachedTranslationMods: "그룹에서 원본 모드와 연결된 번역 모드 자식을 분리한다.", PresetMenu: "프리셋 선택·저장·적용과 아카이브 입출력 메뉴 상태를 관리한다.",
  TranslationWorkHeader: "현재 번역 프로젝트와 진행률을 요약하고 저장·검증·적용 동작을 제공한다.", TranslationActionsPanel: "프로젝트 트리, 원본·대상·비교 언어와 내보내기 동작을 배치한다.", TranslationProjectTree: "프로젝트 트리의 선택 및 컨텍스트 메뉴 상태를 관리한다.", TranslationProjectTreeNode: "프로젝트 트리 노드를 재귀 렌더링하고 선택·복사 동작을 연결한다.",
  TranslationSheetTable: "번역 행 편집과 검색·치환·선택·검증·충돌 해결 인터페이스를 총괄한다.", TranslationValueEditor: "번역값 초안을 유지하고 지연 commit 및 구조화 붙여넣기를 처리한다.", areJsonEntryRowsEqual: "번역 행의 의미 있는 props만 비교해 불필요한 재렌더링을 차단한다.", HighlightedText: "검색어가 일치하는 문자열 구간을 mark 요소로 분할해 표시한다.", HighlightedIssueText: "구조 검증 문제와 검색어 범위를 함께 강조한다.", IssueBadges: "행별 검증 문제 종류를 선택 가능한 배지로 표시한다.", issueHighlightRanges: "태그·placeholder·줄바꿈 토큰의 강조 범위를 계산한다.", collectPatternRanges: "정규식 일치 구간을 검증 문제 범위 목록에 추가한다.", compareValueForEntry: "언어별 비교 맵에서 현재 번역 엔트리에 대응하는 값을 조회한다.", ValidationMetric: "검증 결과의 단일 수치와 상태 색조를 표시한다.", validationRows: "검증 결과 배열을 중복 건수와 메시지가 포함된 표시 행으로 정규화한다.", validationIssueLabel: "검증 문제 코드를 사용자용 짧은 라벨로 변환한다.",
  TranslationPage: "번역 가능한 모드와 기존 작업을 검색·그룹화하여 시작 카드를 렌더링한다.", TranslationStartCard: "한 모드의 언어 후보와 수동 경로를 이용한 번역 시작 동작을 제공한다.", TranslationToolsPage: "번역 도구의 파생 UI 상태를 연결하고 헤더·패널·시트 테이블을 조합한다.",
  ApplyStatusToast: "번역 적용·패키징 결과를 닫을 수 있는 알림으로 표시한다.", CompareStack: "선택한 언어별 비교 문자열을 세로 목록으로 표시한다.", PasteCandidateCard: "기존 값과 충돌하는 붙여넣기 후보의 허가·취소 동작을 제공한다.", ProjectSummary: "프로젝트 메타데이터와 언어 목록을 요약한다.", ResizableHead: "번역 테이블 열의 크기 조절 헤더를 렌더링한다.", AutoGrowTextarea: "포커스된 입력의 scrollHeight에 맞춰 높이를 자동 조정한다.",
  useTranslationToolsUiState: "번역 도구의 필터·검색·열·언어 상태와 시트 기반 파생 데이터를 관리한다.", columnCssVariable: "번역 열 키를 CSS 사용자 속성 이름으로 변환한다.", cachedSplitSheetKey: "번역 키 분해 결과를 캐시에서 조회하거나 계산해 저장한다.", validationIssueKeys: "검증 결과에서 선택한 문제 종류에 해당하는 엔트리 키 집합을 만든다."
};

const utilityDescriptions = {
  presetPreviewSummary:"프리셋 적용 전 활성화·비활성화·누락·버전·의존성 변화를 요약한다.", shortPath:"긴 경로를 마지막 세 구간 중심으로 축약한다.", languageKeyCount:"언어 미리보기의 유효 키 수를 반환한다.", uniqueLanguagePreviews:"언어 코드별 대표 미리보기를 선택해 키 수 기준으로 정렬한다.", normalizeLanguageTag:"언어 별칭을 내부 표준 코드로 정규화한다.", recommendedSourceLanguage:"키가 가장 풍부한 원본 언어 후보를 선택한다.", representativeLanguage:"대상 언어를 우선하고 없으면 권장 원본 언어를 선택한다.", buildModGroups:"검색·필터 조건을 적용해 모드를 병합·정렬된 표시 그룹으로 구성한다.", modDisplayGroupName:"번역 패치의 대상 정보를 이용해 표시 그룹 이름을 결정한다.", buildTargetGroupsByToken:"원본 모드 식별 토큰에서 그룹 이름으로 가는 인덱스를 만든다.", modTargetTokens:"원본 모드와 그룹에서 매칭용 식별 토큰을 생성한다.", translationPatchTargetTokens:"번역 패치의 대상·의존성 정보에서 매칭 토큰을 생성한다.", matchesStatFilter:"모드가 선택한 대시보드 통계 필터와 일치하는지 판별한다.", activeSiblingMods:"같은 활성화 그룹에서 현재 활성 상태인 형제 모드를 찾는다.", preferredGroupActivationTarget:"그룹에서 다운로드 중이 아닌 최적 활성화 대상을 고른다.", compareGroupActivationTarget:"그룹 활성화 후보의 원본·관리·시각·버전 우선순위를 비교한다.", findMergeIndex:"표시 그룹 안에서 같은 모드 또는 보완 가능한 행의 위치를 찾는다.", mergeModRows:"여러 출처에서 감지된 동일 모드 행의 상태와 메타데이터를 병합한다.", mergeDependencies:"의존성 ID를 기준으로 가용성·활성·버전 정보를 병합한다.", compareMods:"선택한 정렬 방식과 이름·버전 보조키로 모드 순서를 비교한다.", matchesModFilters:"활성·변경·번역 적용 필터를 한 번에 평가한다.", canDeleteMod:"모드가 삭제 가능한 관리 대상인지 판별한다.", isDownloadingMod:"외부 모드가 현재 다운로드 중인지 판별한다.", modSearchTokens:"모드 검색에 사용할 이름·ID·경로·의존성 토큰을 수집한다.", modGroupName:"명시적 그룹 또는 정제된 이름으로 모드 그룹명을 계산한다.", modActivationGroupName:"함께 활성화할 수 없는 버전군의 그룹명을 계산한다.", inferredVersionFromName:"모드 이름에서 버전 형태의 접미사를 추론한다.", displayModVersion:"명시 또는 추론된 표시 버전을 반환한다.", compactSourceSummary:"그룹의 출처 라벨을 중복 제거해 요약한다.", compactVersionSummary:"그룹의 버전 목록을 축약한다.", compactLanguageSummary:"그룹의 감지 언어 상태를 축약한다.", activeModSummary:"그룹에서 활성 모드 또는 대표 모드를 선택한다.", activeModVersionSummary:"그룹의 활성 모드 버전을 요약한다.", compactDateSummary:"그룹의 최신 등록 시각을 짧게 표시한다.", compactModifiedSummary:"그룹의 최신 수정 시각을 짧게 표시한다.", groupTranslationSummary:"그룹의 번역 적용 및 패치 활성 상태를 요약한다.", compactTranslationApplyDate:"그룹의 최신 번역 적용 시각을 표시한다.", formatShortDate:"epoch 값을 짧은 날짜 문자열로 변환한다.", formatFullDateTime:"epoch 값을 전체 날짜·시각 문자열로 변환한다.", formatBytes:"바이트 수를 읽기 쉬운 단위로 변환한다.", joinResourcePath:"리소스 기준 경로와 자식 이름을 정규화해 결합한다.", languageResourceRoot:"언어 샘플 경로에서 localization 언어 루트를 추출한다.", defaultTranslationResourcePath:"모드의 기본 번역 리소스 경로를 결정한다.", defaultTranslatableResourcePath:"번역 가능한 우선 리소스 경로를 결정한다.", needsDeferredTranslationAnalysis:"언어 후보를 나중에 분석해야 하는 모드인지 판별한다.", isHardcodedResourcePath:"리소스 경로가 하드코딩 문자열 후보인지 판별한다.", translationTargetOptions:"현재 설정과 감지 언어에서 대상 언어 선택지를 구성한다.", parentPath:"경로의 부모 구간을 반환한다.", languageResourceName:"리소스 경로의 마지막 이름을 반환한다.", firstHardcodedResourcePath:"추출 트리에서 첫 하드코딩 리소스 경로를 찾는다.", hasLocalizationBranch:"트리 노드 아래에 localization 관련 가지가 있는지 검사한다.", languageFolderCode:"언어 샘플 경로 또는 코드에서 폴더 언어 코드를 얻는다.", languageLabel:"언어 코드를 사용자용 언어 이름으로 변환한다.",
  createCompareValueMap:"언어 비교 결과를 원본·정규화·안정 키로 조회 가능한 맵으로 만든다.", buildTranslationProjectTree:"시트 엔트리 경로와 상태를 집계해 번역 프로젝트 트리를 구축한다.", bestLocalizationRoot:"프로젝트 트리에서 가장 대표적인 localization 루트를 고른다.", collectLocalizationNodes:"트리를 재귀 순회해 localization 이름의 노드를 수집한다.", sheetEntryFilePath:"복합 번역 키에서 파일 경로를 추출한다.", splitSheetKey:"file 스킴 번역 키를 파일과 엔트리 키로 분리한다.", languageCodeFromSheetKey:"번역 키의 localization 경로에서 언어 코드를 추출한다.", languageCodeFromSourcePath:"원본 경로의 localization 구간에서 언어 코드를 추출한다.", normalizedLocalizationKey:"언어 폴더를 placeholder로 치환한 비교용 키를 만든다.", stableCompareKey:"언어 구간을 제거한 파일·엔트리 비교 키를 만든다.", pathMatchesProjectNode:"파일 경로가 선택한 프로젝트 노드 범위에 속하는지 판별한다.", parsePastedTranslationJson:"클립보드 문자열을 JSON 값으로 안전하게 파싱한다.", looksLikeJsonPaste:"문자열이 JSON 붙여넣기 후보 형태인지 빠르게 판별한다.", isStructuredTranslationJsonPaste:"문자열이 지원되는 구조화 번역 JSON인지 검사한다.", isTabularTranslationPaste:"문자열이 여러 행 또는 탭 기반 표 데이터인지 판별한다.", structuredTranslationEntries:"다양한 JSON 형태에서 번역 slot 엔트리를 재귀 추출한다.", compactTranslationEntries:"축약 JSON 객체에서 번역 엔트리를 재귀 추출한다.", isTranslationSlotId:"문자열이 번역 slot ID 형식인지 검사한다.", translationSlotEntries:"시트 엔트리에 파일별 안정 slot ID와 축약 파일명을 부여한다.", compactTranslationFile:"localization 언어 루트 앞부분을 제거해 파일 경로를 축약한다.", translationSlotId:"파일 내 순번과 체크섬으로 slot ID를 생성한다.", translationSlotKey:"파일과 slot ID를 충돌 없는 복합 키로 결합한다.", stableSlotKey:"번역 엔트리에서 언어에 독립적인 slot 체크섬 입력을 만든다.", slotChecksum:"안정 키의 FNV 해시를 짧은 36진수 체크섬으로 변환한다.", fnv64:"문자열 바이트에 64비트 FNV-1a 해시를 계산한다.", isTranslatableEntry:"삭제되지 않고 원본 값이 있는 엔트리인지 판별한다.", hasTranslationValue:"번역값이 비어 있지 않은지 판별한다.", whitespaceValueLabel:"공백만 있는 번역값의 길이 라벨을 만든다.", normalizeTranslationLanguageCode:"번역 언어 코드 별칭을 표준 3자리 코드로 정규화한다.", translationLanguagesMatch:"두 번역 언어 코드가 정규화 후 같은지 비교한다.", retargetTranslationSheetPath:"번역 시트 파일명의 대상 언어 구간을 교체한다.", inferPckTargetPath:"시트 원본과 엔트리 경로에서 PCK 적용 대상 경로를 추론한다.", replaceLocalizationLanguageInPath:"localization 경로의 언어 폴더를 대상 언어로 교체한다.", stripFileNameFromPckTarget:"PCK 대상이 파일이면 부모 디렉터리로 정규화한다.", incrementProjectNode:"엔트리 상태에 따라 프로젝트 트리 노드 집계값을 증가시킨다.", rollupProjectNode:"자식 노드의 번역 집계를 부모 노드로 재귀 합산한다.", projectNameFromPath:"원본 경로에서 프로젝트 표시 이름을 추출한다."
};

function complexity(lines) { return lines < 50 ? "simple" : lines <= 200 ? "moderate" : "complex"; }
function domainTags(file, name) {
  const domain = file.includes("/mods/") ? "mod-management" : file.includes("/translation/") ? "translation" : file.includes("i18n") ? "localization" : "shared-ui";
  if (name.startsWith("use")) return ["hook", domain, "state-management"];
  if (/^[A-Z]/.test(name)) return ["component", domain, "ui"];
  if (/^(parse|looks|isStructured|isTabular|structured|compactTranslationEntries)/.test(name)) return ["utility", domain, "parsing"];
  if (/^(format|compact|shortPath|languageLabel)/.test(name)) return ["utility", domain, "formatting"];
  if (/^(normalize|split|stable|slot|translationSlot|retarget|replace)/.test(name)) return ["utility", domain, "normalization"];
  return ["utility", domain, "domain-logic"];
}

const nodes = [];
const edges = [];
const significantByFile = new Map();
for (const result of extract.results) {
  const meta = fileMeta[result.path];
  if (!meta) throw new Error(`missing file metadata: ${result.path}`);
  nodes.push({ id:`file:${result.path}`, type:"file", name:path.posix.basename(result.path), filePath:result.path, summary:meta[0], tags:meta[1], complexity:complexity(result.nonEmptyLines) });
  const exported = new Set((result.exports ?? []).filter((item) => item.name).map((item) => item.name));
  const significant = (result.functions ?? []).filter((fn) => fn.name && ((fn.endLine - fn.startLine + 1) >= 10 || exported.has(fn.name)));
  significantByFile.set(result.path, new Set(significant.map((fn) => fn.name)));
  for (const fn of significant) {
    const summary = descriptions[fn.name] ?? utilityDescriptions[fn.name] ?? `${fn.name} 함수는 ${meta[0]}`;
    const id = `function:${result.path}:${fn.name}`;
    nodes.push({ id, type:"function", name:fn.name, filePath:result.path, lineRange:[fn.startLine,fn.endLine], summary, tags:domainTags(result.path, fn.name), complexity:complexity(fn.endLine-fn.startLine+1) });
    edges.push({ source:`file:${result.path}`, target:id, type:"contains", direction:"forward", weight:1.0 });
    if (exported.has(fn.name)) edges.push({ source:`file:${result.path}`, target:id, type:"exports", direction:"forward", weight:0.8 });
  }
}
for (const file of batch.files) {
  for (const target of batch.batchImportData[file.path]) {
    edges.push({ source:`file:${file.path}`, target:`file:${target}`, type:"imports", direction:"forward", weight:0.7 });
  }
}

const expectedImports = Object.values(batch.batchImportData).reduce((sum, list) => sum + list.length, 0);
if (edges.filter((edge) => edge.type === "imports").length !== expectedImports) throw new Error("import edge count mismatch");
if (nodes.filter((node) => node.type === "file").length !== batch.files.length) throw new Error("file node count mismatch");
if (new Set(nodes.map((node) => node.id)).size !== nodes.length) throw new Error("duplicate node id");

const parts = Math.ceil(Math.max(nodes.length / 60, edges.length / 120));
const sortedFiles = batch.files.map((item) => item.path).sort();
const chunkSize = Math.ceil(sortedFiles.length / parts);
const globalNodeIds = new Set(nodes.map((node) => node.id));
const allowedFiles = new Set([
  ...Object.values(batch.batchImportData).flat(),
  ...Object.values(batch.neighborMap).flat().map((item) => item.path),
]);
for (let index = 0; index < parts; index += 1) {
  const fileSet = new Set(sortedFiles.slice(index * chunkSize, (index + 1) * chunkSize));
  const partNodes = nodes.filter((node) => fileSet.has(node.filePath));
  const partIds = new Set(partNodes.map((node) => node.id));
  const partEdges = edges.filter((edge) => partIds.has(edge.source));
  const fragment = { nodes:partNodes, edges:partEdges };
  const parsed = JSON.parse(JSON.stringify(fragment));
  if (!Array.isArray(parsed.nodes) || !Array.isArray(parsed.edges)) throw new Error(`part ${index+1} malformed`);
  for (const edge of parsed.edges) {
    const targetKnown = globalNodeIds.has(edge.target) || (edge.target.startsWith("file:") && allowedFiles.has(edge.target.slice(5)));
    if (!partIds.has(edge.source) || !targetKnown || edge.source === edge.target) throw new Error(`part ${index+1} invalid edge ${JSON.stringify(edge)}`);
  }
  const output = path.join(root, `.understand-anything/intermediate/batch-3-part-${index+1}.json`);
  fs.writeFileSync(output, JSON.stringify(fragment, null, 2) + "\n", "utf8");
}

console.log(JSON.stringify({ parts, nodes:nodes.length, edges:edges.length, imports:expectedImports, skipped:extract.filesSkipped }));
