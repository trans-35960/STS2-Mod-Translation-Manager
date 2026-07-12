import fs from 'node:fs';
import path from 'node:path';

const root = 'D:/github/STS2-Mod-Translation-Manager';
const extractPath = path.join(root, '.understand-anything/tmp/ua-file-extract-results-2.json');
const inputPath = path.join(root, '.understand-anything/tmp/ua-file-analyzer-input-2.json');
const outputDir = path.join(root, '.understand-anything/intermediate');
const extraction = JSON.parse(fs.readFileSync(extractPath, 'utf8'));
const input = JSON.parse(fs.readFileSync(inputPath, 'utf8'));

if (!extraction.scriptCompleted || extraction.results.length !== input.batchFiles.length) {
  throw new Error('구조 추출 결과가 batchFiles와 일치하지 않습니다.');
}

const fileInfo = new Map(input.batchFiles.map((file) => [file.path, file]));
const importData = input.batchImportData;
const neighborMap = {};

const fileDescriptions = {
  'src/app.rs': ['모드 스캔, 활성 상태, 프리셋, 번역 작업, 세이브 백업과 게임 실행을 하나의 App 파사드로 조정하는 핵심 애플리케이션 서비스입니다.', ['application-service', 'orchestration', 'mod-management', 'rust']],
  'src/cli.rs': ['명령행 인수를 해석하고 App 기능을 호출한 뒤 사용자용 결과를 출력하는 CLI 명령 처리 계층입니다.', ['cli', 'command-handler', 'user-interface', 'rust']],
  'src/config.rs': ['워크스페이스 경로와 게임·세이브·외부 모드 관리자의 기본 위치를 구성하고 Steam 설치 정보를 탐지합니다.', ['configuration', 'path-resolution', 'steam', 'rust']],
  'src/discovery.rs': ['모드 디렉터리를 순회해 모드 종류, 이름, 버전 힌트와 파일 크기·수정 시각 fingerprint를 생성합니다.', ['discovery', 'filesystem', 'fingerprint', 'rust']],
  'src/domain.rs': ['모드 소스와 종류, 스캔 결과, 변경 내역 등 코어 계층이 공유하는 도메인 타입을 정의합니다.', ['data-model', 'domain', 'type-definition', 'rust']],
  'src/error.rs': ['경로를 포함한 I/O 오류와 잘못된 명령을 일관된 형태로 전달하는 애플리케이션 오류 타입을 정의합니다.', ['error-handling', 'data-model', 'rust']],
  'src/json_translation/apply.rs': ['번역 시트 값을 단일 JSON 또는 다중 localization 파일에 반영하고 결과 파일을 직렬화합니다.', ['json', 'translation', 'transformation', 'rust']],
  'src/json_translation/hardcoded.rs': ['.NET 메타데이터와 UTF-16 바이너리에서 하드코딩 문자열을 추출하고 용량 제약을 지키며 번역값을 패치합니다.', ['binary-parsing', 'hardcoded-translation', 'dotnet', 'rust']],
  'src/json_translation/import.rs': ['CSV와 여러 compact JSON 형식의 번역값을 읽어 안정된 slot/key 매핑으로 번역 시트에 병합합니다.', ['deserialization', 'translation-import', 'csv', 'json']],
  'src/json_translation/language_path.rs': ['localization 경로의 언어 코드를 판별·교체하고 대상 언어 파일에서 기존 번역값을 수집합니다.', ['language-resolution', 'path-mapping', 'translation', 'rust']],
  'src/json_translation/mod.rs': ['JSON 번역 하위 모듈을 선언하고 공개 API를 다시 노출하는 Rust 모듈 barrel입니다.', ['barrel', 'translation', 'module-api', 'rust']],
  'src/json_translation/sheet.rs': ['원본 JSON을 평탄화해 번역 시트를 생성·갱신하고 상태, 누락값, 구조 토큰과 하드코딩 용량을 검증합니다.', ['translation-sheet', 'validation', 'serialization', 'json']],
  'src/json_translation/slots.rs': ['번역 항목에 안정된 slot ID를 부여하고 파일별 compact 번역·검증 맵을 생성합니다.', ['slot-mapping', 'identity', 'translation', 'serialization']],
  'src/json_translation/source_json.rs': ['단일 JSON이나 디렉터리의 번역 JSON 파일을 읽고 JSON Pointer 기반 key/value 맵으로 평탄화합니다.', ['json', 'flattening', 'filesystem', 'deserialization']],
  'src/json_translation/tests.rs': ['시트 생성·재계산·검증·가져오기·내보내기·적용과 하드코딩 문자열 처리를 폭넓게 검증하는 회귀 테스트 모음입니다.', ['test', 'translation', 'json', 'regression']],
  'src/json_translation/types.rs': ['JSON 번역 시트, 항목 상태, 검증 문제와 작업 보고서의 직렬화 가능한 도메인 타입을 정의합니다.', ['type-definition', 'data-model', 'translation', 'serde']],
  'src/launcher.rs': ['게임과 Steam 실행 파일을 찾고 실행 상태를 확인하며 현재 또는 바닐라 모드로 프로세스를 시작합니다.', ['process-management', 'game-launcher', 'steam', 'rust']],
  'src/lib.rs': ['코어 crate의 공개 모듈 구성을 선언하는 Rust 라이브러리 진입점입니다.', ['entry-point', 'barrel', 'crate-root', 'rust']],
  'src/path_guard.rs': ['삭제·교체 대상이 허용된 root 내부인지 검증하고 canonical path 기준으로 위험한 경로 조작을 차단합니다.', ['path-security', 'validation', 'filesystem', 'rust']],
  'src/preset.rs': ['활성 모드 구성을 프리셋으로 저장·적용하고 관련 모드 파일을 포함한 archive를 가져오거나 내보냅니다.', ['preset', 'archive', 'mod-management', 'persistence']],
  'src/process.rs': ['Windows 콘솔 창을 숨긴 외부 명령 실행기와 PowerShell archive 압축·해제 도우미를 제공합니다.', ['process-management', 'powershell', 'archive', 'utility']],
  'src/save_backup.rs': ['바닐라·모드 세이브를 스냅샷으로 백업·복원·정리하고 current run 및 Steam Cloud cache를 안전하게 전환합니다.', ['backup', 'save-management', 'filesystem', 'lifecycle']],
  'src/state.rs': ['모드 fingerprint 상태와 사용자가 원하는 활성 모드 key를 읽고 쓰며 신규·변경 모드를 판별합니다.', ['state-management', 'fingerprint', 'persistence', 'rust']],
  'src/text_ui.rs': ['대화형 텍스트 메뉴에서 스캔, 프리셋, 모드 보관소와 게임 실행 명령을 안내하고 처리합니다.', ['text-ui', 'cli', 'interaction', 'rust']],
  'src/translation.rs': ['번역 후보 파일을 탐지하고 archive/PCK를 확장해 작업공간을 만들며 번역 결과를 대상 경로에 병합합니다.', ['translation', 'extraction', 'workspace', 'filesystem']],
  'src/vault.rs': ['게임 mods와 비활성 저장소 사이에서 모드를 이동·복사하고 충돌을 피하며 일괄 활성화 상태를 관리합니다.', ['mod-vault', 'filesystem', 'lifecycle', 'mod-management']],
  'src/vendor_tools.rs': ['번들된 7-Zip과 Godot PCK Explorer 실행 파일의 경로와 사용 가능 상태를 보고합니다.', ['vendor-tools', 'tool-discovery', 'configuration', 'rust']]
};

