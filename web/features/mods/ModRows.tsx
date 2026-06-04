import React from "react";
import {
  ChevronDown,
  ChevronRight,
  DownloadCloud,
  FolderOpen,
  Languages,
  Loader2,
  TriangleAlert,
  Trash2,
} from "lucide-react";
import { IconButton, Pill, SourceTags, StatusBadge } from "../../components/Common";
import { labels } from "../../i18n";
import type { ModDependency, ModGroup, ModRow } from "../../types";
import { LanguageBadges, RepresentativeLanguageBadge } from "./ModBadges";
import { ModTranslationActions } from "./ModTranslationActions";
import {
  activeModSummary,
  activeModVersionSummary,
  canDeleteMod,
  compactDateSummary,
  compactLanguageSummary,
  compactModifiedSummary,
  compactSourceSummary,
  compactTranslationApplyDate,
  compactVersionSummary,
  defaultTranslatableResourcePath,
  displayModVersion,
  formatFullDateTime,
  formatShortDate,
  groupTranslationSummary,
  isDownloadingMod,
  languageResourceRoot,
  needsDeferredTranslationAnalysis,
  recommendedSourceLanguage,
  representativeLanguage,
  shortPath,
  uniqueLanguagePreviews,
} from "./modUtils";

function ModTableRow({
  labels: t,
  mod,
  selected,
  targetLanguage,
  busy,
  toggling,
  locked = false,
  onToggle,
  onOpenPath,
  onDelete,
  onSelect,
  onExtract,
  onStartModTranslation,
  child = false,
  attachedChildCount = 0,
  expanded = false,
  onToggleExpand,
  focused = false,
  showChangeDetails = false,
}: {
  labels: typeof labels.ko;
  mod: ModRow;
  selected: boolean;
  targetLanguage: string;
  busy: string | null;
  toggling: boolean;
  locked?: boolean;
  onToggle: (mod: ModRow) => Promise<void> | void;
  onOpenPath: (path: string) => void;
  onDelete: (mod: ModRow) => void;
  onSelect: (mod: ModRow) => void;
  onExtract: (mod: ModRow) => void;
  onStartModTranslation: (mod: ModRow, resourcePath?: string) => void;
  child?: boolean;
  attachedChildCount?: number;
  expanded?: boolean;
  onToggleExpand?: () => void;
  focused?: boolean;
  showChangeDetails?: boolean;
}) {
  const primaryLanguage = recommendedSourceLanguage(mod.language_preview);
  const primaryResourcePath = primaryLanguage ? languageResourceRoot(primaryLanguage.sample_path) : defaultTranslatableResourcePath(mod);
  const changed = showChangeDetails && mod.update_state !== "clean";
  const rowChangeClass = changed ? ` ${modChangeClass(mod.update_state)}` : "";
  const dependencyWarnings = warningDependencies(mod);
  const translationBusy = isTranslationPreparing(busy, mod.key);
  const downloading = isDownloadingMod(mod);
  const canAnalyzeOnDemand = needsDeferredTranslationAnalysis(mod);
  return (
    <div
      className={`${child ? "table-row mod-grid mod-version-row" : "table-row mod-grid"}${mod.active ? "" : " inactive"}${focused ? " focused-mod" : ""}${changed ? " changed-mod" : ""}${rowChangeClass}`}
      data-mod-key={mod.key}
      tabIndex={-1}
    >
      <div className="mod-title">
        <ModSelectionCheckbox
          checked={selected}
          disabled={Boolean(busy) || locked || !canDeleteMod(mod)}
          label={`${mod.name} 선택`}
          onChange={() => onSelect(mod)}
        />
        <strong>
          <DependencyWarningIcon dependencies={dependencyWarnings} />
          {child ? childModTitle(mod) : mod.name}
          {mod.is_translation_patch && <StatusBadge tone="info">번역</StatusBadge>}
          {downloading && <StatusBadge tone="info">다운로드 중</StatusBadge>}
          {mod.update_state === "new" && <StatusBadge tone="info">NEW</StatusBadge>}
          {mod.needs_recheck && <StatusBadge tone="danger">Recheck</StatusBadge>}
          {mod.translation_review_required && <StatusBadge tone="danger">Translation</StatusBadge>}
          {mod.dependencies.some((dependency) => !dependency.available) && <StatusBadge tone="danger">Missing Dep</StatusBadge>}
          {hasDependencyVersionMismatch(mod) && <StatusBadge tone="danger">Dep Version</StatusBadge>}
        </strong>
        <small>{sourcePathLine(mod)}</small>
        {changed && <ChangeReasonList reasons={mod.change_reasons} />}
        {mod.dependencies.length > 0 && <DependencyList dependencies={mod.dependencies} />}
      </div>
      <div className={changeCellClass(mod, "version-source-cell", showChangeDetails, ["파일 크기", "새로 감지"])}>
        <span>{displayModVersion(mod)}</span>
        <SourceTags value={mod.source_label} />
        <Pill tone={mod.update_state === "clean" ? "good" : "warn"}>{changeStateLabel(mod.update_state)}</Pill>
      </div>
      <DateCell mod={mod} showChangeDetails={showChangeDetails} />
      <TranslationApplyCell mod={mod} />
      <div className="language-cell">
        <LanguageBadges languages={uniqueLanguagePreviews(mod.language_preview)} fallback={mod.translation_state} targetLanguage={targetLanguage} />
        <small>{mod.extraction_hint}</small>
        <ModTranslationActions mod={mod} busy={busy} onStartModTranslation={onStartModTranslation} />
      </div>
      <div className="row-actions">
        {attachedChildCount > 0 && (
          <button
            className="icon-only-button attached-toggle"
            type="button"
            aria-label={expanded ? "번역모드 접기" : "번역모드 펼치기"}
            data-tooltip={expanded ? "번역모드 접기" : "번역모드 펼치기"}
            onClick={onToggleExpand}
            disabled={Boolean(busy)}
          >
            {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </button>
        )}
        <ToggleSwitch active={mod.active} pending={toggling} locked={Boolean(busy) || locked || downloading} label={mod.name} onToggle={() => onToggle(mod)} />
        <IconButton
          label={translationBusy ? `${mod.name} 번역 도구 준비 중...` : `${mod.name} 번역 도구에서 열기`}
          icon={translationBusy ? Loader2 : Languages}
          onClick={() => onStartModTranslation(mod, primaryResourcePath)}
          disabled={Boolean(busy) || locked || downloading || (!primaryResourcePath && mod.extraction_tree.length === 0 && !canAnalyzeOnDemand)}
          loading={translationBusy}
        />
        <IconButton label={`${mod.name} 경로 열기`} icon={FolderOpen} onClick={() => onOpenPath(mod.path)} disabled={Boolean(busy) || !mod.path} />
        <IconButton label={`${mod.name} ${t.extract}`} icon={DownloadCloud} onClick={() => onExtract(mod)} disabled={Boolean(busy) || locked || downloading} />
        <IconButton
          label={`${mod.name} 삭제`}
          icon={Trash2}
          onClick={() => onDelete(mod)}
          disabled={Boolean(busy) || locked || !canDeleteMod(mod)}
          danger
        />
      </div>
    </div>
  );
}

function ModGroupTableRow({
  group,
  busy,
  toggling,
  locked,
  selectedCount,
  selectableCount,
  expanded,
  onToggleGroup,
  onSelectGroup,
  onToggleExpand,
  showChangeDetails = false,
}: {
  group: ModGroup;
  busy: string | null;
  toggling: boolean;
  locked: boolean;
  selectedCount: number;
  selectableCount: number;
  expanded: boolean;
  onToggleGroup: () => void;
  onSelectGroup: () => void;
  onToggleExpand: () => void;
  showChangeDetails?: boolean;
}) {
  const changed = showChangeDetails && group.updateCount > 0;
  const rowChangeClass = changed ? ` ${groupChangeClass(group.mods)}` : "";
  const dependencyWarnings = warningDependenciesForGroup(group.mods);
  const downloading = group.mods.some(isDownloadingMod);
  const summaryMods = activeSummaryMods(group.mods);
  const handleBlankClick = (event: React.MouseEvent<HTMLDivElement>) => {
    if (isExpandableRowBlankClick(event)) {
      onToggleExpand();
    }
  };
  return (
    <div
      className={`table-row mod-grid mod-group-row${changed ? " changed-mod" : ""}${rowChangeClass}`}
      role="button"
      tabIndex={0}
      onClick={handleBlankClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onToggleExpand();
        }
      }}
    >
      <div className="mod-title">
        <ModSelectionCheckbox
          checked={selectableCount > 0 && selectedCount === selectableCount}
          disabled={Boolean(busy) || locked || selectableCount === 0}
          label={`${group.name} 그룹 선택`}
          onChange={onSelectGroup}
        />
        <strong>
          <DependencyWarningIcon dependencies={dependencyWarnings} />
          {group.name}
          <StatusBadge>GROUP</StatusBadge>
          {group.activeCount > 0 && <StatusBadge tone="info">{activeModVersionSummary(group.mods)}</StatusBadge>}
          {downloading && <StatusBadge tone="info">다운로드 중</StatusBadge>}
        </strong>
        <small>{group.mods.length}개 버전 · {activeModSummary(group.mods)} · 변경 {group.updateCount}</small>
      </div>
      <div className="version-source-cell">
        <span>{compactVersionSummary(summaryMods)}</span>
        <small>{compactSourceSummary(summaryMods)}</small>
        <Pill tone={group.updateCount > 0 ? "warn" : "good"}>{group.updateCount > 0 ? "updated" : "clean"}</Pill>
      </div>
      <div className="date-cell">
        <span>{compactDateSummary(summaryMods)}</span>
        <small>{compactModifiedSummary(summaryMods)}</small>
      </div>
      <div className="translation-apply-cell">
        <Pill tone={summaryMods.some((mod) => mod.translation_applied) ? "good" : "warn"}>{groupTranslationSummary(summaryMods)}</Pill>
        <small>{compactTranslationApplyDate(summaryMods)}</small>
      </div>
      <div className="language-cell">
        <strong>{compactLanguageSummary(summaryMods)}</strong>
        <small>클릭해서 버전별 항목 보기</small>
      </div>
      <div className="row-actions group-row-actions" onClick={(event) => event.stopPropagation()}>
        <ToggleSwitch active={group.activeCount > 0} pending={toggling} locked={Boolean(busy) || locked || (downloading && group.activeCount === 0)} label={`${group.name} 그룹`} onToggle={onToggleGroup} />
        <button className="icon-only-button" type="button" aria-label={expanded ? "접기" : "열기"} data-tooltip={expanded ? "접기" : "열기"} onClick={onToggleExpand}>
          {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </button>
      </div>
    </div>
  );
}

