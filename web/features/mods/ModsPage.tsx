import React from "react";
import {
  Gamepad2,
  Languages,
  List,
  Play,
  Search,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import { labels } from "../../i18n";
import type {
  ActiveFilter,
  ChangeFilter,
  DashboardStatFilter,
  ModGroup,
  ModRow,
  ModSort,
  Preset,
  TranslationApplyFilter,
} from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import {
  readStoredModTableColumns,
  readStoredModViewMode,
  writeStoredModTableColumns,
  writeStoredModViewMode,
} from "../../utils/storage";
import { ExtractConfirmModal } from "./ExtractModal";
import { ImportMenu } from "./ImportMenu";
import { ModGroupTableRow, ModTableRow, SimpleModGroupRow, SimpleModRow } from "./ModRows";
import { PresetMenu } from "./PresetMenu";
import {
  buildModGroups,
  languageLabel,
  preferredGroupActivationTarget,
} from "./modUtils";

type DetailModColumnKey = "name" | "version" | "date" | "translation" | "language" | "actions";
type SimpleModColumnKey = "name" | "version" | "language" | "actions";
type DetailModColumns = Record<DetailModColumnKey, number>;
type SimpleModColumns = Record<SimpleModColumnKey, number>;

const defaultDetailModColumns: DetailModColumns = {
  name: 420,
  version: 132,
  date: 118,
  translation: 96,
  language: 176,
  actions: 220,
};

const defaultSimpleModColumns: SimpleModColumns = {
  name: 420,
  version: 104,
  language: 104,
  actions: 220,
};

const minDetailModColumns: DetailModColumns = {
  name: 260,
  version: 88,
  date: 92,
  translation: 82,
  language: 120,
  actions: 212,
};

const minSimpleModColumns: SimpleModColumns = {
  name: 260,
  version: 78,
  language: 84,
  actions: 176,
};

function ModsPage(props: {
  labels: typeof labels.ko;
  mods: ModRow[];
  presets: Preset[];
  search: string;
  statFilter: DashboardStatFilter;
  activeFilter: ActiveFilter;
  changeFilter: ChangeFilter;
  translationApplyFilter: TranslationApplyFilter;
  sort: ModSort;
  selectedPreset: string;
  presetName: string;
  archivePath: string;
  targetLanguage: string;
  vanillaSafe: boolean;
  launchRunning: boolean;
  busy: string | null;
  togglingModKeys: Set<string>;
  initialSimpleView: boolean;
  focusedModKey: string | null;
  setSearch: (value: string) => void;
  setStatFilter: (value: DashboardStatFilter) => void;
  setActiveFilter: (value: ActiveFilter) => void;
  setChangeFilter: (value: ChangeFilter) => void;
  setTranslationApplyFilter: (value: TranslationApplyFilter) => void;
  setSort: (value: ModSort) => void;
  setSelectedPreset: (value: string) => void;
  setPresetName: (value: string) => void;
  setArchivePath: (value: string) => void;
  onToggle: (mod: ModRow) => Promise<void> | void;
  onOpenPath: (path: string) => void;
  onDelete: (mod: ModRow) => void;
  onExtract: (mod: ModRow) => void;
  onStartModTranslation: (mod: ModRow, resourcePath?: string) => void;
  onSavePreset: () => void;
  onApplyPreset: () => void;
  onExportPreset: () => void;
  onImportPreset: () => void;
  onImportFolder: () => void;
  onImportArchive: () => void;
  onImportVortexDownloads: () => void;
  onLaunch: () => void;
  onVanilla: () => void;
  onRefreshDashboard: () => void;
}) {
  const t = props.labels;
  const [expandedGroups, setExpandedGroups] = React.useState<Record<string, boolean>>({});
  const [forceModChanges, setForceModChanges] = React.useState(false);
  const [simpleView, setSimpleView] = React.useState(() => props.initialSimpleView || readStoredModViewMode());
  const [detailColumns, setDetailColumns] = React.useState(() =>
    clampModColumns(readStoredModTableColumns("detail", defaultDetailModColumns), minDetailModColumns),
  );
  const [simpleColumns, setSimpleColumns] = React.useState(() =>
    clampModColumns(readStoredModTableColumns("simple", defaultSimpleModColumns), minSimpleModColumns),
  );
  const groups = React.useMemo(
    () => buildModGroups(props.mods, props.search, props.statFilter, props.activeFilter, props.changeFilter, props.translationApplyFilter, props.sort),
    [props.mods, props.search, props.statFilter, props.activeFilter, props.changeFilter, props.translationApplyFilter, props.sort],
  );
  const showChangeDetails = props.statFilter === "changed";
  const vortexDownloadCount = React.useMemo(
    () => props.mods.filter((mod) => isVortexDownloadMod(mod) && mod.download_state !== "downloading").length,
    [props.mods],
  );

  function toggleGroup(groupId: string) {
    setExpandedGroups((current) => ({ ...current, [groupId]: !current[groupId] }));
  }

  React.useEffect(() => {
    if (!props.launchRunning) {
      setForceModChanges(false);
    }
  }, [props.launchRunning]);

  React.useEffect(() => {
    writeStoredModTableColumns("detail", detailColumns);
  }, [detailColumns]);

  React.useEffect(() => {
    writeStoredModTableColumns("simple", simpleColumns);
  }, [simpleColumns]);

  const modChangesLocked = props.launchRunning && !forceModChanges;

  React.useEffect(() => {
    if (!props.focusedModKey) {
      return;
    }
    const group = groups.find((item) => item.mods.some((mod) => mod.key === props.focusedModKey));
    if (!group) {
      return;
    }
    if (group.mods.length > 1) {
      setExpandedGroups((current) => ({ ...current, [group.id]: true }));
    }
    const escapedKey = props.focusedModKey.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    const timeoutId = window.setTimeout(() => {
      const row = document.querySelector<HTMLElement>(`[data-mod-key="${escapedKey}"]`);
      row?.scrollIntoView({ behavior: "smooth", block: "center" });
      row?.focus({ preventScroll: true });
    }, 120);
    return () => window.clearTimeout(timeoutId);
  }, [groups, props.focusedModKey]);

  function setStoredSimpleView(value: boolean) {
    setSimpleView(value);
    writeStoredModViewMode(value);
    if (!isPreviewRuntime()) {
      void invokeCommand("save_mod_view_mode", { modViewMode: value ? "simple" : "detail" });
    }
  }

  function startDetailColumnResize(column: DetailModColumnKey, event: React.MouseEvent) {
    startColumnResize(event, detailColumns[column], minDetailModColumns[column], (width) => {
      setDetailColumns((current) => ({ ...current, [column]: width }));
    });
  }

  function startSimpleColumnResize(column: SimpleModColumnKey, event: React.MouseEvent) {
    startColumnResize(event, simpleColumns[column], minSimpleModColumns[column], (width) => {
      setSimpleColumns((current) => ({ ...current, [column]: width }));
    });
  }

  const tableStyle = React.useMemo(
    () => ({
      "--mod-name-col": `${detailColumns.name}px`,
      "--mod-version-col": `${detailColumns.version}px`,
      "--mod-date-col": `${detailColumns.date}px`,
      "--mod-translation-col": `${detailColumns.translation}px`,
      "--mod-language-col": `${detailColumns.language}px`,
      "--mod-actions-col": `${detailColumns.actions}px`,
      "--mod-simple-name-col": `${simpleColumns.name}px`,
      "--mod-simple-version-col": `${simpleColumns.version}px`,
      "--mod-simple-language-col": `${simpleColumns.language}px`,
      "--mod-simple-actions-col": `${simpleColumns.actions}px`,
    }) as React.CSSProperties,
    [detailColumns, simpleColumns],
  );

  async function toggleGroupEnabled(group: ModGroup) {
    const activeMods = group.mods.filter((mod) => mod.active);
    if (activeMods.length > 0) {
      for (const mod of activeMods) {
        await props.onToggle(mod);
      }
      return;
    }
    const target = preferredGroupActivationTarget(group.mods);
    if (target) {
      await props.onToggle(target);
    }
  }

  return (
    <>
      <div className={modChangesLocked ? "mod-lock-frame locked" : "mod-lock-frame"}>
      <div className="toolbar mod-toolbar">
        <div className="search-field">
          <Search size={16} />
          <input value={props.search} onChange={(event) => props.setSearch(event.target.value)} placeholder={t.search} />
        </div>
        <select className="filter-select" value={props.activeFilter} onChange={(event) => props.setActiveFilter(event.target.value as ActiveFilter)} title="활성 상태">
          <option value="all">{t.all}</option>
          <option value="enabled">{t.enabled}</option>
          <option value="disabled">{t.disabledToggle}</option>
        </select>
        <select className="filter-select wide" value={props.changeFilter} onChange={(event) => props.setChangeFilter(event.target.value as ChangeFilter)} title="변경 상태">
          <option value="all">변경 전체</option>
          <option value="changed">{t.updates}</option>
          <option value="new">신규</option>
          <option value="updated">업데이트됨</option>
          <option value="clean">변경 없음</option>
        </select>
        <select
          className="filter-select wide"
          value={props.translationApplyFilter}
          onChange={(event) => props.setTranslationApplyFilter(event.target.value as TranslationApplyFilter)}
          title="번역 적용"
        >
          <option value="all">번역 전체</option>
          <option value="applied">적용됨</option>
          <option value="notApplied">미적용</option>
        </select>
        <select className="filter-select sort-select" value={props.sort} onChange={(event) => props.setSort(event.target.value as ModSort)} title="정렬">
          <option value="name">이름순</option>
          <option value="registered">등록일 최신순</option>
          <option value="updated">업데이트 최신순</option>
          <option value="modified">파일 수정 최신순</option>
          <option value="translationApplied">번역 적용 최신순</option>
          <option value="active">활성 우선</option>
          <option value="change">변경 우선</option>
          <option value="source">출처순</option>
          <option value="version">버전순</option>
        </select>
        <span
          className="translation-target-chip"
          aria-label={`번역 대상 ${languageLabel(props.targetLanguage)} (${props.targetLanguage})`}
          data-tooltip={`번역 대상 ${languageLabel(props.targetLanguage)} (${props.targetLanguage})`}
        >
          <Languages size={15} />
          <span>{props.targetLanguage}</span>
        </span>
        <ImportMenu
          busy={props.busy}
          vortexDownloadCount={vortexDownloadCount}
          onImportFolder={props.onImportFolder}
          onImportArchive={props.onImportArchive}
          onImportVortexDownloads={props.onImportVortexDownloads}
        />
        <PresetMenu
          labels={t}
          presets={props.presets}
          selectedPreset={props.selectedPreset}
          presetName={props.presetName}
          archivePath={props.archivePath}
          setSelectedPreset={props.setSelectedPreset}
          setPresetName={props.setPresetName}
          setArchivePath={props.setArchivePath}
          onSave={props.onSavePreset}
          onApply={props.onApplyPreset}
          onExport={props.onExportPreset}
          onImport={props.onImportPreset}
          busy={props.busy}
        />
        <span
          className={`launch-status-chip ${props.vanillaSafe ? "good" : "warn"}`}
          aria-label={props.vanillaSafe ? t.safe : t.unsafe}
          data-tooltip={props.vanillaSafe ? t.safe : t.unsafe}
        >
          {props.vanillaSafe ? <ShieldCheck size={15} /> : <ShieldAlert size={15} />}
        </span>
        <button
          className={simpleView ? "toolbar-icon-button active" : "toolbar-icon-button"}
          type="button"
          aria-label={simpleView ? "상세 보기" : "심플 보기"}
          data-tooltip={simpleView ? "상세 보기" : "심플 보기"}
          aria-pressed={simpleView}
          onClick={() => setStoredSimpleView(!simpleView)}
        >
          <List size={16} />
        </button>
        <button className="toolbar-icon-button" aria-label={t.vanilla} data-tooltip={t.vanilla} onClick={props.onVanilla} disabled={Boolean(props.busy) || props.launchRunning}>
          <Gamepad2 size={16} />
        </button>
        <button className="toolbar-icon-button primary" aria-label={t.launch} data-tooltip={t.launch} onClick={props.onLaunch} disabled={Boolean(props.busy) || props.launchRunning}>
          <Play size={16} fill="currentColor" />
        </button>
      </div>
      <div className={simpleView ? "table mod-table simple" : "table mod-table detail"} style={tableStyle}>
        {simpleView ? (
          <div className="table-head mod-simple-grid">
            <ModResizableHead label="모드 이름" />
            <ModResizableHead label="버전" handleEdge="start" onMouseDown={(event) => startSimpleColumnResize("version", event)} />
            <ModResizableHead label="언어" handleEdge="start" onMouseDown={(event) => startSimpleColumnResize("language", event)} />
            <ModResizableHead className="table-action-head" label="액션" handleEdge="start" onMouseDown={(event) => startSimpleColumnResize("actions", event)} />
          </div>
        ) : (
          <div className="table-head mod-grid">
            <ModResizableHead label={t.mods} onMouseDown={(event) => startDetailColumnResize("name", event)} />
            <ModResizableHead label="버전 & 출처" onMouseDown={(event) => startDetailColumnResize("version", event)} />
            <ModResizableHead label="날짜" onMouseDown={(event) => startDetailColumnResize("date", event)} />
            <ModResizableHead label="번역" onMouseDown={(event) => startDetailColumnResize("translation", event)} />
            <ModResizableHead label={t.languagePreview} onMouseDown={(event) => startDetailColumnResize("language", event)} />
            <ModResizableHead className="table-action-head" label="액션" onMouseDown={(event) => startDetailColumnResize("actions", event)} />
          </div>
        )}
        {groups.map((group) => {
          const isGrouped = group.mods.length > 1;
          const attachedTranslationGroup = attachedTranslationMods(group);
          const isExpanded = expandedGroups[group.id] ?? (attachedTranslationGroup ? true : props.activeFilter !== "all" || props.changeFilter !== "all" || props.translationApplyFilter !== "all");
          if (attachedTranslationGroup) {
            const { parent, children } = attachedTranslationGroup;
            return (
              <React.Fragment key={group.id}>
                {simpleView ? (
                  <SimpleModRow
                    labels={t}
                    mod={parent}
                    targetLanguage={props.targetLanguage}
                    busy={props.busy}
                    toggling={props.togglingModKeys.has(parent.key)}
                    locked={modChangesLocked}
                    onToggle={props.onToggle}
                    onOpenPath={props.onOpenPath}
                    onDelete={props.onDelete}
                    onExtract={props.onExtract}
                    onStartModTranslation={props.onStartModTranslation}
                    attachedChildCount={children.length}
                    expanded={isExpanded}
                    onToggleExpand={() => toggleGroup(group.id)}
                    focused={props.focusedModKey === parent.key}
                    showChangeDetails={showChangeDetails}
                  />
                ) : (
                  <ModTableRow
                    labels={t}
                    mod={parent}
                    targetLanguage={props.targetLanguage}
                    busy={props.busy}
                    toggling={props.togglingModKeys.has(parent.key)}
                    locked={modChangesLocked}
                    onToggle={props.onToggle}
                    onOpenPath={props.onOpenPath}
                    onDelete={props.onDelete}
                    onExtract={props.onExtract}
                    onStartModTranslation={props.onStartModTranslation}
                    attachedChildCount={children.length}
                    expanded={isExpanded}
                    onToggleExpand={() => toggleGroup(group.id)}
                    focused={props.focusedModKey === parent.key}
                    showChangeDetails={showChangeDetails}
                  />
                )}
                {isExpanded &&
                  children.map((mod) => simpleView ? (
                    <SimpleModRow
                      key={mod.key}
                      labels={t}
                      mod={mod}
                      targetLanguage={props.targetLanguage}
                      busy={props.busy}
                      toggling={props.togglingModKeys.has(mod.key)}
                      locked={modChangesLocked}
                      onToggle={props.onToggle}
                      onOpenPath={props.onOpenPath}
                      onDelete={props.onDelete}
                      onExtract={props.onExtract}
                      onStartModTranslation={props.onStartModTranslation}
                      child
                      focused={props.focusedModKey === mod.key}
                      showChangeDetails={showChangeDetails}
                    />
                  ) : (
                    <ModTableRow
                      key={mod.key}
                      labels={t}
                      mod={mod}
                      targetLanguage={props.targetLanguage}
                      busy={props.busy}
                      toggling={props.togglingModKeys.has(mod.key)}
                      locked={modChangesLocked}
                      onToggle={props.onToggle}
                      onOpenPath={props.onOpenPath}
                      onDelete={props.onDelete}
                      onExtract={props.onExtract}
                      onStartModTranslation={props.onStartModTranslation}
                      child
                      focused={props.focusedModKey === mod.key}
                      showChangeDetails={showChangeDetails}
                    />
                  ))}
              </React.Fragment>
            );
          }
          if (!isGrouped) {
            return simpleView ? (
              <SimpleModRow
                key={group.mods[0].key}
                labels={t}
                mod={group.mods[0]}
                targetLanguage={props.targetLanguage}
                busy={props.busy}
                toggling={props.togglingModKeys.has(group.mods[0].key)}
                locked={modChangesLocked}
                onToggle={props.onToggle}
                onOpenPath={props.onOpenPath}
                onDelete={props.onDelete}
                onExtract={props.onExtract}
                onStartModTranslation={props.onStartModTranslation}
                focused={props.focusedModKey === group.mods[0].key}
                showChangeDetails={showChangeDetails}
              />
            ) : (
              <ModTableRow
                key={group.mods[0].key}
                labels={t}
                mod={group.mods[0]}
                targetLanguage={props.targetLanguage}
                busy={props.busy}
                toggling={props.togglingModKeys.has(group.mods[0].key)}
                locked={modChangesLocked}
                onToggle={props.onToggle}
                onOpenPath={props.onOpenPath}
                onDelete={props.onDelete}
                onExtract={props.onExtract}
                onStartModTranslation={props.onStartModTranslation}
                focused={props.focusedModKey === group.mods[0].key}
                showChangeDetails={showChangeDetails}
              />
            );
          }
          return (
            <React.Fragment key={group.id}>
              {simpleView ? (
                <SimpleModGroupRow
                  group={group}
                  targetLanguage={props.targetLanguage}
                  busy={props.busy}
                  toggling={group.mods.some((mod) => props.togglingModKeys.has(mod.key))}
                  locked={modChangesLocked}
                  expanded={isExpanded}
                  onToggleGroup={() => void toggleGroupEnabled(group)}
                  onToggleExpand={() => toggleGroup(group.id)}
                  showChangeDetails={showChangeDetails}
                />
              ) : (
                <ModGroupTableRow
                  group={group}
                  busy={props.busy}
                  toggling={group.mods.some((mod) => props.togglingModKeys.has(mod.key))}
                  locked={modChangesLocked}
                  expanded={isExpanded}
                  onToggleGroup={() => void toggleGroupEnabled(group)}
                  onToggleExpand={() => toggleGroup(group.id)}
                  showChangeDetails={showChangeDetails}
                />
              )}
              {isExpanded &&
                group.mods.map((mod) => simpleView ? (
                  <SimpleModRow
                    key={mod.key}
                    labels={t}
                    mod={mod}
                    targetLanguage={props.targetLanguage}
                    busy={props.busy}
                    toggling={props.togglingModKeys.has(mod.key)}
                    locked={modChangesLocked}
                    onToggle={props.onToggle}
                    onOpenPath={props.onOpenPath}
                    onDelete={props.onDelete}
                    onExtract={props.onExtract}
                    onStartModTranslation={props.onStartModTranslation}
                    child
                    focused={props.focusedModKey === mod.key}
                    showChangeDetails={showChangeDetails}
                  />
                ) : (
                  <ModTableRow
                    key={mod.key}
                    labels={t}
                    mod={mod}
                    targetLanguage={props.targetLanguage}
                    busy={props.busy}
                    toggling={props.togglingModKeys.has(mod.key)}
                    locked={modChangesLocked}
                    onToggle={props.onToggle}
                    onOpenPath={props.onOpenPath}
                    onDelete={props.onDelete}
                    onExtract={props.onExtract}
                    onStartModTranslation={props.onStartModTranslation}
                    child
                    focused={props.focusedModKey === mod.key}
                    showChangeDetails={showChangeDetails}
                  />
                ))}
            </React.Fragment>
          );
        })}
        {groups.length === 0 && <div className="empty compact">조건에 맞는 모드가 없습니다.</div>}
      </div>
      {modChangesLocked && (
        <div className="mod-lock-overlay">
          <div>
            <ShieldAlert size={22} />
            <strong>게임이 실행 중입니다.</strong>
            <span>실행 중에 모드를 변경하면 게임 데이터가 꼬이거나 적용 상태가 달라질 수 있습니다.</span>
            <button
              type="button"
              className="primary icon-button-text"
              onClick={() => {
                setForceModChanges(true);
                props.onRefreshDashboard();
              }}
            >
              그래도 변경하기
            </button>
          </div>
        </div>
      )}
      </div>
    </>
  );
}

function ModResizableHead({
  className = "",
  handleEdge = "end",
  label,
  onMouseDown,
}: {
  className?: string;
  handleEdge?: "start" | "end";
  label: string;
  onMouseDown?: (event: React.MouseEvent) => void;
}) {
  return (
    <span className={["mod-resizable-head", className].filter(Boolean).join(" ")}>
      {label}
      {onMouseDown && (
        <button
          className={`resize-handle ${handleEdge}`}
          type="button"
          aria-label={`${label} 컬럼 너비 조절`}
          onMouseDown={onMouseDown}
        />
      )}
    </span>
  );
}

function startColumnResize(
  event: React.MouseEvent,
  startWidth: number,
  minWidth: number,
  setWidth: (width: number) => void,
) {
  event.preventDefault();
  event.stopPropagation();
  const startX = event.clientX;
  const previousCursor = document.body.style.cursor;
  const previousUserSelect = document.body.style.userSelect;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  const onMove = (moveEvent: MouseEvent) => {
    setWidth(Math.max(minWidth, Math.min(920, startWidth + moveEvent.clientX - startX)));
  };
  const onUp = () => {
    document.body.style.cursor = previousCursor;
    document.body.style.userSelect = previousUserSelect;
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", onUp);
  };
  document.addEventListener("mousemove", onMove);
  document.addEventListener("mouseup", onUp);
}

function clampModColumns<T extends Record<string, number>>(columns: T, minimums: T): T {
  const next = { ...columns };
  for (const key of Object.keys(minimums) as Array<keyof T>) {
    next[key] = Math.max(minimums[key], columns[key]) as T[keyof T];
  }
  return next;
}

export {
  ModsPage,
  ExtractConfirmModal,
};

function isVortexDownloadMod(mod: ModRow): boolean {
  return mod.external && mod.path.replace(/\\/g, "/").toLowerCase().includes("/vortex/downloads/");
}

function attachedTranslationMods(group: ModGroup): { parent: ModRow; children: ModRow[] } | null {
  const originals = group.mods.filter((mod) => !mod.is_translation_patch);
  const translations = group.mods.filter((mod) => mod.is_translation_patch);
  if (originals.length !== 1 || translations.length === 0) {
    return null;
  }
  return {
    parent: originals[0],
    children: translations,
  };
}
