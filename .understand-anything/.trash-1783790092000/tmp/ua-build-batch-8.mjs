import fs from 'node:fs';
import path from 'node:path';

const root = 'D:/github/STS2-Mod-Translation-Manager';
const extraction = JSON.parse(fs.readFileSync(path.join(root, '.understand-anything/tmp/ua-file-extract-results-8.json'), 'utf8'));
const input = JSON.parse(fs.readFileSync(path.join(root, '.understand-anything/tmp/ua-file-analyzer-input-8.json'), 'utf8'));
const outputDir = path.join(root, '.understand-anything/intermediate');
if (!extraction.scriptCompleted || extraction.results.length !== input.batchFiles.length) throw new Error('batch 8 구조 추출 결과 불일치');

const fileInfo = new Map(input.batchFiles.map((file) => [file.path, file]));
const descriptions = {
  '.gitattributes': ['저장소 텍스트 파일의 LF 개행을 고정하고 이미지·실행 파일·동적 라이브러리를 binary로 분류합니다.', ['git', 'repository-policy', 'line-endings']],
  '.nvmrc': ['프런트엔드 도구 실행에 사용할 Node.js 22.12.0 버전을 고정합니다.', ['nodejs', 'runtime-version', 'tooling']],
  '.understand-anything/.understandignore': ['지식 그래프 분석에서 로컬 상태, 생성물, vendor, sample과 asset 경로를 제외하는 범위를 정의합니다.', ['analysis-config', 'ignore-rules', 'local-state']],
  '.understand-anything/config.json': ['Understand-Anything 지식 그래프의 출력 언어를 한국어로 지정합니다.', ['configuration', 'knowledge-graph', 'localization']],
  'replace.cjs': ['로컬 SettingsPage.tsx의 경로·도구·로그 섹션을 접이식 details UI로 일괄 치환하는 일회성 마이그레이션 스크립트입니다.', ['migration-script', 'text-replacement', 'settings-ui', 'nodejs']],
  'src-tauri/build.rs': ['Tauri 빌드 스크립트를 호출해 데스크톱 애플리케이션용 생성 단계와 리소스 처리를 수행합니다.', ['build-system', 'tauri', 'entry-point']],
  'src-tauri/capabilities/default.json': ['메인 데스크톱 창에 창 제어와 파일 열기 dialog 권한을 부여하는 Tauri capability 정책입니다.', ['configuration', 'tauri', 'permissions', 'security']],
  'src-tauri/Cargo.toml': ['Tauri 데스크톱 crate의 패키지 정보와 Serde, Tauri 플러그인, 코어 crate 의존성을 선언합니다.', ['configuration', 'cargo', 'tauri', 'build-system']],
  'src-tauri/src/services/cache_cleanup.rs': ['번역 추출·드롭 미리보기·작업공간 cache 사용량을 계산하고 고아 또는 중복 payload를 정리합니다.', ['cache-management', 'filesystem', 'cleanup', 'tauri-service']],
  'src-tauri/src/services/common.rs': ['Tauri 서비스가 공유하는 런타임 경로, App 구성, DTO 보조 타입, 성능 추적과 공통 경로 유틸리티를 제공합니다.', ['shared-utility', 'runtime-config', 'performance', 'tauri-service']],
  'src-tauri/src/services/dashboard.rs': ['대시보드 로드와 모드 가져오기·전환·복구·프리셋·실행 동작을 조정하고 최신 DashboardDto를 구성합니다.', ['dashboard', 'orchestration', 'mod-management', 'tauri-service']],
  'src-tauri/src/services/dashboard/activation.rs': ['전체 스캔 결과에서 안정 key가 일치하는 모드 레코드를 찾는 dashboard 활성화 보조 로직입니다.', ['dashboard', 'mod-resolution', 'utility', 'tauri-service']],
  'src-tauri/src/services/dashboard/dto_maps.rs': ['코어 도메인의 프리셋·번역 작업·백업·실행·도구 보고서를 frontend DTO로 변환합니다.', ['dto-mapping', 'serialization', 'dashboard', 'tauri-service']],
  'src-tauri/src/services/dashboard/mod_rows.rs': ['스캔 레코드를 dashboard 모드 행으로 통합하고 manifest, 번역 상태, 의존성 및 패치 관계를 해석합니다.', ['dashboard', 'dependency-resolution', 'translation-preview', 'tauri-service']],
  'src-tauri/src/services/dashboard/setup_status.rs': ['필수 경로와 중첩 모드 배치를 검사해 초기 설정 문제 및 안전 경고를 생성합니다.', ['setup-validation', 'path-checking', 'diagnostics', 'tauri-service']],
  'src-tauri/src/services/deleted_mods.rs': ['삭제 모드를 격리 저장하고 tombstone index, 보존 기간, 복원 및 일괄 비우기 수명주기를 관리합니다.', ['deleted-mods', 'quarantine', 'persistence', 'tauri-service']],
  'src-tauri/src/services/json_sheet.rs': ['JSON 번역 시트 생성·저장·검증·가져오기·내보내기와 번역 적용 및 PCK 재패키징 흐름을 연결합니다.', ['json-sheet', 'translation', 'pck', 'tauri-service']],
  'src-tauri/src/services/json_sheet/dto.rs': ['JSON 번역 도메인 타입과 frontend DTO 사이의 상태·보고서·항목 변환을 담당합니다.', ['dto-mapping', 'json', 'translation', 'serialization']],
  'src-tauri/src/services/logs_diagnostics.rs': ['게임 로그의 제한된 tail을 읽고 설정·모드·의존성·백업 상태에서 문제 해결 진단을 생성합니다.', ['diagnostics', 'logging', 'troubleshooting', 'tauri-service']],
  'src-tauri/src/services/pck.rs': ['번역 payload를 Godot PCK에 패치하고 archive 모드를 다시 설치하거나 독립 번역 패치 모드로 내보냅니다.', ['pck', 'translation-patch', 'archive', 'tauri-service']],
  'src-tauri/src/services/settings.rs': ['UI 설정을 검증하고 TSV 상태 파일에 저장하며 게임·세이브·번역 작업 경로를 AppConfig에 반영합니다.', ['settings', 'configuration', 'validation', 'tauri-service']],
  'src-tauri/src/services/tests.rs': ['대시보드, 설정, 삭제·복원, 번역 미리보기, JSON 시트와 PCK 서비스 동작을 검증하는 통합 테스트 모음입니다.', ['test', 'tauri-service', 'integration', 'regression']],
  'src-tauri/src/services/translation_preview.rs': ['모드 resource를 추출하고 선택한 번역 node의 source/translated 작업공간과 번역 시트를 준비합니다.', ['translation-preview', 'extraction', 'workspace', 'tauri-service']],
  'src-tauri/src/services/translation_preview/extraction_cache.rs': ['archive와 PCK를 외부 도구로 확장하고 fingerprint 기반 미리보기 cache 및 payload 탐색을 관리합니다.', ['extraction-cache', 'pck', 'archive', 'filesystem']],
  'src-tauri/src/services/translation_preview/extraction_tree.rs': ['번역·하드코딩 후보를 표시하는 제한된 계층형 extraction tree DTO를 파일 경로에서 구성합니다.', ['extraction-tree', 'translation-preview', 'filesystem', 'dto']]
};