function complexityForLines(lines) {
  if (lines > 200) return 'complex';
  if (lines >= 50) return 'moderate';
  return 'simple';
}

function uniqueTags(values) {
  return [...new Set(values.filter(Boolean))].slice(0, 5);
}

function functionTags(name, filePath) {
  const lower = name.toLowerCase();
  const tags = [];
  if (filePath.includes('/tests.rs') || lower.startsWith('test_')) tags.push('test', 'regression');
  else if (/^(ensure|validate|check)/.test(lower)) tags.push('validation', 'guard');
  else if (/^(read|load)/.test(lower)) tags.push('deserialization', 'file-io');
  else if (/^(write|save)/.test(lower)) tags.push('serialization', 'file-io');
  else if (/^(scan|collect|list)/.test(lower)) tags.push('discovery', 'filesystem');
  else if (/^(copy|move|remove|delete|restore|prune|clear|quarantine)/.test(lower)) tags.push('filesystem', 'lifecycle');
  else if (/^(parse|split|normalize|sanitize|infer|classify)/.test(lower)) tags.push('parsing', 'utility');
  else if (/^(apply|import|export|extract|merge|pack|flatten)/.test(lower)) tags.push('transformation', 'workflow');
  else if (/(launch|process|running|wait|status)/.test(lower)) tags.push('process-management', 'runtime');
  else if (/(backup|bridge|snapshot)/.test(lower)) tags.push('backup', 'persistence');
  else if (/^(default|resolve|find|preferred)/.test(lower)) tags.push('resolution', 'utility');
  else if (/(key|id|hash|fingerprint)/.test(lower)) tags.push('identity', 'utility');
  else tags.push('domain-logic', 'service');
  if (filePath.includes('json_translation')) tags.push('json', 'translation');
  else if (filePath.includes('translation')) tags.push('translation');
  return uniqueTags([...tags, 'rust']);
}

