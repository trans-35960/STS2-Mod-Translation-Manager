import React from "react";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  Copy,
  Eye,
  FileJson,
  Hash,
  List,
  RefreshCw,
  Replace,
  ReplaceAll,
  Trash2,
  WrapText,
} from "lucide-react";
import { Stat } from "../../components/AppShell";
import { Pill } from "../../components/Common";
import type { JsonSheetReport, JsonTranslationEntry, JsonValidationIssue, LanguagePreview, PasteCandidate } from "../../types";
import type {
  ReplaceScope,
  TranslationColumnKey,
  TranslationColumns,
  TranslationEntryFilter,
  TranslationEntryRow,
  TranslationToolsPageProps,
} from "./TranslationToolsTypes";
import {
  AutoGrowTextarea,
  CompareStack,
  PasteCandidateCard,
  ResizableHead,
} from "./TranslationWidgets";
import {
  isTabularTranslationPaste,
  languageCodeFromSheetKey,
  languageFolderCode,
  normalizedLocalizationKey,
  stableCompareKey,
  splitSheetKey,
  compactTranslationFile,
  whitespaceValueLabel,
} from "./translationUtils";
import { normalizeLanguageTag } from "../mods/modUtils";

const EMPTY_VALIDATION_ISSUES: DisplayValidationIssue[] = [];