function SimpleModGroupRow({
  group,
  targetLanguage,
  busy,
  toggling,
  locked,
  selectedCount,
  selectableCount,
  expanded,
  onToggleGroup,
  onSelectGroup,
  onToggleExpand,
  showChangeDetails = false,
}: {
  group: ModGroup;
  targetLanguage: string;
  busy: string | null;
  toggling: boolean;
  locked: boolean;
  selectedCount: number;
  selectableCount: number;
  expanded: boolean;
  onToggleGroup: () => void;
  onSelectGroup: () => void;
  onToggleExpand: () => void;
  showChangeDetails?: boolean;
}) {
  const changed = showChangeDetails && group.updateCount > 0;
  const rowChangeClass = changed ? ` changed-mod ${groupChangeClass(group.mods)}` : "";
  const dependencyWarnings = warningDependenciesForGroup(group.mods);
  const downloading = group.mods.some(isDownloadingMod);
  const summaryMods = activeSummaryMods(group.mods);
  const handleBlankClick = (event: React.MouseEvent<HTMLDivElement>) => {
    if (isExpandableRowBlankClick(event)) {
      onToggleExpand();
    }
  };
  return (
    <div
      className={`table-row mod-simple-grid mod-group-row simple${rowChangeClass ? ` ${rowChangeClass}` : ""}`}
      role="button"
      tabIndex={0}
      onClick={handleBlankClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onToggleExpand();
        }
      }}
    >
      <div className="mod-title">
        <ModSelectionCheckbox
          checked={selectableCount > 0 && selectedCount === selectableCount}
          disabled={Boolean(busy) || locked || selectableCount === 0}
          label={`${group.name} 그룹 선택`}
          onChange={onSelectGroup}
        />
        <strong>
          <DependencyWarningIcon dependencies={dependencyWarnings} />
          {group.name}
          <StatusBadge>GROUP</StatusBadge>
          {group.activeCount > 0 && <StatusBadge tone="info">{activeModVersionSummary(group.mods)}</StatusBadge>}
          {downloading && <StatusBadge tone="info">다운로드 중</StatusBadge>}
        </strong>
        <small>{group.mods.length}개 버전 · {activeModSummary(group.mods)}</small>
      </div>
      <div className="version-source-cell">
        <span>{compactVersionSummary(summaryMods)}</span>
      </div>
      <div className="language-cell">
        <RepresentativeLanguageBadge
          languages={uniqueLanguagePreviews(summaryMods.flatMap((mod) => mod.language_preview))}
          fallback={groupTranslationSummary(summaryMods)}
          targetLanguage={targetLanguage}
        />
      </div>
      <div className="row-actions simple-row-actions" onClick={(event) => event.stopPropagation()}>
        <ToggleSwitch active={group.activeCount > 0} pending={toggling} locked={Boolean(busy) || locked || (downloading && group.activeCount === 0)} label={`${group.name} 그룹`} onToggle={onToggleGroup} />
        <button className="icon-only-button" type="button" aria-label={expanded ? "접기" : "열기"} data-tooltip={expanded ? "접기" : "열기"} onClick={onToggleExpand}>
          {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </button>
      </div>
    </div>
  );
}