function functionSummary(name, filePath) {
  const display = `\`${name}\``;
  const lower = name.toLowerCase();
  if (filePath.includes('/tests.rs') || lower.startsWith('test_')) return `${display} 시나리오에서 JSON 번역 동작과 결과를 검증하는 회귀 테스트입니다.`;
  if (/^(new|from_|create_)/.test(lower)) return `${display} 입력으로 작업에 필요한 초기 값이나 결과 구조를 구성합니다.`;
  if (/^(is_|has_|looks_|can_|should_)/.test(lower)) return `${display} 조건을 판정해 상위 워크플로의 분기 기준을 제공합니다.`;
  if (/^(ensure|validate|check)/.test(lower)) return `${display} 제약을 검증하고 유효하지 않거나 위험한 상태를 오류로 차단합니다.`;
  if (/^(read|load)/.test(lower)) return `${display} 데이터를 읽고 파싱해 상위 서비스가 사용할 구조로 복원합니다.`;
  if (/^(write|save)/.test(lower)) return `${display} 현재 상태를 직렬화해 지정된 저장소에 기록합니다.`;
  if (/^(scan|collect|list)/.test(lower)) return `${display} 대상 경로 또는 항목을 순회해 정렬된 분석 결과를 수집합니다.`;
  if (/^(copy|move|remove|delete|restore|prune|clear|quarantine)/.test(lower)) return `${display} 파일시스템 항목의 이동·복사·삭제 수명주기를 안전하게 처리합니다.`;
  if (/^(parse|split|normalize|sanitize|infer|classify)/.test(lower)) return `${display} 원시 입력을 해석하거나 정규화해 일관된 내부 표현으로 변환합니다.`;
  if (/^(apply|import|export|extract|merge|pack|flatten)/.test(lower)) return `${display} 입력 데이터를 변환해 번역·모드 관리 워크플로의 결과물을 생성합니다.`;
  if (/(launch|process|running|wait|status)/.test(lower)) return `${display} 게임 또는 보조 프로세스의 상태 확인과 실행 흐름을 담당합니다.`;
  if (/(backup|bridge|snapshot)/.test(lower)) return `${display} 세이브 데이터를 보존하거나 실행 모드 사이에서 안전하게 연결합니다.`;
  if (/^(default|resolve|find|preferred)/.test(lower)) return `${display} 설정과 파일시스템 단서를 바탕으로 가장 적합한 경로나 값을 결정합니다.`;
  if (/(key|id|hash|fingerprint)/.test(lower)) return `${display} 항목을 안정적으로 식별하거나 변경 여부를 비교할 값을 계산합니다.`;
  return `${display} 관련 도메인 규칙을 구현하고 상위 서비스가 사용할 결과를 반환합니다.`;
}

function classTags(name, filePath) {
  const tags = ['data-model', 'rust-type'];
  if (/Report|Entry|Record|Summary|Context/.test(name)) tags.push('domain');
  if (/Status|Kind|Source|Mode/.test(name)) tags.push('state-model');
  if (filePath.includes('json_translation')) tags.push('translation', 'serde');
  return uniqueTags([...tags, 'rust']);
}