export function TranslationSheetTable({
  props,
  sheetStats,
  activeProjectPath,
  entryFilter,
  setEntryFilter,
  entrySearch,
  setEntrySearch,
  replaceSearch,
  setReplaceSearch,
  replaceWith,
  setReplaceWith,
  showIds,
  setShowIds,
  columns,
  sheetSourceLanguage,
  selectedCompareLanguages,
  pasteCandidateCount,
  validationWarningCount,
  validationIssueKindFilter,
  showCompareColumn,
  slotIdByEntryKey,
  filteredEntries,
  visibleEntries,
  replaceMatchCount,
  startColumnResize,
  expandVisibleEntriesOnScroll,
  loadMoreVisibleEntries,
  showEntryCondition,
  showValidationIssueKind,
  replaceTranslatedValues,
  focusEntryKey,
  revealEntryKey,
  clearFocusEntryKey,
}: {
  props: TranslationToolsPageProps;
  sheetStats: JsonSheetReport | null;
  activeProjectPath: string | null;
  entryFilter: TranslationEntryFilter;
  setEntryFilter: React.Dispatch<React.SetStateAction<TranslationEntryFilter>>;
  entrySearch: string;
  setEntrySearch: React.Dispatch<React.SetStateAction<string>>;
  replaceSearch: string;
  setReplaceSearch: React.Dispatch<React.SetStateAction<string>>;
  replaceWith: string;
  setReplaceWith: React.Dispatch<React.SetStateAction<string>>;
  showIds: boolean;
  setShowIds: React.Dispatch<React.SetStateAction<boolean>>;
  columns: TranslationColumns;
  sheetSourceLanguage: string;
  selectedCompareLanguages: LanguagePreview[];
  pasteCandidateCount: number;
  validationWarningCount: number;
  validationIssueKindFilter: string | null;
  showCompareColumn: boolean;
  slotIdByEntryKey: Map<string, string>;
  filteredEntries: TranslationEntryRow[];
  visibleEntries: TranslationEntryRow[];
  replaceMatchCount: number;
  startColumnResize: (column: TranslationColumnKey, event: React.MouseEvent) => void;
  expandVisibleEntriesOnScroll: (event: React.UIEvent<HTMLDivElement>) => void;
  loadMoreVisibleEntries: () => void;
  showEntryCondition: (filter: TranslationEntryFilter) => void;
  showValidationIssueKind: (kind: string | null) => void;
  replaceTranslatedValues: (scope: ReplaceScope) => void;
  focusEntryKey: string | null;
  revealEntryKey: (key: string) => void;
  clearFocusEntryKey: () => void;
}) {
  const t = props.labels;
  const rowRefs = React.useRef(new Map<string, HTMLDivElement>());
  const [copiedIdKey, setCopiedIdKey] = React.useState<string | null>(null);
  const [validationCollapsed, setValidationCollapsed] = React.useState(false);
  const [pasteConflictCollapsed, setPasteConflictCollapsed] = React.useState(false);
  const [wrapSourceWithTranslation, setWrapSourceWithTranslation] = React.useState(false);
  const searchHighlight = entrySearch.trim();
  const showValidationWarnings = entryFilter === "warning";
  const selectedRowsSet = React.useMemo(() => new Set(props.selectedRows), [props.selectedRows]);
  const allValidationItems = React.useMemo(
    () => validationRows(props.validation),
    [props.validation],
  );
  const validationItems = React.useMemo(
    () =>
      validationIssueKindFilter
        ? allValidationItems.filter((issue) => issue.kind === validationIssueKindFilter)
        : allValidationItems,
    [allValidationItems, validationIssueKindFilter],
  );
  const validationIssueCounts = React.useMemo(
    () => summarizeValidationIssues(allValidationItems),
    [allValidationItems],
  );
  const issuesByEntryKey = React.useMemo(() => {
    const map = new Map<string, DisplayValidationIssue[]>();
    for (const issue of allValidationItems) {
      map.set(issue.key, [...(map.get(issue.key) ?? []), issue]);
    }
    return map;
  }, [allValidationItems]);
  const pasteCandidateItems = React.useMemo(() => {
    if (!props.sheet) {
      return [];
    }
    const entryByKey = new Map(
      props.sheet.entries.map((entry, index) => [
        entry.key,
        {
          entry,
          index,
          parts: splitSheetKey(entry.key),
          slotId: slotIdByEntryKey.get(entry.key) ?? "",
        },
      ]),
    );
    return Object.entries(props.pasteCandidatesByKey)
      .map(([entryKey, candidate]) => {
        const item = entryByKey.get(entryKey);
        return item ? { entryKey, candidate, ...item } : null;
      })
      .filter((item): item is NonNullable<typeof item> => Boolean(item))
      .sort((left, right) => {
        const leftLabel = `${left.parts.file}\u0000${left.slotId || left.parts.key}`;
        const rightLabel = `${right.parts.file}\u0000${right.slotId || right.parts.key}`;
        return leftLabel.localeCompare(rightLabel);
      });
  }, [props.pasteCandidatesByKey, props.sheet, slotIdByEntryKey]);

  React.useEffect(() => {
    if (allValidationItems.length > 0) {
      setValidationCollapsed(false);
    }
  }, [allValidationItems.length]);

  React.useEffect(() => {
    if (pasteCandidateCount > 0) {
      setPasteConflictCollapsed(false);
    }
  }, [pasteCandidateCount]);

  React.useEffect(() => {
    if (!focusEntryKey) {
      return;
    }
    const row = rowRefs.current.get(focusEntryKey);
    if (!row) {
      return;
    }
    row.closest(".json-entry-list")?.scrollIntoView({ block: "start" });
    window.requestAnimationFrame(() => {
      row.scrollIntoView({ block: "center", inline: "nearest" });
      const content = document.querySelector<HTMLElement>(".content");
      if (content) {
        const rowRect = row.getBoundingClientRect();
        const contentRect = content.getBoundingClientRect();
        const targetTop = rowRect.top - contentRect.top + content.scrollTop - 180;
        content.scrollTo({ top: Math.max(0, targetTop), behavior: "smooth" });
      }
      row.querySelector("textarea")?.focus();
    });
    const timeoutId = window.setTimeout(clearFocusEntryKey, 2200);
    return () => window.clearTimeout(timeoutId);
  }, [clearFocusEntryKey, focusEntryKey, visibleEntries]);

  function copyEntryId(entryKey: string, id: string, event: React.MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    const value = id || splitSheetKey(entryKey).key || entryKey;
    void navigator.clipboard?.writeText(value);
    setCopiedIdKey(entryKey);
    window.setTimeout(() => setCopiedIdKey((current) => (current === entryKey ? null : current)), 1200);
  }

  return (
    <section className="tool-results">
      {sheetStats && (
        <div className="result-strip">
          <Stat label="Entries" value={sheetStats.entries} active={entryFilter === "all" && !activeProjectPath && !entrySearch} onClick={() => showEntryCondition("all")} />
          <Stat label="New" value={sheetStats.new_entries} tone={sheetStats.new_entries ? "warn" : undefined} active={entryFilter === "new"} onClick={() => showEntryCondition("new")} />
          <Stat label="Updated" value={sheetStats.updated_entries} tone={sheetStats.updated_entries ? "warn" : undefined} active={entryFilter === "updated"} onClick={() => showEntryCondition("updated")} />
          <Stat label="Removed" value={sheetStats.removed_entries} active={entryFilter === "removed"} onClick={() => showEntryCondition("removed")} />
          <Stat label="Warning" value={validationWarningCount} tone={validationWarningCount ? "warn" : "good"} active={entryFilter === "warning"} onClick={() => showEntryCondition("warning")} />
        </div>
      )}
      {props.validation && showValidationWarnings && (
        <article className={[
          props.validation.valid ? "validation-card good" : "validation-card warn",
          validationCollapsed ? "collapsed" : "",
        ].filter(Boolean).join(" ")} aria-label="번역 검증 결과">
          <div
            className="validation-card-head"
            onClick={() => setValidationCollapsed((value) => !value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                setValidationCollapsed((value) => !value);
              }
            }}
            role="button"
            tabIndex={0}
            title={validationCollapsed ? "검증 결과 펼치기" : "검증 결과 접기"}
          >
            <div>
              <strong>
                {validationCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                {props.validation.valid ? "검증 통과" : "수정 필요"}
              </strong>
              <span>{props.validation.valid ? "번역 적용 전에 확인된 문제가 없습니다." : "항목을 클릭하면 해당 행으로 이동합니다."}</span>
            </div>
            <div className="validation-metrics">
              <ValidationMetric label="전체" value={props.validation.total_entries} />
              <ValidationMetric label="빈 값" value={props.validation.missing_entries.length} tone={props.validation.missing_entries.length ? "warn" : "good"} />
              <ValidationMetric label="원본 변경" value={props.validation.updated_entries.length} tone={props.validation.updated_entries.length ? "warn" : "good"} />
              <ValidationMetric label="삭제됨" value={props.validation.removed_entries.length} />
              <ValidationMetric label="구조" value={props.validation.format_issues?.length ?? 0} tone={(props.validation.format_issues?.length ?? 0) ? "warn" : "good"} />
            </div>
          </div>
          {!validationCollapsed && validationItems.length > 0 && (
            <div className="validation-issue-panel">
              <div className="validation-issue-chips">
                {validationIssueCounts.map((item) => (
                  <button
                    className={validationIssueKindFilter === item.kind ? "active" : ""}
                    key={item.kind}
                    type="button"
                    onClick={() => showValidationIssueKind(validationIssueKindFilter === item.kind ? null : item.kind)}
                  >
                    {validationIssueLabel(item.kind)} <b>{item.count}</b>
                  </button>
                ))}
              </div>
              <div className="validation-issue-list">
                {validationItems.map((issue, issueIndex) => (
                  <button
                    className="validation-issue-row"
                    type="button"
                    key={`${issue.kind}-${issue.key}-${issueIndex}`}
                    onClick={() => revealEntryKey(issue.key)}
                  >
                    <code title={issue.key}>{formatValidationIssueKey(issue.key)}</code>
                    <span>{issue.message}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </article>
      )}
      {pasteCandidateCount > 0 && (
        <article className={pasteConflictCollapsed ? "paste-conflict-toolbar collapsed" : "paste-conflict-toolbar"}>
          <div
            className="paste-conflict-head"
            onClick={() => setPasteConflictCollapsed((value) => !value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                setPasteConflictCollapsed((value) => !value);
              }
            }}
            role="button"
            tabIndex={0}
            title={pasteConflictCollapsed ? "붙여넣기 충돌 펼치기" : "붙여넣기 충돌 접기"}
          >
            <div>
              <strong>
                {pasteConflictCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                붙여넣기 충돌 {pasteCandidateCount}개
              </strong>
              <span>기존 번역값과 다른 새 값은 바로 덮어쓰지 않고 후보로 남겼습니다.</span>
            </div>
            <div className="paste-conflict-actions" onKeyDown={(event) => event.stopPropagation()}>
              <button type="button" onClick={(event) => {
                event.stopPropagation();
                showEntryCondition("conflict");
              }}>
                충돌만 보기
              </button>
              <button type="button" onClick={(event) => {
                event.stopPropagation();
                props.onApplyAllPasteCandidates();
              }}>
                모두 허가
              </button>
              <button type="button" onClick={(event) => {
                event.stopPropagation();
                props.onDismissAllPasteCandidates();
              }}>
                모두 취소
              </button>
            </div>
          </div>
          {!pasteConflictCollapsed && pasteCandidateItems.length > 0 && (
            <div className="paste-conflict-list" aria-label="붙여넣기 충돌 항목">
              {pasteCandidateItems.map(({ entryKey, candidate, entry, parts, slotId }) => (
                <div
                  className="paste-conflict-row"
                  key={entryKey}
                  onClick={() => revealEntryKey(entryKey)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      revealEntryKey(entryKey);
                    }
                  }}
                  role="button"
                  tabIndex={0}
                  title="행으로 이동해서 편집"
                >
                  <div className="paste-conflict-meta">
                    <b>{slotId || parts.key || "id 없음"}</b>
                    <code>{parts.file}</code>
                    <code>{parts.key}</code>
                  </div>
                  <div className="paste-conflict-values">
                    <span className="paste-conflict-value current">
                      <em>현재</em>
                      <p>{entry.translated_value || "-"}</p>
                    </span>
                    <span className="paste-conflict-value candidate">
                      <em>붙여넣기</em>
                      <p>{candidate.value || "-"}</p>
                    </span>
                    <span className="paste-conflict-source">{candidate.source}</span>
                  </div>
                  <div className="paste-conflict-row-actions">
                    <button
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        props.onApplyPasteCandidate(entryKey);
                      }}
                    >
                      허가
                    </button>
                    <button
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        props.onDismissPasteCandidate(entryKey);
                      }}
                    >
                      취소
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </article>
      )}
      {!props.sheet && <div className="empty">{t.jsonToolHint}</div>}
      {props.sheet && (
        <div
          className="json-entry-list"
          tabIndex={0}
          onPaste={(event) => {
            const text = event.clipboardData.getData("text/plain");
            if (props.onPasteStructuredJson(text)) {
              event.preventDefault();
            }
          }}
        >
          <div className="sheet-toolbar">
            <div className="sheet-title-stack">
              <strong>번역 시트</strong>
              <span>
                {props.selectedRows.length
                  ? `${props.selectedRows.length}개 행 선택`
                  : `${filteredEntries.length}개 표시 대상 / 행 클릭, Shift/Ctrl 선택`}
              </span>
            </div>
            <label className="sheet-search">
              <span>검색</span>
              <input
                value={entrySearch}
                onChange={(event) => setEntrySearch(event.target.value)}
                placeholder="id / 원본 / 번역 검색"
              />
            </label>
            <div className="sheet-replace">
              <input
                value={replaceSearch}
                onChange={(event) => setReplaceSearch(event.target.value)}
                placeholder="찾을 번역어"
                aria-label="치환할 원문"
              />
              <input
                value={replaceWith}
                onChange={(event) => setReplaceWith(event.target.value)}
                placeholder="바꿀 값"
                aria-label="치환 결과"
              />
              <button
                type="button"
                onClick={() => replaceTranslatedValues("filtered")}
                disabled={!replaceSearch || replaceMatchCount === 0}
                aria-label={`${props.selectedRows.length > 0 ? "선택 항목 치환" : "현재 표시 항목 치환"} ${replaceMatchCount}개`}
                data-tooltip={`${props.selectedRows.length > 0 ? "선택 항목 치환" : "현재 표시 항목 치환"}: ${replaceMatchCount}개`}
              >
                <Replace size={15} />
                {replaceMatchCount > 0 && <span>{replaceMatchCount}</span>}
              </button>
              <button
                type="button"
                onClick={() => replaceTranslatedValues("all")}
                disabled={!replaceSearch || !props.sheet?.entries.some((entry) => entry.translated_value.includes(replaceSearch))}
                aria-label="전체 시트 치환"
                data-tooltip="전체 시트 치환"
              >
                <ReplaceAll size={15} />
              </button>
            </div>
            <div className="toolbar-actions">
              <button className={entryFilter === "all" ? "active" : ""} aria-label="전체" data-tooltip="전체" onClick={() => setEntryFilter("all")}>
                <List size={15} />
              </button>
              <button className={entryFilter === "new" ? "active" : ""} aria-label="신규" data-tooltip="신규" onClick={() => setEntryFilter("new")}>
                <FileJson size={15} />
              </button>
              <button className={entryFilter === "empty" ? "active" : ""} aria-label="빈칸" data-tooltip="빈칸" onClick={() => setEntryFilter("empty")}>
                <CircleDashed size={15} />
              </button>
              <button className={entryFilter === "updated" ? "active" : ""} aria-label="업데이트" data-tooltip="업데이트" onClick={() => setEntryFilter("updated")}>
                <RefreshCw size={15} />
              </button>
              <button className={entryFilter === "removed" ? "active" : ""} aria-label={`삭제됨 ${sheetStats?.removed_entries ?? 0}`} data-tooltip={`삭제됨 ${sheetStats?.removed_entries ?? 0}`} onClick={() => setEntryFilter("removed")} disabled={!sheetStats?.removed_entries}>
                <Trash2 size={15} />
                {(sheetStats?.removed_entries ?? 0) > 0 && <span>{sheetStats?.removed_entries}</span>}
              </button>
              <button className={entryFilter === "conflict" ? "active" : ""} aria-label={`충돌 ${pasteCandidateCount}`} data-tooltip={`충돌 ${pasteCandidateCount}`} onClick={() => setEntryFilter("conflict")} disabled={pasteCandidateCount === 0}>
                <AlertTriangle size={15} />
                {pasteCandidateCount > 0 && <span>{pasteCandidateCount}</span>}
              </button>
              <button className={showIds ? "active" : ""} type="button" aria-label="id 보기" data-tooltip="id 보기" onClick={() => setShowIds((value) => !value)}>
                {showIds ? <Eye size={15} /> : <Hash size={15} />}
              </button>
              <button
                className={wrapSourceWithTranslation ? "active" : ""}
                type="button"
                aria-label="번역 입력 행 원본 줄바꿈"
                data-tooltip="번역 입력 행 원본 줄바꿈"
                aria-pressed={wrapSourceWithTranslation}
                onClick={() => setWrapSourceWithTranslation((value) => !value)}
              >
                <WrapText size={15} />
              </button>
              <button aria-label="선택 복사" data-tooltip="선택 복사" onClick={props.onCopySelected} disabled={props.selectedRows.length === 0}>
                <Copy size={15} />
              </button>
            </div>
          </div>
          <div
            className="json-grid"
            data-compare={showCompareColumn ? "on" : "off"}
            data-id={showIds ? "on" : "off"}
            onScroll={expandVisibleEntriesOnScroll}
            style={{
              "--col-status": `${columns.status}px`,
              "--col-id": `${columns.id}px`,
              "--col-file": `${columns.file}px`,
              "--col-key": `${columns.key}px`,
              "--col-source": `${columns.source}px`,
              "--col-compare": `${columns.compare}px`,
              "--col-translated": `${columns.translated}px`,
            } as React.CSSProperties}
          >
            <div className="json-entry-head">
              <ResizableHead label="status" onMouseDown={(event) => startColumnResize("status", event)} />
              {showIds && <ResizableHead label="id" onMouseDown={(event) => startColumnResize("id", event)} />}
              <ResizableHead label="translated_value" onMouseDown={(event) => startColumnResize("translated", event)} />
              {showCompareColumn && <ResizableHead label="compare" onMouseDown={(event) => startColumnResize("compare", event)} />}
              <ResizableHead label={sheetSourceLanguage ? `source (${sheetSourceLanguage})` : "source"} onMouseDown={(event) => startColumnResize("source", event)} />
              <ResizableHead label="file" onMouseDown={(event) => startColumnResize("file", event)} />
              <ResizableHead label="key" onMouseDown={(event) => startColumnResize("key", event)} />
            </div>
            {visibleEntries.map(({ entry, index, parts }) => (
              <JsonEntryRow
                compareValuesByLanguage={props.compareValuesByLanguage}
                copiedIdKey={copiedIdKey}
                entry={entry}
                index={index}
                issues={showValidationWarnings ? issuesByEntryKey.get(entry.key) ?? EMPTY_VALIDATION_ISSUES : EMPTY_VALIDATION_ISSUES}
                key={`${entry.status}-${entry.key}`}
                onApplyPasteCandidate={props.onApplyPasteCandidate}
                onCopyEntryId={copyEntryId}
                onDismissPasteCandidate={props.onDismissPasteCandidate}
                onEditEntry={props.onEditEntry}
                onPasteEntries={props.onPasteEntries}
                onPasteStructuredJson={props.onPasteStructuredJson}
                onSelectRow={props.onSelectRow}
                onSelectValidationIssueKind={showValidationIssueKind}
                parts={parts}
                pasteCandidate={props.pasteCandidatesByKey[entry.key]}
                registerRow={(element) => {
                  if (element) {
                    rowRefs.current.set(entry.key, element);
                  } else {
                    rowRefs.current.delete(entry.key);
                  }
                }}
                searchHighlight={searchHighlight}
                selected={selectedRowsSet.has(index)}
                selectedCompareLanguages={selectedCompareLanguages}
                sheetSourceLanguage={sheetSourceLanguage}
                showCompareColumn={showCompareColumn}
                showIds={showIds}
                slotId={slotIdByEntryKey.get(entry.key) ?? ""}
                validationIssueKindFilter={validationIssueKindFilter}
                focused={focusEntryKey === entry.key}
                wrapSource={wrapSourceWithTranslation}
              />
            ))}
            {visibleEntries.length < filteredEntries.length && (
              <button className="load-more" type="button" onClick={loadMoreVisibleEntries}>
                더 보기 {visibleEntries.length}/{filteredEntries.length}
              </button>
            )}
            {filteredEntries.length === 0 && <div className="empty compact">조건에 맞는 항목이 없습니다.</div>}
          </div>
        </div>
      )}
    </section>
  );
}

const JsonEntryRow = React.memo(function JsonEntryRow({
  compareValuesByLanguage,
  copiedIdKey,
  entry,
  focused,
  index,
  issues,
  onApplyPasteCandidate,
  onCopyEntryId,
  onDismissPasteCandidate,
  onEditEntry,
  onPasteEntries,
  onPasteStructuredJson,
  onSelectRow,
  onSelectValidationIssueKind,
  parts,
  pasteCandidate,
  registerRow,
  searchHighlight,
  selected,
  selectedCompareLanguages,
  sheetSourceLanguage,
  showCompareColumn,
  showIds,
  slotId,
  validationIssueKindFilter,
  wrapSource,
}: {
  compareValuesByLanguage: Record<string, Record<string, string>>;
  copiedIdKey: string | null;
  entry: JsonTranslationEntry;
  focused: boolean;
  index: number;
  issues: DisplayValidationIssue[];
  onApplyPasteCandidate: (entryKey: string) => void;
  onCopyEntryId: (entryKey: string, id: string, event: React.MouseEvent) => void;
  onDismissPasteCandidate: (entryKey: string) => void;
  onEditEntry: (index: number, value: string) => void;
  onPasteEntries: (startIndex: number, text: string) => void;
  onPasteStructuredJson: (text: string) => boolean;
  onSelectRow: (index: number, event: React.MouseEvent) => void;
  onSelectValidationIssueKind: (kind: string | null) => void;
  parts: TranslationEntryRow["parts"];
  pasteCandidate: PasteCandidate | undefined;
  registerRow: (element: HTMLDivElement | null) => void;
  searchHighlight: string;
  selected: boolean;
  selectedCompareLanguages: LanguagePreview[];
  sheetSourceLanguage: string;
  showCompareColumn: boolean;
  showIds: boolean;
  slotId: string;
  validationIssueKindFilter: string | null;
  wrapSource: boolean;
}) {
  const issueKinds = React.useMemo(() => issues.map((issue) => issue.kind), [issues]);
  const whitespaceLabel = whitespaceValueLabel(entry.translated_value);
  const sourceCellClass = [
    "source-value-cell",
    issues.length > 0 ? "warning-source" : "",
    wrapSource && entry.translated_value.trim() ? "wrapped-source" : "",
  ].filter(Boolean).join(" ");

  return (
    <div
      className={[
        "json-entry-row",
        selected ? "selected" : "",
        focused ? "focus-pulse" : "",
        issues.length > 0 ? "warning-row" : "",
      ].filter(Boolean).join(" ")}
      data-entry-key={entry.key}
      onClick={(event) => onSelectRow(index, event)}
      ref={registerRow}
    >
      <Pill tone={entry.status === "updated" || entry.status === "new" || entry.status === "missing" ? "warn" : "good"}>{entry.status}</Pill>
      {showIds && (
        <code
          className={copiedIdKey === entry.key ? "short-id-cell copied" : "short-id-cell"}
          title={`${slotId || entry.key} - 더블클릭하면 id를 복사합니다.`}
          onClick={(event) => event.stopPropagation()}
          onDoubleClick={(event) => onCopyEntryId(entry.key, slotId, event)}
        >
          <HighlightedText text={slotId} query={searchHighlight} />
          {copiedIdKey === entry.key && <small>복사됨</small>}
        </code>
      )}
      <div className="translation-value-cell">
        <AutoGrowTextarea
          value={entry.translated_value}
          placeholder="-"
          onChange={(event) => onEditEntry(index, event.target.value)}
          onPaste={(event) => {
            const text = event.clipboardData.getData("text/plain");
            if (onPasteStructuredJson(text)) {
              event.preventDefault();
              return;
            }
            if (isTabularTranslationPaste(text)) {
              event.preventDefault();
              onPasteEntries(index, text);
            }
          }}
        />
        {searchHighlight && includesSearch(entry.translated_value, searchHighlight) && (
          <div className="search-match-preview" aria-label="translated_value 검색 결과">
            <HighlightedText text={entry.translated_value} query={searchHighlight} />
          </div>
        )}
        {issues.length > 0 && (
          <IssueBadges
            activeKind={validationIssueKindFilter}
            issues={issues}
            onSelect={onSelectValidationIssueKind}
          />
        )}
        {issues.length > 0 && (
          <div className="issue-match-preview" aria-label="translated_value 구조 경고 강조">
            <HighlightedIssueText kinds={issueKinds} text={entry.translated_value || "(빈 번역값)"} query={searchHighlight} />
          </div>
        )}
        {whitespaceLabel && (
          <small className="whitespace-marker">{whitespaceLabel}</small>
        )}
        {pasteCandidate && (
          <PasteCandidateCard
            candidate={pasteCandidate}
            onApply={() => onApplyPasteCandidate(entry.key)}
            onDismiss={() => onDismissPasteCandidate(entry.key)}
          />
        )}
      </div>
      {showCompareColumn && (
        <CompareStack
          items={selectedCompareLanguages.map((language) => ({
            id: `${language.sample_path}-${entry.key}`,
            code: languageFolderCode(language),
            value: compareValueForEntry(entry, language, compareValuesByLanguage, sheetSourceLanguage),
          }))}
        />
      )}
      <span className={sourceCellClass} title={entry.source_value}>
        <HighlightedIssueText kinds={issueKinds} text={entry.source_value} query={searchHighlight} />
      </span>
      <code title={parts.file}>
        <HighlightedText text={parts.file} query={searchHighlight} />
      </code>
      <code title={parts.key}>
        <HighlightedText text={parts.key} query={searchHighlight} />
      </code>
    </div>
  );
});

function HighlightedText({ text, query }: { text: string; query: string }) {
  if (!query) {
    return <>{text}</>;
  }
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const parts: React.ReactNode[] = [];
  let cursor = 0;
  let matchIndex = lowerText.indexOf(lowerQuery);
  while (matchIndex !== -1) {
    if (matchIndex > cursor) {
      parts.push(text.slice(cursor, matchIndex));
    }
    const matchEnd = matchIndex + query.length;
    parts.push(
      <mark className="search-hit" key={`${matchIndex}-${matchEnd}`}>
        {text.slice(matchIndex, matchEnd)}
      </mark>,
    );
    cursor = matchEnd;
    matchIndex = lowerText.indexOf(lowerQuery, cursor);
  }
  if (cursor < text.length) {
    parts.push(text.slice(cursor));
  }
  return <>{parts}</>;
}

function HighlightedIssueText({
  text,
  kinds,
  query = "",
}: {
  text: string;
  kinds: string[];
  query?: string;
}) {
  const ranges = issueHighlightRanges(text, kinds);
  if (ranges.length === 0) {
    return <HighlightedText text={text} query={query} />;
  }
  const parts: React.ReactNode[] = [];
  let cursor = 0;
  for (const [rangeIndex, range] of ranges.entries()) {
    if (range.start > cursor) {
      const plain = text.slice(cursor, range.start);
      parts.push(<HighlightedText key={`plain-${cursor}-${range.start}`} text={plain} query={query} />);
    }
    parts.push(
      <mark className={`issue-hit ${range.kind}`} key={`${range.kind}-${range.start}-${range.end}-${rangeIndex}`}>
        {text.slice(range.start, range.end)}
      </mark>,
    );
    cursor = range.end;
  }
  if (cursor < text.length) {
    parts.push(<HighlightedText key={`plain-${cursor}-${text.length}`} text={text.slice(cursor)} query={query} />);
  }
  return <>{parts}</>;
}

function IssueBadges({
  activeKind,
  issues,
  onSelect,
}: {
  activeKind: string | null;
  issues: DisplayValidationIssue[];
  onSelect: (kind: string | null) => void;
}) {
  return (
    <div className="row-warning-badges" aria-label="검증 경고 종류">
      {issues.map((issue, issueIndex) => (
        <button
          className={activeKind === issue.kind ? `active ${issue.kind}` : issue.kind}
          key={`${issue.kind}-${issue.key}-${issue.message}-${issueIndex}`}
          title={issue.message}
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onSelect(activeKind === issue.kind ? null : issue.kind);
          }}
        >
          {validationIssueLabel(issue.kind)}
          {issue.occurrenceCount > 1 && <b>{issue.occurrenceCount}</b>}
        </button>
      ))}
    </div>
  );
}

function includesSearch(value: string, query: string) {
  return value.toLowerCase().includes(query.toLowerCase());
}

function issueHighlightRanges(text: string, kinds: string[]) {
  const wanted = new Set(kinds);
  const ranges: Array<{ start: number; end: number; kind: string }> = [];
  if (wanted.has("angle_tags")) {
    collectPatternRanges(text, /<\/?[^>\n]{1,80}>/g, "angle_tags", ranges);
  }
  if (wanted.has("square_tags")) {
    collectPatternRanges(text, /\[[^\]\n]{1,80}\]/g, "square_tags", ranges);
  }
  if (wanted.has("placeholders")) {
    collectPatternRanges(text, /\{[^}\n]{1,80}\}/g, "placeholders", ranges);
  }
  if (wanted.has("bang_tokens")) {
    collectPatternRanges(text, /![A-Za-z0-9]{1,12}!/g, "bang_tokens", ranges);
  }
  if (wanted.has("line_break_marker")) {
    collectPatternRanges(text, /\bNL\b/g, "line_break_marker", ranges);
  }
  return ranges
    .sort((left, right) => left.start - right.start || right.end - left.end)
    .filter((range, index, sorted) => index === 0 || range.start >= sorted[index - 1].end);
}

