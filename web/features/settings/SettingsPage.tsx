import React from "react";
import {
  AlertTriangle,
  Archive,
  CheckCircle2,
  FolderOpen,
  RefreshCw,
  Replace,
  ReplaceAll,
  Save,
  ShieldAlert,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { Segmented } from "../../components/AppShell";
import { Pill } from "../../components/Common";
import { labels } from "../../i18n";
import type {
  Dashboard,
  DeletedMod,
  GameLog,
  Locale,
  SaveBackup,
  SetupIssue,
  TroubleshootDiagnostic,
  UiSettings,
} from "../../types";
import { logTone } from "../../utils/logging";
import { formatBytes, formatFullDateTime, formatShortDate } from "../mods/modUtils";


function SettingsPage({
  labels: t,
  dashboard,
  draft,
  setupIssues,
  diagnostics,
  logs,
  gameLogs,
  gameLogsLoading,
  setDraft,
  onChooseTranslationWorkDir,
  onChooseGameExePath,
  onChooseGameLogPath,
  onChooseSaveDir,
  onChooseSaveBackupDir,
  onSave,
  onRefreshGameLogs,
  onOpenPath,
  onRepairInstallations,
  onClearCurrentRuns,
  onCleanupCaches,
  onRestoreDeleted,
  onEmptyDeleted,
  onCreateSaveBackup,
  onRestoreSaveBackup,
  onDeleteSaveBackups,
  busy,
  locale,
  setLocale,
}: {
  labels: typeof labels.ko;
  dashboard: Dashboard;
  draft: UiSettings;
  setupIssues: SetupIssue[];
  diagnostics: TroubleshootDiagnostic[];
  logs: string[];
  gameLogs: GameLog[];
  gameLogsLoading: boolean;
  setDraft: (value: UiSettings) => void;
  onChooseTranslationWorkDir: () => void;
  onChooseGameExePath: () => void;
  onChooseGameLogPath: () => void;
  onChooseSaveDir: () => void;
  onChooseSaveBackupDir: () => void;
  onSave: () => void;
  onRefreshGameLogs: () => void;
  onOpenPath: (path: string) => void;
  onRepairInstallations: () => void;
  onClearCurrentRuns: () => void;
  onCleanupCaches: () => void;
  onRestoreDeleted: (item: DeletedMod) => void;
  onEmptyDeleted: () => void;
  onCreateSaveBackup: () => void;
  onRestoreSaveBackup: (item: SaveBackup) => void;
  onDeleteSaveBackups: (items: SaveBackup[]) => void;
  busy: string | null;
  locale: Locale;
  setLocale: (value: Locale) => void;
}) {
  const settingsLocked = dashboard.launch.running;
  const inputDisabled = Boolean(busy) || settingsLocked;
  const hasIssue = (field: string) => setupIssues.some((issue) => issue.field === field && issue.blocking);
  const getIssueClass = (field: string) => hasIssue(field) ? " has-issue" : "";
  const pathRows = [
    ["Workspace", dashboard.paths.workspace],
    ["Game", dashboard.paths.game],
    ["Game Mods", dashboard.paths.game_mods],
    ["Disabled Mods", dashboard.paths.disabled],
    ["Save", dashboard.paths.save_dir],
    ["Save Backups", dashboard.paths.save_backup],
    ["Presets", dashboard.paths.presets],
    ["Translation", dashboard.paths.translation_work],
    ["Vendor", dashboard.paths.vendor],
  ];
  function confirmCleanupCaches() {
    const message = [
      "작업 캐시를 정리할까요?",
      "",
      `정리 대상: ${formatBytes(dashboard.cache_usage.bytes)} · 폴더 ${dashboard.cache_usage.dirs}개 · 파일 ${dashboard.cache_usage.files}개`,
      "",
      "삭제되는 것:",
      "- 모드/번역 미리보기 추출 캐시",
      "- 드래그한 압축 모드 임시 해제 파일",
      "- PCK/압축 빌드 임시 파일",
      "",
      "남기는 것:",
      "- 번역 시트와 번역 메모리",
      "- 세이브 백업",
      "- 최근 삭제 복구 항목",
      "",
      "계속할까요?",
    ].join("\n");
    if (window.confirm(message)) {
      onCleanupCaches();
    }
  }
  return (
    <div className="settings-grid">
      <section className="settings-section">
        <div className="settings-title-row">
          <h2>{t.settings}</h2>
          {settingsLocked && (
            <span className="lock-chip">
              <ShieldAlert size={14} />
              게임 실행 중
            </span>
          )}
        </div>
        {settingsLocked && (
          <div className="settings-lock-notice">
            게임이 실행 중이라 경로, 언어, 실행 파일 설정을 잠갔습니다. 게임을 종료하면 다시 변경할 수 있습니다.
          </div>
        )}
        <SetupIssuesPanel issues={setupIssues} />
        <label className="settings-field">
          <span>UI 언어</span>
          <Segmented value={locale} onChange={setLocale} disabled={inputDisabled} />
        </label>
        <label className="settings-field">
          <span>{t.extractionPath}</span>
          <div className={`path-picker${getIssueClass("translation_work_dir")}`}>
            <input
              value={draft.translation_work_dir}
              onChange={(event) => setDraft({ ...draft, translation_work_dir: event.target.value })}
              disabled={inputDisabled}
            />
            <button className="icon-button-text" onClick={onChooseTranslationWorkDir} disabled={inputDisabled}>
              <FolderOpen size={15} />
              {t.choose}
            </button>
          </div>
        </label>
        <label className="settings-field">
          <span>게임 실행 파일</span>
          <div className={`path-picker${getIssueClass("game_exe_path")}`}>
            <input
              value={draft.game_exe_path}
              onChange={(event) => setDraft({ ...draft, game_exe_path: event.target.value })}
              placeholder="자동 탐색 실패 시 Slay the Spire 2 .exe 선택"
              disabled={inputDisabled}
            />
            <button className="icon-button-text" onClick={onChooseGameExePath} disabled={inputDisabled}>
              <FolderOpen size={15} />
              {t.choose}
            </button>
          </div>
          <small>
            현재 실행 대상: {dashboard.launch.target_label}
            {dashboard.launch.running ? " · 실행 중" : ""}
          </small>
        </label>
        <label className="settings-field">
          <span>게임 로그 파일</span>
          <div className="path-picker">
            <input
              value={draft.game_log_path}
              onChange={(event) => setDraft({ ...draft, game_log_path: event.target.value })}
              placeholder="자동 탐색 실패 시 godot.log 선택"
              disabled={inputDisabled}
            />
            <button className="icon-button-text" onClick={onChooseGameLogPath} disabled={inputDisabled}>
              <FolderOpen size={15} />
              {t.choose}
            </button>
          </div>
          <small>기본 위치: {"C:\\Users\\angel\\AppData\\Roaming\\SlayTheSpire2\\logs\\godot.log"}</small>
        </label>
        <label className="settings-field">
          <span>세이브 폴더</span>
          <div className={`path-picker${getIssueClass("save_dir")}`}>
            <input
              value={draft.save_dir}
              onChange={(event) => setDraft({ ...draft, save_dir: event.target.value })}
              placeholder="자동 탐색: AppData/Roaming/SlayTheSpire2/steam/{Steam ID}"
              disabled={inputDisabled}
            />
            <button className="icon-button-text" onClick={onChooseSaveDir} disabled={inputDisabled}>
              <FolderOpen size={15} />
              {t.choose}
            </button>
          </div>
          <small>기본 위치: {"C:\\Users\\angel\\AppData\\Roaming\\SlayTheSpire2\\steam\\76561198093641030"}</small>
        </label>
        <label className="settings-field">
          <span>세이브 백업 경로</span>
          <div className={`path-picker${getIssueClass("save_backup_dir")}`}>
            <input
              value={draft.save_backup_dir}
              onChange={(event) => setDraft({ ...draft, save_backup_dir: event.target.value })}
              disabled={inputDisabled}
            />
            <button className="icon-button-text" onClick={onChooseSaveBackupDir} disabled={inputDisabled}>
              <FolderOpen size={15} />
              {t.choose}
            </button>
          </div>
        </label>
        <div className="settings-field two-column-field">
          <label>
            <span>세이브 보관기간</span>
            <select
              value={draft.save_backup_retention_days}
              onChange={(event) => setDraft({ ...draft, save_backup_retention_days: Number(event.target.value) })}
              disabled={inputDisabled}
            >
              <option value={3}>3일</option>
              <option value={7}>7일</option>
              <option value={14}>14일</option>
              <option value={30}>30일</option>
              <option value={0}>기간 제한 없음</option>
            </select>
          </label>
          <label>
            <span>최대 백업 수</span>
            <input className={getIssueClass("save_backup_max_entries")}
              type="number"
              min={1}
              max={200}
              value={draft.save_backup_max_entries}
              onChange={(event) => setDraft({ ...draft, save_backup_max_entries: Number(event.target.value) })}
              disabled={inputDisabled}
            />
          </label>
        </div>
        <label className="settings-field">
          <span>{t.targetLanguage}</span>
          <select
            value={draft.target_language}
            onChange={(event) => setDraft({ ...draft, target_language: event.target.value })}
            disabled={inputDisabled}
          >
            <option value="kor">한국어 (kor)</option>
            <option value="en">English</option>
            <option value="ja">日本語</option>
            <option value="zh-cn">简体中文</option>
            <option value="zh-tw">繁體中文</option>
          </select>
        </label>
        <label className="settings-field">
          <span>삭제 보관기간</span>
          <select
            value={draft.deleted_retention_days}
            onChange={(event) => setDraft({ ...draft, deleted_retention_days: Number(event.target.value) })}
            disabled={inputDisabled}
          >
            <option value={7}>7일</option>
            <option value={14}>14일</option>
            <option value={30}>30일</option>
            <option value={90}>90일</option>
            <option value={0}>자동 비우기 안 함</option>
          </select>
        </label>
        <div className="settings-action-bar">
          <button className="primary icon-button-text" onClick={onSave} disabled={inputDisabled}>
            <Save size={15} />
            {t.saveSettings}
          </button>
          <div className="cache-cleanup-row">
            <button className="icon-button-text cache-cleanup-button" type="button" onClick={confirmCleanupCaches} disabled={Boolean(busy)}>
              <Trash2 size={15} />
              작업 캐시 정리
            </button>
            <small>
              현재 사용량 {formatBytes(dashboard.cache_usage.bytes)} · 폴더 {dashboard.cache_usage.dirs}개 · 파일 {dashboard.cache_usage.files}개
            </small>
          </div>
        </div>
        <details className="settings-details" style={{ marginTop: "16px", padding: "8px 0" }}>
          <summary style={{ cursor: "pointer", fontWeight: "bold" }}>고급 경로 정보 표시</summary>
          <div className="details-content" style={{ marginTop: "8px" }}>
            {pathRows.map(([label, value]) => (
              <div className="path-row" key={label}>
                <span>{label}</span>
                <code>{value}</code>
              </div>
            ))}
          </div>
        </details>
        <DeletedModsPanel
          items={dashboard.deleted_mods}
          busy={busy}
          retentionDays={draft.deleted_retention_days}
          onRestore={onRestoreDeleted}
          onEmpty={onEmptyDeleted}
          onOpenPath={onOpenPath}
        />
        <SaveBackupsPanel
          items={dashboard.save_backups}
          busy={busy}
          retentionDays={draft.save_backup_retention_days}
          maxEntries={draft.save_backup_max_entries}
          onCreate={onCreateSaveBackup}
          onRestore={onRestoreSaveBackup}
          onDelete={onDeleteSaveBackups}
          onOpenPath={onOpenPath}
        />
      </section>
      <div className="settings-side-stack">
        <TroubleshootPanel
          diagnostics={diagnostics}
          busy={busy}
          onRepairInstallations={onRepairInstallations}
          onClearCurrentRuns={onClearCurrentRuns}
          onOpenPath={onOpenPath}
        />
        <details className="settings-section settings-details">
          <summary style={{ cursor: "pointer", outline: "none" }}><h2 style={{ display: "inline-block", margin: 0 }}>필수 내장 도구</h2></summary>
          <div style={{ marginTop: 12 }}>
            <p>{t.optional7z}</p>
            {dashboard.tools.map((tool) => (
              <div className="tool-row" key={tool.name}>
                <Pill tone={tool.available ? "good" : "warn"}>{tool.available ? "OK" : "Missing"}</Pill>
                <div>
                  <strong>{tool.name}</strong>
                  <small>{tool.expected_path}</small>
                </div>
              </div>
            ))}
          </div>
        </details>
        <details className="settings-section settings-log-section settings-details">
          <summary style={{ cursor: "pointer", outline: "none" }}><h2 style={{ display: "inline-block", margin: 0 }}>{t.logs}</h2></summary>
          <div style={{ marginTop: 12 }}>
            <LogsPanel labels={t} logs={logs} />
          </div>
        </details>
        <details className="settings-section settings-log-section settings-details">
          <summary style={{ cursor: "pointer", outline: "none" }}>
            <div className="settings-title-row" style={{ display: "inline-flex", width: "calc(100% - 24px)", alignItems: "center" }}>
              <h2 style={{ margin: 0 }}>게임 로그</h2>
              <button className="icon-button-text compact" type="button" onClick={onRefreshGameLogs} disabled={gameLogsLoading} style={{ marginLeft: "auto" }}>
                <RefreshCw size={14} />
                {gameLogsLoading ? "확인 중" : "새로고침"}
              </button>
            </div>
          </summary>
          <div style={{ marginTop: 12 }}>
            <GameLogsPanel logs={gameLogs} onOpenPath={onOpenPath} />
          </div>
        </details>
      </div>
    </div>
  );
}

function SetupIssuesPanel({ issues }: { issues: SetupIssue[] }) {
  if (issues.length === 0) {
    return (
      <div className="setup-check-panel good">
        <ShieldCheck size={16} />
        <span>실행에 필요한 기본 설정이 준비되었습니다.</span>
      </div>
    );
  }
  return (
    <div className="setup-check-panel warn">
      <AlertTriangle size={16} />
      <div>
        <strong>먼저 확인할 설정이 있습니다.</strong>
        {issues.map((issue) => (
          <span key={`${issue.field}-${issue.message}`}>{issue.blocking ? "필수" : "권장"} · {issue.message}</span>
        ))}
      </div>
    </div>
  );
}

function TroubleshootPanel({
  diagnostics,
  busy,
  onRepairInstallations,
  onClearCurrentRuns,
  onOpenPath,
}: {
  diagnostics: TroubleshootDiagnostic[];
  busy: string | null;
  onRepairInstallations: () => void;
  onClearCurrentRuns: () => void;
  onOpenPath: (path: string) => void;
}) {
  const fixable = diagnostics.some((item) => item.can_auto_fix);
  const saveFixable = diagnostics.some((item) => item.can_auto_fix && item.category === "safety");
  const fixing = busy === "clear_current_runs" || busy === "repair_mod_installations";
  const visible = diagnostics.slice(0, 10);
  const runAutoFix = () => {
    if (saveFixable) {
      onClearCurrentRuns();
      return;
    }
    onRepairInstallations();
  };
  const runDiagnosticFix = (item: TroubleshootDiagnostic) => {
    if (item.category === "safety") {
      onClearCurrentRuns();
      return;
    }
    onRepairInstallations();
  };
  return (
    <section className="settings-section troubleshoot-section">
      <div className="settings-title-row">
        <div>
          <h2>문제 해결</h2>
          <small>{diagnosticSummary(diagnostics)}</small>
        </div>
        <button className="icon-button-text compact" type="button" onClick={runAutoFix} disabled={Boolean(busy) || !fixable} aria-busy={fixing}>
          <ReplaceAll size={14} className={fixing ? "spin-icon" : undefined} />
          {fixing ? "정리 중" : "자동 정리"}
        </button>
      </div>
      {visible.length === 0 && <div className="empty compact">표시할 진단 항목이 없습니다.</div>}
      <div className="diagnostic-list">
        {visible.map((item) => (
          <article className={`diagnostic-card ${diagnosticTone(item.severity)}`} key={item.id}>
            <div className="diagnostic-icon">
              {item.severity === "good" ? <CheckCircle2 size={16} /> : item.severity === "error" ? <ShieldAlert size={16} /> : <AlertTriangle size={16} />}
            </div>
            <div>
              <strong>{item.title}</strong>
              <small>{diagnosticCategoryLabel(item.category)} · {item.detail}</small>
              {item.related_path && <code title={item.related_path}>{item.related_path}</code>}
            </div>
            <div className="diagnostic-actions">
              {item.can_auto_fix && (
                <button className="icon-button-text compact" type="button" aria-label={item.action_label} data-tooltip={fixing ? "정리 중" : item.action_label} onClick={() => runDiagnosticFix(item)} disabled={Boolean(busy)} aria-busy={fixing}>
                  <Replace size={14} className={fixing ? "spin-icon" : undefined} />
                  {fixing ? "정리 중" : "정리"}
                </button>
              )}
              {item.related_path && (
                <button className="icon-only-button" type="button" aria-label="경로 열기" data-tooltip="경로 열기" onClick={() => onOpenPath(item.related_path)} disabled={Boolean(busy)}>
                  <FolderOpen size={15} />
                </button>
              )}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function diagnosticSummary(items: TroubleshootDiagnostic[]) {
  const errors = items.filter((item) => item.severity === "error").length;
  const warnings = items.filter((item) => item.severity === "warn").length;
  if (errors || warnings) {
    return `오류 ${errors}개 · 경고 ${warnings}개`;
  }
  return "즉시 막히는 문제 없음";
}

function diagnosticTone(severity: string) {
  if (severity === "error") return "error";
  if (severity === "good") return "good";
  return "warn";
}

function diagnosticCategoryLabel(category: string) {
  switch (category) {
    case "install":
      return "설치";
    case "log":
      return "로그";
    case "update":
      return "업데이트";
    case "translation":
      return "번역";
    case "dependency":
      return "의존성";
    case "safety":
      return "세이브";
    default:
      return "설정";
  }
}

function DeletedModsPanel({
  items,
  busy,
  retentionDays,
  onRestore,
  onEmpty,
  onOpenPath,
}: {
  items: DeletedMod[];
  busy: string | null;
  retentionDays: number;
  onRestore: (item: DeletedMod) => void;
  onEmpty: () => void;
  onOpenPath: (path: string) => void;
}) {
  const totalBytes = items.reduce((total, item) => total + item.bytes, 0);
  const retentionLabel = retentionDays === 0 ? "자동 비우기 안 함" : `${retentionDays}일 보관`;
  return (
    <details className="deleted-mods-panel summary-panel">
      <summary>
        <div className="settings-title-row compact">
          <div>
            <h2>최근 삭제</h2>
            <small>{retentionLabel} · 삭제 {items.length}개 · 총 {formatBytes(totalBytes)}</small>
          </div>
          <button
            className="icon-button-text compact danger-text"
            type="button"
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onEmpty();
            }}
            disabled={Boolean(busy) || items.length === 0}
          >
            <Trash2 size={14} />
            비우기
          </button>
        </div>
      </summary>
      {items.length === 0 && <div className="empty compact">복원 가능한 삭제 항목이 없습니다.</div>}
      <div className="deleted-mod-list">
        {items.map((item) => (
          <article className="deleted-mod-card" key={item.id}>
            <div>
              <strong>{item.name}</strong>
              <small>{formatShortDate(item.deleted_epoch)} 삭제 · {formatBytes(item.bytes)}</small>
              <code title={item.original_path}>{item.original_path}</code>
              {item.expires_epoch && <span>보관 만료 {formatShortDate(item.expires_epoch)}</span>}
            </div>
            <div className="deleted-mod-actions">
              <button className="icon-only-button" type="button" aria-label="백업 위치 열기" data-tooltip="백업 위치 열기" onClick={() => onOpenPath(item.backup_path)} disabled={Boolean(busy)}>
                <FolderOpen size={15} />
              </button>
              <button className="icon-button-text compact" type="button" onClick={() => onRestore(item)} disabled={Boolean(busy)}>
                <RefreshCw size={14} />
                복원
              </button>
            </div>
          </article>
        ))}
      </div>
    </details>
  );
}

function SaveBackupsPanel({
  items,
  busy,
  retentionDays,
  maxEntries,
  onCreate,
  onRestore,
  onDelete,
  onOpenPath,
}: {
  items: SaveBackup[];
  busy: string | null;
  retentionDays: number;
  maxEntries: number;
  onCreate: () => void;
  onRestore: (item: SaveBackup) => void;
  onDelete: (items: SaveBackup[]) => void;
  onOpenPath: (path: string) => void;
}) {
  const groups = groupSaveBackups(items);
  const [selectedGroupIds, setSelectedGroupIds] = React.useState<Set<string>>(() => new Set());
  const totalBytes = items.reduce((total, item) => total + item.bytes, 0);
  const retentionLabel = retentionDays === 0 ? "기간 제한 없음" : `${retentionDays}일 보관`;
  const selectedGroups = groups.filter((group) => selectedGroupIds.has(group.id));
  const selectedItems = selectedGroups.flatMap((group) => group.items);
  const allSelected = groups.length > 0 && selectedGroupIds.size === groups.length;

  React.useEffect(() => {
    const visibleIds = new Set(groups.map((group) => group.id));
    setSelectedGroupIds((current) => {
      const next = new Set(Array.from(current).filter((id) => visibleIds.has(id)));
      return next.size === current.size ? current : next;
    });
  }, [groups]);

  function toggleSelectedGroup(id: string) {
    setSelectedGroupIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function toggleAllGroups() {
    setSelectedGroupIds((current) => {
      if (groups.length > 0 && current.size === groups.length) {
        return new Set();
      }
      return new Set(groups.map((group) => group.id));
    });
  }

  function deleteSelectedGroups(event: React.MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (selectedItems.length === 0) {
      return;
    }
    const confirmed = window.confirm(
      `선택한 세이브 백업 ${selectedGroups.length}세트, ${selectedItems.length}개를 삭제할까요?\n\n삭제한 백업은 복원할 수 없습니다.`,
    );
    if (!confirmed) {
      return;
    }
    onDelete(selectedItems);
    setSelectedGroupIds(new Set());
  }

  return (
    <details className="deleted-mods-panel save-backups-panel summary-panel">
      <summary>
        <div className="settings-title-row compact">
          <div>
            <h2>세이브 백업</h2>
            <small>{retentionLabel} · {groups.length}세트 · 백업 {items.length}개 · 총 {formatBytes(totalBytes)} · 종류별 최대 {maxEntries}개</small>
          </div>
          <div className="save-backup-summary-actions">
            <button
              className="icon-button-text compact"
              type="button"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                toggleAllGroups();
              }}
              disabled={Boolean(busy) || groups.length === 0}
            >
              <CheckCircle2 size={14} />
              {allSelected ? "선택 해제" : "전체 선택"}
            </button>
            <button
              className="icon-button-text compact danger-text"
              type="button"
              onClick={deleteSelectedGroups}
              disabled={Boolean(busy) || selectedItems.length === 0}
            >
              <Trash2 size={14} />
              선택 삭제
            </button>
            <button
              className="icon-button-text compact"
              type="button"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                onCreate();
              }}
              disabled={Boolean(busy)}
            >
              <Archive size={14} />
              지금 백업
            </button>
          </div>
        </div>
      </summary>
      {groups.length === 0 && <div className="empty compact">아직 복원 가능한 세이브 백업이 없습니다.</div>}
      <div className="save-backup-set-list">
        {groups.map((group) => (
          <details className="save-backup-set" key={group.id}>
            <summary>
              <label
                className="save-backup-select"
                onClick={(event) => event.stopPropagation()}
              >
                <input
                  type="checkbox"
                  checked={selectedGroupIds.has(group.id)}
                  onChange={() => toggleSelectedGroup(group.id)}
                  disabled={Boolean(busy)}
                  aria-label={`${formatFullDateTime(group.createdEpoch)} 백업 선택`}
                />
              </label>
              <div>
                <strong>{formatFullDateTime(group.createdEpoch)}</strong>
                <small>{backupSetLabel(group)} · {formatBytes(group.bytes)}</small>
              </div>
              <button
                className="icon-only-button danger-text"
                type="button"
                aria-label="이 백업 세트 삭제"
                data-tooltip="이 백업 세트 삭제"
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  if (window.confirm("이 세이브 백업 세트를 삭제할까요?\n\n삭제한 백업은 복원할 수 없습니다.")) {
                    onDelete(group.items);
                  }
                }}
                disabled={Boolean(busy)}
              >
                <Trash2 size={15} />
              </button>
            </summary>
            <div className="save-backup-pair">
              <SaveBackupSlot item={group.vanilla} kindLabel="바닐라" busy={busy} onRestore={onRestore} onOpenPath={onOpenPath} />
              <SaveBackupSlot item={group.modded} kindLabel="모드" busy={busy} onRestore={onRestore} onOpenPath={onOpenPath} />
            </div>
          </details>
        ))}
      </div>
    </details>
  );
}