function SimpleModRow({
  labels: t,
  mod,
  selected,
  targetLanguage,
  busy,
  toggling,
  locked = false,
  onToggle,
  onOpenPath,
  onDelete,
  onSelect,
  onExtract,
  onStartModTranslation,
  child = false,
  attachedChildCount = 0,
  expanded = false,
  onToggleExpand,
  focused = false,
  showChangeDetails = false,
}: {
  labels: typeof labels.ko;
  mod: ModRow;
  selected: boolean;
  targetLanguage: string;
  busy: string | null;
  toggling: boolean;
  locked?: boolean;
  onToggle: (mod: ModRow) => Promise<void> | void;
  onOpenPath: (path: string) => void;
  onDelete: (mod: ModRow) => void;
  onSelect: (mod: ModRow) => void;
  onExtract: (mod: ModRow) => void;
  onStartModTranslation: (mod: ModRow, resourcePath?: string) => void;
  child?: boolean;
  attachedChildCount?: number;
  expanded?: boolean;
  onToggleExpand?: () => void;
  focused?: boolean;
  showChangeDetails?: boolean;
}) {
  const selectedLanguage = representativeLanguage(mod.language_preview, targetLanguage);
  const primaryResourcePath = selectedLanguage ? languageResourceRoot(selectedLanguage.sample_path) : defaultTranslatableResourcePath(mod);
  const changed = showChangeDetails && mod.update_state !== "clean";
  const rowChangeClass = changed ? ` ${modChangeClass(mod.update_state)}` : "";
  const dependencyWarnings = warningDependencies(mod);
  const translationBusy = isTranslationPreparing(busy, mod.key);
  const downloading = isDownloadingMod(mod);
  const canAnalyzeOnDemand = needsDeferredTranslationAnalysis(mod);
  return (
    <div
      className={`${child ? "table-row mod-simple-grid mod-version-row simple" : "table-row mod-simple-grid simple"}${mod.active ? "" : " inactive"}${focused ? " focused-mod" : ""}${changed ? " changed-mod" : ""}${rowChangeClass}`}
      data-mod-key={mod.key}
      tabIndex={-1}
    >
      <div className="mod-title">
        <ModSelectionCheckbox
          checked={selected}
          disabled={Boolean(busy) || locked || !canDeleteMod(mod)}
          label={`${mod.name} 선택`}
          onChange={() => onSelect(mod)}
        />
        <strong>
          <DependencyWarningIcon dependencies={dependencyWarnings} />
          {child ? childModTitle(mod) : mod.name}
          {mod.is_translation_patch && <StatusBadge tone="info">번역</StatusBadge>}
          {downloading && <StatusBadge tone="info">다운로드 중</StatusBadge>}
          {mod.update_state === "new" && <StatusBadge tone="info">NEW</StatusBadge>}
          {mod.needs_recheck && <StatusBadge tone="danger">Recheck</StatusBadge>}
          {mod.dependencies.some((dependency) => !dependency.available) && <StatusBadge tone="danger">Missing Dep</StatusBadge>}
          {hasDependencyVersionMismatch(mod) && <StatusBadge tone="danger">Dep Version</StatusBadge>}
        </strong>
        <small>{sourcePathLine(mod)}</small>
        {changed && <ChangeReasonList reasons={mod.change_reasons} />}
      </div>
      <div className={changeCellClass(mod, "version-source-cell", showChangeDetails, ["파일 크기", "새로 감지"])}>
        <span>{displayModVersion(mod)}</span>
      </div>
      <div className="language-cell">
        <RepresentativeLanguageBadge languages={uniqueLanguagePreviews(mod.language_preview)} fallback={mod.translation_state} targetLanguage={targetLanguage} />
      </div>
      <div className="row-actions simple-row-actions">
        {attachedChildCount > 0 && (
          <button
            className="icon-only-button attached-toggle"
            type="button"
            aria-label={expanded ? "번역모드 접기" : "번역모드 펼치기"}
            data-tooltip={expanded ? "번역모드 접기" : "번역모드 펼치기"}
            onClick={onToggleExpand}
            disabled={Boolean(busy)}
          >
            {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </button>
        )}
        <ToggleSwitch active={mod.active} pending={toggling} locked={Boolean(busy) || locked || downloading} label={mod.name} onToggle={() => onToggle(mod)} />
        <IconButton
          label={translationBusy ? `${mod.name} 번역 도구 준비 중...` : `${mod.name} 번역 도구에서 열기`}
          icon={translationBusy ? Loader2 : Languages}
          onClick={() => onStartModTranslation(mod, primaryResourcePath)}
          disabled={Boolean(busy) || locked || downloading || (!primaryResourcePath && mod.extraction_tree.length === 0 && !canAnalyzeOnDemand)}
          loading={translationBusy}
        />
        <IconButton label={`${mod.name} 경로 열기`} icon={FolderOpen} onClick={() => onOpenPath(mod.path)} disabled={Boolean(busy) || !mod.path} />
        <IconButton label={`${mod.name} ${t.extract}`} icon={DownloadCloud} onClick={() => onExtract(mod)} disabled={Boolean(busy) || locked || downloading} />
        <IconButton
          label={`${mod.name} 삭제`}
          icon={Trash2}
          onClick={() => onDelete(mod)}
          disabled={Boolean(busy) || locked || !canDeleteMod(mod)}
          danger
        />
      </div>
    </div>
  );
}

function ModSelectionCheckbox({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled: boolean;
  label: string;
  onChange: () => void;
}) {
  return (
    <label className="mod-row-select" aria-label={label} data-tooltip={label} onClick={(event) => event.stopPropagation()}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={onChange}
      />
    </label>
  );
}

function DateCell({ mod, showChangeDetails }: { mod: ModRow; showChangeDetails: boolean }) {
  return (
    <div className={changeCellClass(mod, "date-cell", showChangeDetails, ["수정일", "새로 감지"])} title={`등록 ${formatFullDateTime(mod.registered_epoch)} / 업데이트 ${formatFullDateTime(mod.updated_epoch)} / 파일 수정 ${formatFullDateTime(mod.modified_epoch)}`}>
      <span>등록 {formatShortDate(mod.registered_epoch)}</span>
      <small>업데이트 {formatShortDate(mod.updated_epoch)}</small>
      <small>파일 {formatShortDate(mod.modified_epoch)}</small>
    </div>
  );
}

function ChangeReasonList({ reasons }: { reasons: string[] }) {
  const labels = reasons.length > 0 ? reasons : ["변경 감지"];
  return (
    <div className="change-reason-list">
      {labels.map((reason) => (
        <span key={reason}>{reason}</span>
      ))}
    </div>
  );
}

function DependencyWarningIcon({ dependencies }: { dependencies: ModDependency[] }) {
  if (dependencies.length === 0) {
    return null;
  }
  const labels = dependencies.map(dependencyWarningLabel);
  return (
    <span className="dependency-warning-icon" title={`선행 모드 확인 필요: ${labels.join(", ")}`} aria-label="선행 모드 확인 필요">
      <TriangleAlert size={15} />
    </span>
  );
}

function isTranslationPreparing(busy: string | null, modKey: string): boolean {
  return busy === `prepare_translation_node:${modKey}` || busy === "prepare_translation_node";
}

function activeSummaryMods(mods: ModRow[]): ModRow[] {
  const activeMods = mods.filter((mod) => mod.active);
  return activeMods.length === 1 ? activeMods : mods;
}

function isExpandableRowBlankClick(event: React.MouseEvent<HTMLDivElement>): boolean {
  const target = event.target;
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.closest("button, label, input, select, textarea, a")) {
    return false;
  }
  if (target.closest("strong, small, span, svg, path")) {
    return false;
  }
  const blankTarget = target.closest(".mod-group-row, .mod-title, .version-source-cell, .date-cell, .translation-apply-cell, .language-cell");
  return blankTarget === event.currentTarget || blankTarget === target;
}