function classSummary(name, filePath) {
  if (name === 'App') return '`App`은 설정을 소유하고 모드 스캔, 상태 전환, 프리셋, 번역 및 게임 실행 서비스를 조정하는 코어 파사드입니다.';
  const purpose = fileDescriptions[filePath]?.[0] ?? '해당 모듈의 도메인 로직';
  return `\`${name}\`는 ${purpose.replace(/입니다\.$/, '')}에 필요한 상태와 값을 표현하는 Rust 타입입니다.`;
}

function calleeBase(value) {
  return String(value ?? '').split('::').at(-1).split('.').at(-1).replace(/!$/, '');
}

const nodes = [];
const edges = [];
const nodeIds = new Set();
const edgeKeys = new Set();
const significantByFile = new Map();
const exportSets = new Map();

function addNode(node) {
  if (nodeIds.has(node.id)) throw new Error(`중복 노드 ID: ${node.id}`);
  nodeIds.add(node.id);
  nodes.push(node);
}

function addEdge(edge) {
  if (edge.source === edge.target) return;
  const key = `${edge.source}\u0000${edge.target}\u0000${edge.type}`;
  if (edgeKeys.has(key)) return;
  edgeKeys.add(key);
  edges.push(edge);
}

for (const result of extraction.results) {
  const file = fileInfo.get(result.path);
  if (!file) throw new Error(`batchFiles에 없는 추출 결과: ${result.path}`);
  const [summary, tags] = fileDescriptions[result.path] ?? [`${result.path}의 프로젝트 기능을 구현하는 Rust 소스 파일입니다.`, ['rust', 'source-code', 'domain-logic']];
  addNode({
    id: `file:${result.path}`,
    type: 'file',
    name: path.posix.basename(result.path),
    filePath: result.path,
    summary,
    tags,
    complexity: complexityForLines(result.nonEmptyLines),
    languageNotes: 'Rust의 Result 기반 오류 전파와 소유권을 활용해 파일시스템 중심 워크플로를 명시적으로 구성합니다.'
  });

  const exported = new Set((result.exports ?? []).map((entry) => entry?.name).filter(Boolean));
  exportSets.set(result.path, exported);
  const sig = { functions: new Map(), classes: new Map() };

  for (const fn of result.functions ?? []) {
    if (!fn?.name) continue;
    const lines = fn.endLine - fn.startLine + 1;
    if (lines < 10 && !exported.has(fn.name)) continue;
    if (sig.functions.has(fn.name)) throw new Error(`유의미 함수 이름 충돌: ${result.path}:${fn.name}`);
    const id = `function:${result.path}:${fn.name}`;
    const node = {
      id,
      type: 'function',
      name: fn.name,
      filePath: result.path,
      lineRange: [fn.startLine, fn.endLine],
      summary: functionSummary(fn.name, result.path),
      tags: functionTags(fn.name, result.path),
      complexity: complexityForLines(lines)
    };
    addNode(node);
    sig.functions.set(fn.name, node);
    addEdge({ source: `file:${result.path}`, target: id, type: 'contains', direction: 'forward', weight: 1.0 });
    if (exported.has(fn.name)) addEdge({ source: `file:${result.path}`, target: id, type: 'exports', direction: 'forward', weight: 0.8 });
  }

  for (const cls of result.classes ?? []) {
    if (!cls?.name) continue;
    const lines = cls.endLine - cls.startLine + 1;
    const methods = Array.isArray(cls.methods) ? cls.methods.length : 0;
    if (lines < 20 && methods < 2 && !exported.has(cls.name)) continue;
    if (sig.classes.has(cls.name)) throw new Error(`유의미 타입 이름 충돌: ${result.path}:${cls.name}`);
    const id = `class:${result.path}:${cls.name}`;
    const node = {
      id,
      type: 'class',
      name: cls.name,
      filePath: result.path,
      lineRange: [cls.startLine, cls.endLine],
      summary: classSummary(cls.name, result.path),
      tags: classTags(cls.name, result.path),
      complexity: methods > 8 || lines > 200 ? 'complex' : (methods >= 3 || lines >= 50 ? 'moderate' : 'simple')
    };
    addNode(node);
    sig.classes.set(cls.name, node);
    addEdge({ source: `file:${result.path}`, target: id, type: 'contains', direction: 'forward', weight: 1.0 });
    if (exported.has(cls.name)) addEdge({ source: `file:${result.path}`, target: id, type: 'exports', direction: 'forward', weight: 0.8 });
  }
  significantByFile.set(result.path, sig);
}