function SaveBackupSlot({
  item,
  kindLabel,
  busy,
  onRestore,
  onOpenPath,
}: {
  item?: SaveBackup;
  kindLabel: string;
  busy: string | null;
  onRestore: (item: SaveBackup) => void;
  onOpenPath: (path: string) => void;
}) {
  if (!item) {
    return (
      <div className="save-backup-slot missing">
        <strong>{kindLabel}</strong>
        <span>같은 세트에 백업이 없습니다.</span>
      </div>
    );
  }
  return (
    <article className="save-backup-slot">
      <div>
        <strong>{kindLabel}</strong>
        <small>{formatShortDate(item.created_epoch)} · {formatBytes(item.bytes)}</small>
        <span title={item.path}>{compactBackupPath(item.path)}</span>
      </div>
      <div className="deleted-mod-actions">
        <button className="icon-only-button" type="button" aria-label="백업 위치 열기" data-tooltip="백업 위치 열기" onClick={() => onOpenPath(item.path)} disabled={Boolean(busy)}>
          <FolderOpen size={15} />
        </button>
        <button className="icon-button-text compact" type="button" onClick={() => onRestore(item)} disabled={Boolean(busy)}>
          <RefreshCw size={14} />
          복원
        </button>
      </div>
    </article>
  );
}