function ToggleSwitch({
  active,
  pending,
  locked,
  label,
  onToggle,
}: {
  active: boolean;
  pending: boolean;
  locked: boolean;
  label: string;
  onToggle: () => void;
}) {
  const actionLabel = active ? `${label} 비활성화` : `${label} 활성화`;
  return (
    <label
      className={pending ? "switch compact-switch pending" : "switch compact-switch"}
      aria-label={pending ? `${label} 전환 중...` : actionLabel}
      data-tooltip={pending ? `${label} 전환 중...` : actionLabel}
      aria-busy={pending}
    >
      <input type="checkbox" checked={active} onChange={onToggle} disabled={pending || locked} />
      <span></span>
      {pending && <Loader2 size={14} className="spin-icon switch-spinner" aria-hidden="true" />}
    </label>
  );
}

function warningDependencies(mod: ModRow): ModDependency[] {
  if (!mod.active) {
    return [];
  }
  return mod.dependencies.filter((dependency) => (dependency.available && !dependency.active) || dependency.version_matches === false);
}

function warningDependenciesForGroup(mods: ModRow[]): ModDependency[] {
  const byId = new Map<string, ModDependency>();
  for (const dependency of mods.flatMap(warningDependencies)) {
    byId.set(dependency.id, dependency);
  }
  return Array.from(byId.values());
}