const nodeIds = new Set();
const edgeKeys = new Set();
const nodes = [];
const edges = [];
const significant = new Map();
const fileNodeId = new Map();

const complexity = (lines) => lines > 200 ? 'complex' : (lines >= 50 ? 'moderate' : 'simple');
const unique = (values) => [...new Set(values.filter(Boolean))].slice(0, 5);
function levelId(file) { return file.fileCategory === 'config' ? `config:${file.path}` : `file:${file.path}`; }
function addNode(node) { if (nodeIds.has(node.id)) throw new Error(`중복 노드: ${node.id}`); nodeIds.add(node.id); nodes.push(node); }
function addEdge(edge) { const key = `${edge.source}\0${edge.target}\0${edge.type}`; if (!edgeKeys.has(key) && edge.source !== edge.target) { edgeKeys.add(key); edges.push(edge); } }

function tagsForFunction(name, filePath) {
  const n = name.toLowerCase();
  const tags = [];
  if (filePath.endsWith('/tests.rs') || n.startsWith('test_')) tags.push('test', 'regression');
  else if (/^(ensure|validate|check)/.test(n)) tags.push('validation', 'guard');
  else if (/^(read|load)/.test(n)) tags.push('deserialization', 'file-io');
  else if (/^(write|save|record|remember)/.test(n)) tags.push('serialization', 'persistence');
  else if (/^(scan|collect|list|find)/.test(n)) tags.push('discovery', 'filesystem');
  else if (/^(copy|move|remove|delete|restore|prune|cleanup|clear|quarantine)/.test(n)) tags.push('filesystem', 'lifecycle');
  else if (/^(parse|split|normalize|sanitize|infer|resolve|default|preferred)/.test(n)) tags.push('resolution', 'utility');
  else if (/^(apply|import|export|extract|expand|build|pack|prepare|recalculate)/.test(n)) tags.push('workflow', 'transformation');
  else if (/(dashboard|dto)/.test(n)) tags.push('dashboard', 'dto-mapping');
  else if (/(cache)/.test(n)) tags.push('cache-management', 'state');
  else if (/^(is_|has_|looks_|can_|should_)/.test(n)) tags.push('predicate', 'validation');
  else tags.push('domain-logic', 'service');
  return unique([...tags, 'rust']);
}

