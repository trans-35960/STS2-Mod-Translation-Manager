import React from "react";
import { Braces, Plus, Table2, Trash2 } from "lucide-react";
import type { GlossaryTerm } from "../../types";

type Row = { id: number; source: string; targets: string };

const TARGET_SEPARATOR = " / ";
const FILTER_THRESHOLD = 12;

/**
 * Edits `원문 → 번역` terms as a table, or as JSON (`{"원문": "번역"}`) for
 * bulk edits. Pasting a JSON object or `원문 = 번역` lines into any cell adds
 * those terms instead of filling the cell. With `keepUntranslated`, terms
 * without a translation are kept (to fill in later) and can be listed alone.
 */
export function TermTableEditor({
  terms,
  onChange,
  compact = false,
  keepUntranslated = false,
  emptyText = "등록된 용어가 없습니다.",
}: {
  terms: GlossaryTerm[];
  onChange: (terms: GlossaryTerm[]) => void;
  compact?: boolean;
  keepUntranslated?: boolean;
  emptyText?: string;
}) {
  const nextId = React.useRef(0);
  const toRows = React.useCallback(
    (list: GlossaryTerm[]) => list.map((term) => ({ id: nextId.current++, source: term.source, targets: term.targets.join(TARGET_SEPARATOR) })),
    [],
  );
  const [rows, setRows] = React.useState<Row[]>(() => toRows(terms));
  const [mode, setMode] = React.useState<"table" | "json">("table");
  const [jsonText, setJsonText] = React.useState("");
  const [jsonError, setJsonError] = React.useState("");
  const [filter, setFilter] = React.useState("");
  const [notice, setNotice] = React.useState("");
  // Rows shown by the "빈 번역" filter, fixed when it is turned on so a row
  // does not vanish while its translation is being typed.
  const [untranslatedIds, setUntranslatedIds] = React.useState<Set<number> | null>(null);
  const emitted = React.useRef(termsSignature(terms));

  // Follow outside changes (loading, terms added elsewhere) but not our own edits.
  React.useEffect(() => {
    const signature = termsSignature(terms);
    if (signature !== emitted.current) {
      emitted.current = signature;
      setRows(toRows(terms));
      setUntranslatedIds(null);
    }
  }, [terms, toRows]);

  function commit(next: Row[]) {
    setRows(next);
    const parsed = rowsToTerms(next, keepUntranslated);
    emitted.current = termsSignature(parsed);
    onChange(parsed);
  }

  function update(id: number, field: "source" | "targets", value: string) {
    commit(rows.map((row) => (row.id === id ? { ...row, [field]: value } : row)));
  }

  function merge(added: GlossaryTerm[]) {
    const bySource = new Map(added.map((term) => [term.source.toLowerCase(), term]));
    const kept = rows.filter((row) => !bySource.has(row.source.trim().toLowerCase()));
    commit([...kept.filter((row) => row.source.trim() || row.targets.trim()), ...toRows(added)]);
    setFilter("");
    setUntranslatedIds(null);
    setNotice(`용어 ${added.length}개를 넣었습니다.`);
  }

  function onCellPaste(event: React.ClipboardEvent<HTMLInputElement>) {
    const text = event.clipboardData.getData("text/plain");
    if (!/[\n{]/.test(text.trim())) {
      return;
    }
    const parsed = parseTermsInput(text, keepUntranslated);
    if (parsed && parsed.length > 0) {
      event.preventDefault();
      merge(parsed);
    }
  }

  function openJson() {
    setJsonText(formatTermsJson(rowsToTerms(rows, keepUntranslated)));
    setJsonError("");
    setMode("json");
  }

  function applyJson() {
    const parsed = parseTermsInput(jsonText, keepUntranslated);
    if (parsed === null) {
      setJsonError('JSON을 읽지 못했습니다. {"원문": "번역"} 또는 {"원문": ["번역1", "번역2"]} 형식으로 적어 주세요.');
      return;
    }
    commit(toRows(parsed));
    setUntranslatedIds(null);
    setMode("table");
    setNotice(`용어 ${parsed.length}개로 바꿨습니다.`);
  }

  const query = filter.trim().toLowerCase();
  const visible = rows.filter(
    (row) =>
      (!untranslatedIds || untranslatedIds.has(row.id)) &&
      (!query || row.source.toLowerCase().includes(query) || row.targets.toLowerCase().includes(query)),
  );
  const filled = rows.filter((row) => row.source.trim()).length;
  const untranslated = rows.filter((row) => row.source.trim() && !row.targets.trim()).length;

  return (
    <div className={compact ? "term-table compact" : "term-table"}>
      <div className="term-table-toolbar">
        <div className="term-table-mode" role="tablist" aria-label="용어 편집 방식">
          <button type="button" role="tab" aria-selected={mode === "table"} className={mode === "table" ? "active" : ""} onClick={() => setMode("table")}>
            <Table2 size={13} />표
          </button>
          <button type="button" role="tab" aria-selected={mode === "json"} className={mode === "json" ? "active" : ""} onClick={openJson}>
            <Braces size={13} />
            JSON
          </button>
        </div>
        {mode === "table" && rows.length > FILTER_THRESHOLD && (
          <input className="term-table-filter" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder={`${filled}개 중 찾기`} aria-label="용어 찾기" />
        )}
        {mode === "table" && keepUntranslated && (untranslated > 0 || untranslatedIds) && (
          <button
            type="button"
            className={untranslatedIds ? "term-table-untranslated active" : "term-table-untranslated"}
            aria-pressed={Boolean(untranslatedIds)}
            title="번역이 빈 용어만 봅니다. 번역은 비어 있으면 프롬프트와 검증에 쓰이지 않습니다."
            onClick={() =>
              setUntranslatedIds((current) =>
                current ? null : new Set(rows.filter((row) => row.source.trim() && !row.targets.trim()).map((row) => row.id)),
              )
            }
          >
            빈 번역 {untranslated}
          </button>
        )}
        {mode === "table" && (
          <button
            className="toolbar-icon-button"
            type="button"
            aria-label="용어 추가"
            data-tooltip="용어 추가"
            onClick={() => {
              setFilter("");
              setRows((current) => [...current, { id: nextId.current++, source: "", targets: "" }]);
            }}
          >
            <Plus size={14} />
          </button>
        )}
      </div>

      {mode === "table" ? (
        <>
          <div className="term-table-rows" role="table" aria-label="용어 목록">
            {rows.length > 0 && (
              <div className="term-table-row head" role="row">
                <span role="columnheader">원문</span>
                <span role="columnheader">번역 (여러 개는 /)</span>
                <span />
              </div>
            )}
            {visible.map((row) => (
              <div className="term-table-row" role="row" key={row.id}>
                <input aria-label="원문 용어" placeholder="원문" value={row.source} onPaste={onCellPaste} onChange={(event) => update(row.id, "source", event.target.value)} />
                <input aria-label="번역 용어" placeholder="번역" value={row.targets} onPaste={onCellPaste} onChange={(event) => update(row.id, "targets", event.target.value)} />
                <button
                  className="toolbar-icon-button"
                  type="button"
                  aria-label={`${row.source || "빈"} 용어 삭제`}
                  onClick={() => commit(rows.filter((other) => other.id !== row.id))}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
            {rows.length === 0 && <p className="term-table-empty">{emptyText} + 로 추가하거나 JSON을 칸에 붙여넣으세요.</p>}
            {rows.length > 0 && visible.length === 0 && (
              <p className="term-table-empty">{untranslatedIds && !query ? "번역이 빈 용어가 없습니다." : `"${filter}"에 맞는 용어가 없습니다.`}</p>
            )}
          </div>
          {notice && <p className="term-table-notice">{notice}</p>}
        </>
      ) : (
        <div className="term-table-json">
          <textarea
            data-own-paste
            value={jsonText}
            spellCheck={false}
            onChange={(event) => setJsonText(event.target.value)}
            rows={compact ? 8 : 12}
            aria-label="용어 JSON"
          />
          <div className="term-table-json-actions">
            <small>{'{"원문": "번역"} · 여러 번역은 ["번역1", "번역2"] · `원문 = 번역` 줄도 됩니다.'}</small>
            <button type="button" className="icon-button-text compact" onClick={() => setMode("table")}>
              취소
            </button>
            <button type="button" className="primary icon-button-text compact" onClick={applyJson}>
              적용
            </button>
          </div>
          {jsonError && <p className="inline-warning">{jsonError}</p>}
        </div>
      )}
    </div>
  );
}

function rowsToTerms(rows: Row[], keepUntranslated: boolean): GlossaryTerm[] {
  const seen = new Set<string>();
  const terms: GlossaryTerm[] = [];
  for (const row of rows) {
    const source = row.source.trim();
    const targets = splitTargets(row.targets);
    if (!source || (targets.length === 0 && !keepUntranslated) || seen.has(source.toLowerCase())) {
      continue;
    }
    seen.add(source.toLowerCase());
    terms.push({ source, targets });
  }
  return terms;
}

function splitTargets(value: string) {
  return value
    .split("/")
    .map((target) => target.trim())
    .filter(Boolean);
}

function termsSignature(terms: GlossaryTerm[]) {
  return JSON.stringify(terms.map((term) => [term.source, term.targets]));
}

export function formatTermsJson(terms: GlossaryTerm[]) {
  const map: Record<string, string | string[]> = {};
  for (const term of terms) {
    map[term.source] = term.targets.length === 1 ? term.targets[0] : term.targets.length === 0 ? "" : term.targets;
  }
  return JSON.stringify(map, null, 2);
}

/**
 * Reads terms from JSON (`{"원문": "번역" | ["번역", ...]}` or
 * `[{"source", "target(s)"}]`, code fences allowed) or from `원문 = 번역`,
 * `원문 → 번역` or tab-separated lines. Empty translations are kept only with
 * `allowEmpty`. Returns null when nothing fits.
 */
export function parseTermsInput(text: string, allowEmpty = false): GlossaryTerm[] | null {
  const body = text.replace(/```[a-z]*\s*/gi, "").replace(/```/g, "").trim();
  if (!body) {
    return [];
  }
  if (body.startsWith("{") || body.startsWith("[")) {
    try {
      return fromJson(JSON.parse(body), allowEmpty);
    } catch {
      // Fall through to line parsing (and fail there if it is broken JSON).
      if (body.startsWith("[") || /^\{\s*"/.test(body)) {
        return null;
      }
    }
  }
  const terms: GlossaryTerm[] = [];
  for (const line of body.split("\n")) {
    const match = line.match(/^\s*(.+?)\s*(?:=|→|->|\t)\s*(.*?)\s*,?\s*$/);
    if (!match) {
      continue;
    }
    const source = unquote(match[1]);
    const targets = splitTargets(unquote(match[2]));
    if (source && (targets.length > 0 || allowEmpty)) {
      terms.push({ source, targets });
    }
  }
  return terms.length > 0 ? dedupe(terms) : null;
}

function fromJson(value: unknown, allowEmpty: boolean): GlossaryTerm[] | null {
  const terms: GlossaryTerm[] = [];
  const toTargets = (raw: unknown) =>
    (Array.isArray(raw) ? raw : [raw]).flatMap((target) => (typeof target === "string" ? splitTargets(target) : []));
  if (Array.isArray(value)) {
    for (const item of value) {
      if (item && typeof item === "object") {
        const record = item as Record<string, unknown>;
        const source = typeof record.source === "string" ? record.source.trim() : "";
        const targets = toTargets(record.targets ?? record.target ?? record.translation);
        if (source && (targets.length > 0 || allowEmpty)) {
          terms.push({ source, targets });
        }
      }
    }
  } else if (value && typeof value === "object") {
    for (const [source, raw] of Object.entries(value as Record<string, unknown>)) {
      const targets = toTargets(raw);
      if (source.trim() && (targets.length > 0 || allowEmpty)) {
        terms.push({ source: source.trim(), targets });
      }
    }
  } else {
    return null;
  }
  return dedupe(terms);
}

function unquote(value: string) {
  return value.trim().replace(/^["'](.*)["']$/, "$1").trim();
}

function dedupe(terms: GlossaryTerm[]) {
  const bySource = new Map<string, GlossaryTerm>();
  for (const term of terms) {
    bySource.set(term.source.toLowerCase(), term);
  }
  return [...bySource.values()];
}