function hasDependencyVersionMismatch(mod: ModRow): boolean {
  return mod.dependencies.some((dependency) => dependency.version_matches === false);
}

function dependencyWarningLabel(dependency: ModDependency): string {
  if (dependency.version_matches === false) {
    return `${dependency.name} 버전 불일치 (${dependency.version_current ?? "-"} / 필요 ${dependency.version_required ?? "-"})`;
  }
  return `${dependency.name} 비활성`;
}

function childModTitle(mod: ModRow): string {
  return mod.is_translation_patch ? mod.name : displayModVersion(mod);
}

function changeCellClass(mod: ModRow, baseClass: string, showChangeDetails: boolean, reasons: string[]): string {
  if (!showChangeDetails || mod.update_state === "clean") {
    return baseClass;
  }
  if (mod.change_reasons.some((reason) => reasons.includes(reason))) {
    return `${baseClass} change-highlight`;
  }
  return baseClass;
}

function changeStateLabel(value: string): string {
  if (value === "new") return "신규";
  if (value === "updated") return "업데이트";
  return value;
}

function modChangeClass(value: string): string {
  if (value === "new") return "new-mod";
  if (value === "updated") return "updated-mod";
  return "updated-mod";
}

function groupChangeClass(mods: ModRow[]): string {
  const changed = mods.filter((mod) => mod.update_state !== "clean");
  const hasNew = changed.some((mod) => mod.update_state === "new");
  const hasUpdated = changed.some((mod) => mod.update_state !== "new");
  if (hasNew && hasUpdated) return "mixed-mod";
  if (hasNew) return "new-mod";
  return "updated-mod";
}