function summaryForFunction(name, filePath) {
  const n = name.toLowerCase(); const d = `\`${name}\``;
  if (filePath.endsWith('/tests.rs') || n.startsWith('test_')) return `${d} 시나리오의 Tauri 서비스 결과와 파일시스템 부작용을 검증하는 회귀 테스트입니다.`;
  if (/^(is_|has_|looks_|can_|should_)/.test(n)) return `${d} 조건을 판정해 서비스 워크플로의 분기 기준을 제공합니다.`;
  if (/^(ensure|validate|check)/.test(n)) return `${d} 입력·경로·상태 제약을 검증하고 위험한 작업을 차단합니다.`;
  if (/^(read|load)/.test(n)) return `${d} 저장된 데이터를 읽고 frontend 또는 상위 서비스가 사용할 구조로 복원합니다.`;
  if (/^(write|save|record|remember)/.test(n)) return `${d} 서비스 상태나 분석 결과를 직렬화해 저장합니다.`;
  if (/^(scan|collect|list|find)/.test(n)) return `${d} 관련 경로와 레코드를 순회해 서비스 처리 대상을 수집합니다.`;
  if (/^(copy|move|remove|delete|restore|prune|cleanup|clear|quarantine)/.test(n)) return `${d} 관리 대상 파일과 cache의 수명주기 작업을 안전하게 수행합니다.`;
  if (/^(parse|split|normalize|sanitize|infer|resolve|default|preferred)/.test(n)) return `${d} 입력과 경로 단서를 해석해 일관된 내부 값으로 결정합니다.`;
  if (/^(apply|import|export|extract|expand|build|pack|prepare|recalculate)/.test(n)) return `${d} 번역 또는 모드 데이터를 변환해 다음 단계의 결과물을 생성합니다.`;
  if (/(dashboard)/.test(n)) return `${d} 현재 서비스 상태를 집계해 dashboard 응답을 구성합니다.`;
  if (/(dto)/.test(n)) return `${d} 코어 도메인 값을 frontend 전송용 DTO로 변환합니다.`;
  if (/(cache)/.test(n)) return `${d} 재사용 가능한 작업 결과와 cache 메타데이터를 관리합니다.`;
  return `${d} 관련 Tauri 서비스의 도메인 규칙을 구현하고 처리 결과를 반환합니다.`;
}