function collectPatternRanges(
  text: string,
  pattern: RegExp,
  kind: string,
  ranges: Array<{ start: number; end: number; kind: string }>,
) {
  for (const match of text.matchAll(pattern)) {
    const start = match.index ?? 0;
    ranges.push({ start, end: start + match[0].length, kind });
  }
}

function compareValueForEntry(
  entry: JsonTranslationEntry,
  language: LanguagePreview,
  valuesByLanguage: Record<string, Record<string, string>>,
  sheetSourceLanguage: string,
) {
  const sourceLanguage = languageCodeFromSheetKey(entry.key) || sheetSourceLanguage;
  const compareLanguage = languageFolderCode(language);
  if (sourceLanguage && normalizeLanguageTag(sourceLanguage) === normalizeLanguageTag(compareLanguage)) {
    return entry.source_value;
  }
  const values = valuesByLanguage[language.sample_path];
  if (!values) {
    return "";
  }
  return values[entry.key] ?? values[normalizedLocalizationKey(entry.key)] ?? values[stableCompareKey(entry.key)] ?? "";
}

function ValidationMetric({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: "good" | "warn";
}) {
  return (
    <span className={tone ? `validation-metric ${tone}` : "validation-metric"}>
      <small>{label}</small>
      <b>{value}</b>
    </span>
  );
}

