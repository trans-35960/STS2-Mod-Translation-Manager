import React from "react";
import {
  Bug,
  ChevronDown,
  ChevronRight,
  Gamepad2,
  Languages,
  List,
  ListOrdered,
  Play,
  Search,
  CheckSquare,
  ShieldAlert,
  GitBranch,
  ShieldCheck,
  Square,
  Trash2,
  X,
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
import { DllTranslateModal } from "./DllTranslateModal";
import { ExtractConfirmModal } from "./ExtractModal";
import { ImportMenu } from "./ImportMenu";
import { ModGroupTableRow, ModTableRow, SimpleModGroupRow, SimpleModRow } from "./ModRows";
import { ModErrorsModal } from "./ModErrorsModal";
import { ModLoadOrderModal } from "./ModLoadOrderModal";
import { PresetMenu } from "./PresetMenu";
import {
  buildModGroups,
  canDeleteMod,
  languageLabel,
  preferredGroupActivationTarget,
} from "./modUtils";
import {
  TRANSLATION_SOURCE_HINTS,
  TRANSLATION_SOURCE_LABELS,
  groupTranslationStatus,
  translationSection,
  type TranslationSection,
  type TranslationSource,
  type TranslationStatus,
} from "./translationStatus";
import { errorOccurrences, useModErrors, type ModErrorState } from "./useModErrors";

type DetailModColumnKey = "name" | "version" | "date" | "translation" | "language" | "actions";
type SimpleModColumnKey = "name" | "translation" | "actions";
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
  translation: 150,
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
  translation: 110,
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
  gameBranch: string | null;
  gameVersion: string | null;
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
  onDeleteSelected: (mods: ModRow[]) => Promise<boolean> | boolean | void;
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
}) {
  const t = props.labels;
  const [expandedGroups, setExpandedGroups] = React.useState<Record<string, boolean>>({});
  const [selectedModKeys, setSelectedModKeys] = React.useState<Set<string>>(() => new Set());
  const [simpleView, setSimpleView] = React.useState(() => props.initialSimpleView || readStoredModViewMode());
  const [detailColumns, setDetailColumns] = React.useState(() =>
    clampModColumns(readStoredModTableColumns("detail", defaultDetailModColumns), minDetailModColumns),
  );
  const [simpleColumns, setSimpleColumns] = React.useState(() =>
    clampModColumns(readStoredModTableColumns("simple", defaultSimpleModColumns), minSimpleModColumns),
  );
  const groups = React.useMemo(
    () =>
      buildModGroups(
        props.mods,
        props.search,
        props.statFilter,
        props.activeFilter,
        props.changeFilter,
        props.translationApplyFilter,
        props.sort,
        props.targetLanguage,
      ),
    [props.mods, props.search, props.statFilter, props.activeFilter, props.changeFilter, props.translationApplyFilter, props.sort, props.targetLanguage],
  );
  const sections = React.useMemo(() => buildModSections(groups, props.targetLanguage), [groups, props.targetLanguage]);
  const modErrors = useModErrors(props.mods, props.launchRunning);
  const [errorModalKey, setErrorModalKey] = React.useState<string | null>(null);
  const [dllMod, setDllMod] = React.useState<ModRow | null>(null);
  const [loadOrderOpen, setLoadOrderOpen] = React.useState(false);
  const [collapsedSections, setCollapsedSections] = React.useState<Partial<Record<TranslationSection, boolean>>>({});
  const [sectionFacets, setSectionFacets] = React.useState<Partial<Record<TranslationSection, string>>>({});
  const showChangeDetails = props.statFilter === "changed";
  const vortexDownloadCount = React.useMemo(
    () => props.mods.filter((mod) => isVortexDownloadMod(mod) && mod.download_state !== "downloading").length,
    [props.mods],
  );
  const selectableMods = React.useMemo(() => {
    const byKey = new Map<string, ModRow>();
    for (const group of groups) {
      for (const mod of group.mods) {
        if (canDeleteMod(mod)) {
          byKey.set(mod.key, mod);
        }
      }
    }
    return Array.from(byKey.values());
  }, [groups]);
  const selectedMods = React.useMemo(
    () => selectableMods.filter((mod) => selectedModKeys.has(mod.key)),
    [selectableMods, selectedModKeys],
  );
  const allSelectableSelected = selectableMods.length > 0 && selectableMods.every((mod) => selectedModKeys.has(mod.key));

  function toggleGroup(groupId: string) {
    setExpandedGroups((current) => ({ ...current, [groupId]: !current[groupId] }));
  }

  React.useEffect(() => {
    setSelectedModKeys((current) => {
      const validKeys = new Set(props.mods.filter(canDeleteMod).map((mod) => mod.key));
      const next = new Set(Array.from(current).filter((key) => validKeys.has(key)));
      return next.size === current.size ? current : next;
    });
  }, [props.mods]);

  React.useEffect(() => {
    writeStoredModTableColumns("detail", detailColumns);
  }, [detailColumns]);

  React.useEffect(() => {
    writeStoredModTableColumns("simple", simpleColumns);
  }, [simpleColumns]);

  // On/off only changes what the next launch loads, so it stays available
  // while the game runs; deleting moves folders the game may hold open.
  const modChangesLocked = props.launchRunning;

  // The focused mod stays highlighted, but the list scrolls to it only once
  // per visit: re-running on every list update (polling, Workshop titles and
  // thumbnails arriving) kept dragging the view back to it.
  const scrolledFocusKeyRef = React.useRef<string | null>(null);
  const focusScrollTimerRef = React.useRef<number | undefined>(undefined);
  React.useEffect(() => {
    if (!props.focusedModKey) {
      scrolledFocusKeyRef.current = null;
      return;
    }
    if (scrolledFocusKeyRef.current === props.focusedModKey) {
      return;
    }
    const group = groups.find((item) => item.mods.some((mod) => mod.key === props.focusedModKey));
    if (!group) {
      return;
    }
    scrolledFocusKeyRef.current = props.focusedModKey;
    const section = translationSection(groupTranslationStatus(group.mods, props.targetLanguage));
    setCollapsedSections((current) => (current[section] ? { ...current, [section]: false } : current));
    setSectionFacets((current) => (current[section] ? { ...current, [section]: undefined } : current));
    if (group.mods.length > 1) {
      setExpandedGroups((current) => ({ ...current, [group.id]: true }));
    }
    const escapedKey = props.focusedModKey.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    // Not cancelled by a list update that lands meanwhile, only on unmount.
    focusScrollTimerRef.current = window.setTimeout(() => {
      const row = document.querySelector<HTMLElement>(`[data-mod-key="${escapedKey}"]`);
      row?.scrollIntoView({ behavior: "smooth", block: "center" });
      row?.focus({ preventScroll: true });
    }, 120);
  }, [groups, props.focusedModKey, props.targetLanguage]);
  React.useEffect(() => () => window.clearTimeout(focusScrollTimerRef.current), []);

  function setStoredSimpleView(value: boolean) {
    setSimpleView(value);
    writeStoredModViewMode(value);
    if (!isPreviewRuntime()) {
      void invokeCommand("save_mod_view_mode", { modViewMode: value ? "simple" : "detail" });
    }
  }

  function startDetailColumnResize(column: DetailModColumnKey, event: React.MouseEvent) {
    startColumnResize(event, detailColumns[column], minDetailModColumns[column], detailColumnVar(column), (width) => {
      setDetailColumns((current) => ({ ...current, [column]: width }));
    });
  }

  function startSimpleColumnResize(column: SimpleModColumnKey, event: React.MouseEvent) {
    startColumnResize(event, simpleColumns[column], minSimpleModColumns[column], simpleColumnVar(column), (width) => {
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
      "--mod-simple-translation-col": `${simpleColumns.translation}px`,
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

  function toggleModSelection(mod: ModRow) {
    if (!canDeleteMod(mod)) {
      return;
    }
    setSelectedModKeys((current) => {
      const next = new Set(current);
      if (next.has(mod.key)) {
        next.delete(mod.key);
      } else {
        next.add(mod.key);
      }
      return next;
    });
  }

  function toggleGroupSelection(group: ModGroup) {
    const deletable = group.mods.filter(canDeleteMod);
    if (deletable.length === 0) {
      return;
    }
    const allSelected = deletable.every((mod) => selectedModKeys.has(mod.key));
    setSelectedModKeys((current) => {
      const next = new Set(current);
      for (const mod of deletable) {
        if (allSelected) {
          next.delete(mod.key);
        } else {
          next.add(mod.key);
        }
      }
      return next;
    });
  }

  function toggleAllVisibleSelection() {
    setSelectedModKeys((current) => {
      const next = new Set(current);
      if (allSelectableSelected) {
        for (const mod of selectableMods) {
          next.delete(mod.key);
        }
      } else {
        for (const mod of selectableMods) {
          next.add(mod.key);
        }
      }
      return next;
    });
  }

  async function deleteSelectedMods() {
    if (selectedMods.length === 0) {
      return;
    }
    const deleted = await props.onDeleteSelected(selectedMods);
    if (deleted !== false) {
      setSelectedModKeys(new Set());
    }
  }

  function openErrors(modKey: string | null) {
    setErrorModalKey(modKey ?? ALL_ERRORS);
  }

  function rowProps(mod: ModRow) {
    return {
      labels: t,
      mod,
      selected: selectedModKeys.has(mod.key),
      targetLanguage: props.targetLanguage,
      busy: props.busy,
      toggling: props.togglingModKeys.has(mod.key),
      locked: modChangesLocked,
      onToggle: props.onToggle,
      onOpenPath: props.onOpenPath,
      onDelete: props.onDelete,
      onSelect: toggleModSelection,
      onExtract: props.onExtract,
      onStartModTranslation: props.onStartModTranslation,
      focused: props.focusedModKey === mod.key,
      showChangeDetails,
      errors: modErrors.byMod.get(mod.key),
      onOpenErrors: openErrors,
      onTranslateDll: setDllMod,
    };
  }

  function renderModRow(
    mod: ModRow,
    extra: { child?: boolean; attachedChildCount?: number; expanded?: boolean; onToggleExpand?: () => void } = {},
  ) {
    return simpleView ? <SimpleModRow key={mod.key} {...rowProps(mod)} {...extra} /> : <ModTableRow key={mod.key} {...rowProps(mod)} {...extra} />;
  }

  function renderGroup(group: ModGroup) {
    const isGrouped = group.mods.length > 1;
    const attachedTranslationGroup = attachedTranslationMods(group);
    const isExpanded = expandedGroups[group.id] ?? (attachedTranslationGroup ? false : props.activeFilter !== "all" || props.changeFilter !== "all" || props.translationApplyFilter !== "all");
    if (attachedTranslationGroup) {
      const { parent, children } = attachedTranslationGroup;
      return (
        <React.Fragment key={group.id}>
          {renderModRow(parent, { attachedChildCount: children.length, expanded: isExpanded, onToggleExpand: () => toggleGroup(group.id) })}
          {isExpanded && children.map((mod) => renderModRow(mod, { child: true }))}
        </React.Fragment>
      );
    }
    if (!isGrouped) {
      return renderModRow(group.mods[0]);
    }
    const groupProps = {
      group,
      busy: props.busy,
      toggling: group.mods.some((mod) => props.togglingModKeys.has(mod.key)),
      locked: modChangesLocked,
      selectedCount: group.mods.filter((mod) => selectedModKeys.has(mod.key) && canDeleteMod(mod)).length,
      selectableCount: group.mods.filter(canDeleteMod).length,
      expanded: isExpanded,
      onToggleGroup: () => void toggleGroupEnabled(group),
      onSelectGroup: () => toggleGroupSelection(group),
      onToggleExpand: () => toggleGroup(group.id),
      showChangeDetails,
      errorsByMod: modErrors.byMod,
      onOpenErrors: openErrors,
    };
    return (
      <React.Fragment key={group.id}>
        {simpleView ? <SimpleModGroupRow {...groupProps} targetLanguage={props.targetLanguage} /> : <ModGroupTableRow {...groupProps} />}
        {isExpanded && group.mods.map((mod) => renderModRow(mod, { child: true }))}
      </React.Fragment>
    );
  }

  return (
    <>
      <div className="mod-lock-frame">
      {modChangesLocked && (
        <div className="mod-running-notice" role="status">
          <ShieldAlert size={15} />
          게임 실행 중입니다. 켜기/끄기는 다음 실행부터 적용되고, 삭제는 게임을 종료한 뒤 할 수 있습니다.
        </div>
      )}
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
          <option value="applied">번역됨</option>
          <option value="notApplied">번역 안 됨</option>
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
        <span className="mod-selection-actions">
          <button
            type="button"
            className="toolbar-icon-button mod-select-all"
            aria-label={allSelectableSelected ? "현재 표시 모드 선택 해제" : `현재 표시 모드 선택 (${selectedMods.length}개 선택됨)`}
            data-tooltip={allSelectableSelected ? "현재 표시 모드 선택 해제" : `현재 표시 모드 선택 (${selectedMods.length}개 선택됨)`}
            onClick={toggleAllVisibleSelection}
            disabled={Boolean(props.busy) || modChangesLocked || selectableMods.length === 0}
          >
            {allSelectableSelected ? <CheckSquare size={16} /> : <Square size={16} />}
          </button>
          <button
            className="toolbar-icon-button danger"
            type="button"
            aria-label={`선택 모드 삭제 ${selectedMods.length}개`}
            data-tooltip={`선택 모드 삭제 ${selectedMods.length}개`}
            onClick={() => void deleteSelectedMods()}
            disabled={Boolean(props.busy) || modChangesLocked || selectedMods.length === 0}
          >
            <Trash2 size={16} />
          </button>
        </span>
        <span
          className={`launch-status-chip ${props.vanillaSafe ? "good" : "warn"}`}
          aria-label={props.vanillaSafe ? t.safe : t.unsafe}
          data-tooltip={props.vanillaSafe ? t.safe : t.unsafe}
        >
          {props.vanillaSafe ? <ShieldCheck size={15} /> : <ShieldAlert size={15} />}
        </span>
        {props.gameBranch && (
          <span
            className={`launch-status-chip${props.gameBranch === "public" ? "" : " beta"}`}
            aria-label={gameBranchLabel(props.gameBranch, props.gameVersion)}
            data-tooltip={gameBranchLabel(props.gameBranch, props.gameVersion)}
          >
            <GitBranch size={15} />
          </span>
        )}
        <button
          className="toolbar-icon-button"
          type="button"
          aria-label="모드 로드 순서"
          data-tooltip="모드 로드 순서"
          onClick={() => setLoadOrderOpen(true)}
          disabled={Boolean(props.busy)}
        >
          <ListOrdered size={16} />
        </button>
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
      <ModErrorSummary state={modErrors} launchRunning={props.launchRunning} onOpen={() => openErrors(null)} />
      <div className={simpleView ? "table mod-table simple" : "table mod-table detail"} style={tableStyle}>
        {simpleView ? (
          <div className="table-head mod-simple-grid">
            <ModResizableHead label="모드 이름" />
            <ModResizableHead label="번역" handleEdge="start" onMouseDown={(event) => startSimpleColumnResize("translation", event)} />
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
        {/* Both views share the sections, so switching keeps the order and grouping. */}
        {sections.map((section) => (
              <React.Fragment key={section.id}>
                <ModSectionHeader
                  section={section}
                  collapsed={Boolean(collapsedSections[section.id])}
                  facetKey={sectionFacets[section.id]}
                  onToggle={() => setCollapsedSections((current) => ({ ...current, [section.id]: !current[section.id] }))}
                  onFacet={(key) => {
                    setSectionFacets((current) => ({ ...current, [section.id]: key }));
                    if (key) {
                      setCollapsedSections((current) => ({ ...current, [section.id]: false }));
                    }
                  }}
                />
                {!collapsedSections[section.id] && visibleSectionGroups(section, sectionFacets[section.id]).map(renderGroup)}
              </React.Fragment>
            ))}
        {groups.length === 0 && <div className="empty compact">조건에 맞는 모드가 없습니다.</div>}
      </div>
      </div>
      {errorModalKey && (
        <ModErrorsModal
          state={modErrors}
          mods={props.mods}
          modKey={errorModalKey === ALL_ERRORS ? null : errorModalKey}
          game={{ gameVersion: props.gameVersion, gameBranch: props.gameBranch }}
          onClose={() => setErrorModalKey(null)}
          onOpenPath={props.onOpenPath}
        />
      )}
      {loadOrderOpen && <ModLoadOrderModal mods={props.mods} onClose={() => setLoadOrderOpen(false)} />}
      {dllMod && (
        <DllTranslateModal
          mod={dllMod}
          onCancel={() => setDllMod(null)}
          onOpen={(dllPath) => {
            const mod = dllMod;
            setDllMod(null);
            props.onStartModTranslation(mod, dllPath);
          }}
        />
      )}
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
  cssVariable: string,
  setWidth: (width: number) => void,
) {
  event.preventDefault();
  event.stopPropagation();
  const startX = event.clientX;
  const previousCursor = document.body.style.cursor;
  const previousUserSelect = document.body.style.userSelect;
  const table = event.currentTarget.closest<HTMLElement>(".mod-table");
  const startedAt = window.performance.now();
  let latestWidth = startWidth;
  let moveCount = 0;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  const onMove = (moveEvent: MouseEvent) => {
    moveCount += 1;
    latestWidth = Math.max(minWidth, Math.min(920, startWidth + moveEvent.clientX - startX));
    table?.style.setProperty(cssVariable, `${latestWidth}px`);
  };
  const onUp = () => {
    document.body.style.cursor = previousCursor;
    document.body.style.userSelect = previousUserSelect;
    setWidth(latestWidth);
    const elapsed = Math.round(window.performance.now() - startedAt);
    if (elapsed >= 250 || moveCount >= 10) {
      console.info(`[perf] column_resize ${cssVariable} ${elapsed}ms moves=${moveCount}`);
    }
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", onUp);
  };
  document.addEventListener("mousemove", onMove);
  document.addEventListener("mouseup", onUp);
}

function detailColumnVar(column: DetailModColumnKey): string {
  switch (column) {
    case "name":
      return "--mod-name-col";
    case "version":
      return "--mod-version-col";
    case "date":
      return "--mod-date-col";
    case "translation":
      return "--mod-translation-col";
    case "language":
      return "--mod-language-col";
    case "actions":
      return "--mod-actions-col";
  }
}

function simpleColumnVar(column: SimpleModColumnKey): string {
  switch (column) {
    case "name":
      return "--mod-simple-name-col";
    case "translation":
      return "--mod-simple-translation-col";
    case "actions":
      return "--mod-simple-actions-col";
  }
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

const ALL_ERRORS = "*";

/** A filter chip in a section header: how many mods are translated (or not) a given way. */
type SectionFacet = {
  key: string;
  label: string;
  count: number;
  hint: string;
  matches: (status: TranslationStatus) => boolean;
};

type ModSection = {
  id: TranslationSection;
  title: string;
  hint: string;
  groups: ModGroup[];
  /** Status of each group, in the same order. */
  statuses: TranslationStatus[];
  facets: SectionFacet[];
};

const SECTION_ORDER: TranslationSection[] = ["system", "translated", "untranslated"];

/** Simple view: the system (bridge) mod apart, then translated and untranslated mods. */
function buildModSections(groups: ModGroup[], targetLanguage: string): ModSection[] {
  const byId = new Map<TranslationSection, { groups: ModGroup[]; statuses: TranslationStatus[] }>();
  for (const group of groups) {
    const status = groupTranslationStatus(group.mods, targetLanguage);
    const id = translationSection(status);
    const entry = byId.get(id) ?? { groups: [], statuses: [] };
    entry.groups.push(group);
    entry.statuses.push(status);
    byId.set(id, entry);
  }
  const language = languageLabel(targetLanguage);
  return SECTION_ORDER.flatMap((id): ModSection[] => {
    const entry = byId.get(id);
    if (!entry) {
      return [];
    }
    const facets = (candidates: Array<Omit<SectionFacet, "count">>) =>
      candidates
        .map((facet) => ({ ...facet, count: entry.statuses.filter(facet.matches).length }))
        .filter((facet) => facet.count > 0);
    if (id === "system") {
      return [{ id, title: "시스템 모드", hint: "이 프로그램이 설치·관리하는 모드입니다", ...entry, facets: [] }];
    }
    if (id === "translated") {
      return [
        {
          id,
          title: `${language} 번역됨`,
          hint: "자체 번역, 패키지 수정, 브리지 번역, 번역 모드 중 하나 이상으로 번역된 모드",
          ...entry,
          facets: facets(
            (Object.keys(TRANSLATION_SOURCE_LABELS) as TranslationSource[]).map((source) => ({
              key: source,
              label: TRANSLATION_SOURCE_LABELS[source],
              hint: TRANSLATION_SOURCE_HINTS[source],
              matches: (status: TranslationStatus) => status.kind === "translated" && status.sources.includes(source),
            })),
          ),
        },
      ];
    }
    return [
      {
        id,
        title: `${language} 번역 안 됨`,
        hint: "내 언어 번역이 없는 모드",
        ...entry,
        facets: facets([
          {
            key: "needsTranslation",
            label: "번역 필요",
            hint: "언어 파일이나 DLL 문자열이 있어 번역할 수 있는 모드",
            matches: (status) => status.kind === "untranslated" && !status.patchOff,
          },
          {
            key: "patchOff",
            label: "번역 모드 꺼짐",
            hint: "번역 모드가 설치돼 있지만 꺼져 있는 모드",
            matches: (status) => status.kind === "untranslated" && status.patchOff,
          },
          {
            key: "noText",
            label: "언어 파일 없음",
            hint: "언어 파일을 찾지 못한 모드",
            matches: (status) => status.kind === "noText",
          },
        ]),
      },
    ];
  });
}

/** The section's groups narrowed to the chosen facet, if any. */
function visibleSectionGroups(section: ModSection, facetKey: string | undefined): ModGroup[] {
  const facet = section.facets.find((item) => item.key === facetKey);
  return facet ? section.groups.filter((_, index) => facet.matches(section.statuses[index])) : section.groups;
}

function ModSectionHeader({
  section,
  collapsed,
  facetKey,
  onToggle,
  onFacet,
}: {
  section: ModSection;
  collapsed: boolean;
  facetKey: string | undefined;
  onToggle: () => void;
  onFacet: (key: string | undefined) => void;
}) {
  const shown = visibleSectionGroups(section, facetKey).length;
  return (
    <div className={`mod-section-header ${section.id}`}>
      <button type="button" className="mod-section-toggle" aria-expanded={!collapsed} onClick={onToggle} data-tooltip={collapsed ? "펼치기" : "접기"}>
        {collapsed ? <ChevronRight size={15} /> : <ChevronDown size={15} />}
        <strong>{section.title}</strong>
        <span className="mod-section-count">{facetKey ? `${shown}/${section.groups.length}` : section.groups.length}</span>
      </button>
      {section.facets.length > 0 && (
        <div className="mod-section-facets" role="group" aria-label={`${section.title} 필터`}>
          {section.facets.map((facet) => {
            const active = facet.key === facetKey;
            return (
              <button
                type="button"
                className={active ? "mod-section-facet active" : "mod-section-facet"}
                aria-pressed={active}
                key={facet.key}
                data-tooltip={active ? "필터 해제" : `${facet.hint}만 보기`}
                onClick={() => onFacet(active ? undefined : facet.key)}
              >
                {facet.label}
                <b>{facet.count}</b>
                {active && <X size={11} />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** One line about errors the latest game run logged; opens them all. */
function ModErrorSummary({ state, launchRunning, onOpen }: { state: ModErrorState; launchRunning: boolean; onOpen: () => void }) {
  const errors = state.report?.errors ?? [];
  if (errors.length === 0) {
    return null;
  }
  const total = errorOccurrences(errors);
  const unattributed = errorOccurrences(state.unattributed);
  return (
    <div className="mod-error-summary" role="status">
      <Bug size={15} />
      <span>
        {launchRunning ? "실행 중인 게임" : "최근 게임 실행"}에서 오류 <strong>{total}회</strong>
        {state.byMod.size > 0 && ` · 원인 모드 ${state.byMod.size}개`}
        {unattributed > 0 && ` · 원인 미상 ${unattributed}회`}
      </span>
      <button type="button" onClick={onOpen}>
        오류 보기·복사
      </button>
    </div>
  );
}

function gameBranchLabel(branch: string, version: string | null): string {
  const kind = branch === "public" ? "정식" : "베타";
  return `Steam 브랜치: ${branch} (${kind})${version ? ` · 게임 ${version}` : ""}`;
}