for (const result of extraction.results) {
  const file = fileInfo.get(result.path); if (!file) throw new Error(`알 수 없는 파일: ${result.path}`);
  const id = levelId(file); fileNodeId.set(file.path, id);
  const [summary, tags] = descriptions[file.path] ?? [`${file.path}의 프로젝트 역할을 구현하거나 구성합니다.`, ['source-code', 'project-config', 'utility']];
  addNode({ id, type: file.fileCategory === 'config' ? 'config' : 'file', name: path.posix.basename(file.path), filePath: file.path, summary, tags, complexity: complexity(result.nonEmptyLines) });
  const exported = new Set((result.exports ?? []).map((entry) => entry?.name).filter(Boolean));
  const sig = { functions: new Map(), classes: new Map() };
  for (const fn of result.functions ?? []) {
    if (!fn?.name) continue; const lines = fn.endLine - fn.startLine + 1;
    if (lines < 10 && !exported.has(fn.name)) continue;
    if (sig.functions.has(fn.name)) throw new Error(`함수 ID 충돌: ${result.path}:${fn.name}`);
    const functionId = `function:${result.path}:${fn.name}`;
    addNode({ id: functionId, type: 'function', name: fn.name, filePath: result.path, lineRange: [fn.startLine, fn.endLine], summary: summaryForFunction(fn.name, result.path), tags: tagsForFunction(fn.name, result.path), complexity: complexity(lines) });
    sig.functions.set(fn.name, functionId);
    addEdge({ source: id, target: functionId, type: 'contains', direction: 'forward', weight: 1.0 });
    if (exported.has(fn.name)) addEdge({ source: id, target: functionId, type: 'exports', direction: 'forward', weight: 0.8 });
  }
  for (const cls of result.classes ?? []) {
    if (!cls?.name) continue; const lines = cls.endLine - cls.startLine + 1; const methodCount = Array.isArray(cls.methods) ? cls.methods.length : 0;
    if (lines < 20 && methodCount < 2 && !exported.has(cls.name)) continue;
    if (sig.classes.has(cls.name)) throw new Error(`타입 ID 충돌: ${result.path}:${cls.name}`);
    const classId = `class:${result.path}:${cls.name}`;
    addNode({ id: classId, type: 'class', name: cls.name, filePath: result.path, lineRange: [cls.startLine, cls.endLine], summary: `\`${cls.name}\`는 ${summary.replace(/입니다\.$/, '')}에 필요한 상태와 값을 표현하는 Rust 타입입니다.`, tags: unique(['data-model', 'rust-type', file.path.includes('translation') ? 'translation' : 'tauri-service', 'rust']), complexity: methodCount > 8 || lines > 200 ? 'complex' : (methodCount >= 3 || lines >= 50 ? 'moderate' : 'simple') });
    sig.classes.set(cls.name, classId);
    addEdge({ source: id, target: classId, type: 'contains', direction: 'forward', weight: 1.0 });
    if (exported.has(cls.name)) addEdge({ source: id, target: classId, type: 'exports', direction: 'forward', weight: 0.8 });
  }
  significant.set(result.path, sig);
}

let importEdges = 0;
for (const file of input.batchFiles) for (const target of input.batchImportData[file.path] ?? []) {
  importEdges += 1;
  addEdge({ source: fileNodeId.get(file.path), target: `file:${target}`, type: 'imports', direction: 'forward', weight: 0.7 });
}
if (importEdges !== 0 || edges.some((edge) => edge.type === 'imports')) throw new Error('batch 8 imports edge는 0개여야 합니다.');

const parts = Math.ceil(Math.max(nodes.length / 60, edges.length / 120));
const sortedFiles = input.batchFiles.map((file) => file.path).sort();
const chunkSize = Math.ceil(sortedFiles.length / parts);
const summaries = [];
for (let i = 0; i < parts; i += 1) {
  const files = sortedFiles.slice(i * chunkSize, (i + 1) * chunkSize); if (!files.length) continue;
  const fileSet = new Set(files);
  const partNodes = nodes.filter((node) => fileSet.has(node.filePath));
  const partIds = new Set(partNodes.map((node) => node.id));
  const partEdges = edges.filter((edge) => partIds.has(edge.source));
  for (const edge of partEdges) if (!partIds.has(edge.target)) throw new Error(`part ${i + 1} 외부 참조: ${edge.target}`);
  const outputPath = path.join(outputDir, `batch-8-part-${i + 1}.json`);
  fs.writeFileSync(outputPath, `${JSON.stringify({ nodes: partNodes, edges: partEdges }, null, 2)}\n`, 'utf8');
  JSON.parse(fs.readFileSync(outputPath, 'utf8'));
  summaries.push({ part: i + 1, files, nodes: partNodes.length, edges: partEdges.length });
}
console.log(JSON.stringify({ files: input.batchFiles.length, nodes: nodes.length, edges: edges.length, functions: [...significant.values()].reduce((n, x) => n + x.functions.size, 0), classes: [...significant.values()].reduce((n, x) => n + x.classes.size, 0), importEdges, parts: summaries }, null, 2));