function sourcePathLine(mod: ModRow): string {
  const path = shortPath(mod.path);
  return mod.source_label ? `${mod.source_label} · ${path}` : path;
}

function TranslationApplyCell({ mod }: { mod: ModRow }) {
  const hasPatch = mod.translation_patch_count > 0;
  const patchActive = mod.translation_patch_active_count > 0;
  const tone = mod.translation_applied || patchActive ? "good" : hasPatch ? "info" : "warn";
  const label = mod.translation_applied ? "적용됨" : patchActive ? "번역모드 활성" : hasPatch ? "번역모드 있음" : "미적용";
  const detail = mod.translation_applied
    ? formatShortDate(mod.translation_applied_epoch)
    : hasPatch
      ? `${mod.translation_patch_names.slice(0, 2).join(", ")}${mod.translation_patch_count > 2 ? ` +${mod.translation_patch_count - 2}` : ""}`
      : "직접 적용 없음";
  return (
    <div className="translation-apply-cell">
      <Pill tone={tone}>{label}</Pill>
      <small>{detail}</small>
    </div>
  );
}

function DependencyList({ dependencies }: { dependencies: ModDependency[] }) {
  return (
    <div className="dependency-list">
      <span>선행</span>
      {dependencies.map((dependency) => {
        const tone = !dependency.available ? "missing" : dependency.active ? "active" : "inactive";
        const label = dependency.available ? dependency.name : dependency.id;
        const versionLabel = dependency.version_required ? ` @ ${dependency.version_required}` : "";
        const title = dependency.available
          ? `${dependency.name}: ${dependency.active ? "활성" : "비활성"}${dependency.version_required ? ` / 필요 ${dependency.version_required}, 현재 ${dependency.version_current ?? "-"}` : ""}`
          : `${dependency.id}: 없음`;
        return (
          <b className={dependency.version_matches === false ? "missing" : tone} title={title} key={dependency.id}>
            {label}{versionLabel}
          </b>
        );
      })}
    </div>
  );
}

export { ModGroupTableRow, ModTableRow, SimpleModGroupRow, SimpleModRow };