type SaveBackupGroup = {
  id: string;
  createdEpoch: number;
  bytes: number;
  items: SaveBackup[];
  vanilla?: SaveBackup;
  modded?: SaveBackup;
};

function groupSaveBackups(items: SaveBackup[]): SaveBackupGroup[] {
  const sorted = [...items].sort((left, right) => right.created_epoch - left.created_epoch);
  const used = new Set<string>();
  const groups: SaveBackupGroup[] = [];
  for (const item of sorted) {
    if (used.has(item.id)) continue;
    used.add(item.id);
    const oppositeKind = item.kind === "vanilla" ? "modded" : "vanilla";
    const pair = sorted
      .filter((candidate) => !used.has(candidate.id) && candidate.kind === oppositeKind)
      .map((candidate) => ({ candidate, gap: Math.abs(candidate.created_epoch - item.created_epoch) }))
      .filter(({ gap }) => gap <= 10)
      .sort((left, right) => left.gap - right.gap)[0]?.candidate;
    if (pair) used.add(pair.id);
    const groupItems = pair ? [item, pair] : [item];
    const vanilla = groupItems.find((entry) => entry.kind === "vanilla");
    const modded = groupItems.find((entry) => entry.kind === "modded");
    const createdEpoch = Math.max(...groupItems.map((entry) => entry.created_epoch));
    groups.push({
      id: groupItems.map((entry) => entry.id).sort().join("|"),
      createdEpoch,
      bytes: groupItems.reduce((total, entry) => total + entry.bytes, 0),
      items: groupItems,
      vanilla,
      modded,
    });
  }
  return groups.sort((left, right) => right.createdEpoch - left.createdEpoch);
}