let importCount = 0;
for (const file of input.batchFiles) {
  const targets = importData[file.path] ?? [];
  for (const target of targets) {
    importCount += 1;
    addEdge({ source: `file:${file.path}`, target: `file:${target}`, type: 'imports', direction: 'forward', weight: 0.7 });
  }
}
if (importCount !== 76 || edges.filter((edge) => edge.type === 'imports').length !== 76) {
  throw new Error(`imports edge 1:1 검증 실패: expected=76 actual=${edges.filter((edge) => edge.type === 'imports').length}`);
}

for (const result of extraction.results) {
  const sourceSig = significantByFile.get(result.path);
  const imported = importData[result.path] ?? [];
  for (const call of result.callGraph ?? []) {
    const caller = sourceSig.functions.get(call?.caller);
    if (!caller) continue;
    const callee = calleeBase(call?.callee);
    if (!callee) continue;
    const matches = imported
      .map((targetPath) => significantByFile.get(targetPath)?.functions.get(callee))
      .filter(Boolean);
    if (matches.length === 1) {
      addEdge({ source: caller.id, target: matches[0].id, type: 'calls', direction: 'forward', weight: 0.8 });
    }
  }
}

const totalSignificantFunctions = [...significantByFile.values()].reduce((sum, value) => sum + value.functions.size, 0);
const totalSignificantClasses = [...significantByFile.values()].reduce((sum, value) => sum + value.classes.size, 0);
const expectedNodes = input.batchFiles.length + totalSignificantFunctions + totalSignificantClasses;
if (nodes.length !== expectedNodes) throw new Error(`노드 수 검증 실패: expected=${expectedNodes} actual=${nodes.length}`);

const parts = Math.ceil(Math.max(nodes.length / 60, edges.length / 120));
const sortedFiles = input.batchFiles.map((file) => file.path).sort();
const groupSize = Math.ceil(sortedFiles.length / parts);
const filePart = new Map();
for (let index = 0; index < parts; index += 1) {
  for (const filePath of sortedFiles.slice(index * groupSize, (index + 1) * groupSize)) {
    filePart.set(filePath, index + 1);
  }
}
const nodeFile = new Map(nodes.map((node) => [node.id, node.filePath]));
const finalEdges = edges.filter((edge) => {
  if (edge.type !== 'calls') return true;
  return filePart.get(nodeFile.get(edge.source)) === filePart.get(nodeFile.get(edge.target));
});
const finalPartCount = Math.ceil(Math.max(nodes.length / 60, finalEdges.length / 120));
if (finalPartCount !== parts) throw new Error(`호출 edge 분할 후 part 수가 변경되었습니다: ${parts} -> ${finalPartCount}`);
const partSummaries = [];

for (let index = 0; index < parts; index += 1) {
  const paths = sortedFiles.slice(index * groupSize, (index + 1) * groupSize);
  if (paths.length === 0) continue;
  const pathSet = new Set(paths);
  const partNodes = nodes.filter((node) => pathSet.has(node.filePath));
  const partNodeIds = new Set(partNodes.map((node) => node.id));
  const partEdges = finalEdges.filter((edge) => partNodeIds.has(edge.source));
  const output = { nodes: partNodes, edges: partEdges };
  const outputPath = path.join(outputDir, `batch-2-part-${index + 1}.json`);
  fs.writeFileSync(outputPath, `${JSON.stringify(output, null, 2)}\n`, 'utf8');
  JSON.parse(fs.readFileSync(outputPath, 'utf8'));
  partSummaries.push({ part: index + 1, files: paths, nodes: partNodes.length, edges: partEdges.length });
}

console.log(JSON.stringify({
  files: input.batchFiles.length,
  significantFunctions: totalSignificantFunctions,
  significantClasses: totalSignificantClasses,
  nodes: nodes.length,
  edges: finalEdges.length,
  importEdges: finalEdges.filter((edge) => edge.type === 'imports').length,
  containsEdges: finalEdges.filter((edge) => edge.type === 'contains').length,
  exportEdges: finalEdges.filter((edge) => edge.type === 'exports').length,
  callEdges: finalEdges.filter((edge) => edge.type === 'calls').length,
  parts: partSummaries
}, null, 2));