type DisplayValidationIssue = JsonValidationIssue & { occurrenceCount: number };

function formatValidationIssueKey(key: string) {
  const parts = splitSheetKey(key);
  if (!key.startsWith("file://")) {
    return key;
  }
  const compactFile = compactTranslationFile(parts.file);
  return parts.key ? `${compactFile}#${parts.key}` : compactFile;
}

function validationRows(validation: TranslationToolsPageProps["validation"]): DisplayValidationIssue[] {
  if (!validation) {
    return [];
  }
  const rows = [
    ...validation.missing_entries.map((key) => ({
      key,
      kind: "missing",
      message: "번역값이 비어 있습니다.",
    })),
    ...validation.updated_entries.map((key) => ({
      key,
      kind: "updated",
      message: "원본 값이 이전 시트와 달라졌습니다. 번역을 확인하거나 수정하면 경고에서 제외됩니다.",
    })),
    ...validation.removed_entries.map((key) => ({
      key,
      kind: "removed",
      message: "원본에서 삭제된 항목입니다.",
    })),
    ...(validation.format_issues ?? []),
  ];
  const groupedRows = new Map<string, JsonValidationIssue & { count: number }>();
  for (const row of rows) {
    const mapKey = `${row.kind}\u0000${row.key}\u0000${row.message}`;
    const current = groupedRows.get(mapKey);
    if (current) {
      current.count += 1;
    } else {
      groupedRows.set(mapKey, { ...row, count: 1 });
    }
  }
  return Array.from(groupedRows.values()).map(({ count, ...row }) => ({
    ...row,
    occurrenceCount: count,
    message: count > 1 ? `${row.message} (${count}건)` : row.message,
  }));
}

function summarizeValidationIssues(issues: DisplayValidationIssue[]) {
  const counts = new Map<string, number>();
  for (const issue of issues) {
    counts.set(issue.kind, (counts.get(issue.kind) ?? 0) + issue.occurrenceCount);
  }
  return Array.from(counts.entries())
    .map(([kind, count]) => ({ kind, count }))
    .sort((left, right) => right.count - left.count || validationIssueLabel(left.kind).localeCompare(validationIssueLabel(right.kind)));
}

function validationIssueLabel(kind: string) {
  switch (kind) {
    case "line_breaks":
      return "줄바꿈";
    case "line_break_marker":
      return "NL";
    case "angle_tags":
      return "꺾쇠 태그";
    case "square_tags":
      return "대괄호";
    case "placeholders":
      return "플레이스홀더";
    case "bang_tokens":
      return "토큰";
    case "missing":
      return "빈 값";
    case "updated":
      return "원본 변경";
    case "removed":
      return "삭제됨";
    default:
      return kind;
  }
}