function backupSetLabel(group: SaveBackupGroup) {
  if (group.vanilla && group.modded) return "바닐라 + 모드";
  if (group.vanilla) return "바닐라만";
  return "모드만";
}

function compactBackupPath(path: string) {
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length < 3) return path;
  return `${parts[parts.length - 3]}/${parts[parts.length - 2]}/${parts[parts.length - 1]}`;
}

function LogsPage({ labels: t, logs }: { labels: typeof labels.ko; logs: string[] }) {
  return <LogsPanel labels={t} logs={logs} />;
}

function LogsPanel({ labels: t, logs }: { labels: typeof labels.ko; logs: string[] }) {
  return (
    <div className="logs">
      {logs.length === 0 && <div className="empty">{t.noLogs}</div>}
      {logs.map((log) => <p className={`log-line ${logTone(log)}`} key={log}>{log}</p>)}
    </div>
  );
}

function GameLogsPanel({ logs, onOpenPath }: { logs: GameLog[]; onOpenPath: (path: string) => void }) {
  const existingLogs = logs.filter((log) => log.exists);
  const missingLogs = logs.filter((log) => !log.exists).slice(0, 8);
  return (
    <div className="game-log-list">
      {existingLogs.length === 0 && (
        <div className="game-log-empty">
          <strong>아직 발견된 로그 파일이 없습니다.</strong>
          <span>자동 후보를 확인했습니다. 못 찾으면 왼쪽의 게임 로그 파일에서 godot.log를 직접 선택해 주세요.</span>
        </div>
      )}
      {existingLogs.map((log) => (
        <article className="game-log-card" key={log.path}>
          <div className="game-log-head">
            <div>
              <strong>{log.path}</strong>
              <small>{formatFullDateTime(log.modified_epoch)} · {formatBytes(log.bytes)}</small>
            </div>
            <button className="icon-only-button" type="button" aria-label="로그 위치 열기" data-tooltip="로그 위치 열기" onClick={() => onOpenPath(log.path)}>
              <FolderOpen size={15} />
            </button>
          </div>
          <div className="logs game-log-tail">
            {log.lines.length === 0 && <p>로그 내용이 비어 있습니다.</p>}
            {log.lines.map((line, index) => <p className={`log-line ${logTone(line)}`} key={`${log.path}-${index}`}>{line}</p>)}
          </div>
        </article>
      ))}
      {missingLogs.length > 0 && (
        <details className="missing-log-paths">
          <summary>확인한 후보 경로 {logs.filter((log) => !log.exists).length}개</summary>
          {missingLogs.map((log) => <code key={log.path}>{log.path}</code>)}
        </details>
      )}
    </div>
  );
}

export { SettingsPage, LogsPanel, GameLogsPanel };
