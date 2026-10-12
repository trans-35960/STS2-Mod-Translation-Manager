import React from "react";
import { Check, Copy, ListPlus, Loader2, Search } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { AiPromptParts, JsonTranslationSheet, TermCandidate } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { parsePastedTranslationJson } from "./translationUtils";

type Row = TermCandidate & { checked: boolean; translation: string };

const KIND_LABELS: Record<string, string> = {
  highlight: "강조",
  title: "이름",
  capitalized: "대문자",
};

/**
 * Mechanical glossary candidates for the open sheet. The user reviews them,
 * fills missing translations (by hand or with a terms-only AI prompt), and
 * adds the chosen ones to the mod glossary before translating the sheet.
 */
export function TermCandidatesPanel({
  sheet,
  projectKey,
  context,
  registeredSources,
  onAddTerms,
}: {
  sheet: JsonTranslationSheet;
  projectKey: string;
  context: { modName: string; modDescription: string; tone: string; extraInstructions: string; maxChars: number };
  registeredSources: Set<string>;
  onAddTerms: (terms: Array<{ source: string; target: string }>) => void;
}) {
  const [rows, setRows] = React.useState<Row[] | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [message, setMessage] = React.useState("");
  const [prompts, setPrompts] = React.useState<AiPromptParts | null>(null);
  const [copied, setCopied] = React.useState<Set<number>>(() => new Set());
  // Sources shown by the "번역어 없는 것만" filter, fixed when it is turned on
  // so a row stays while its translation is typed.
  const [untranslatedOnly, setUntranslatedOnly] = React.useState<Set<string> | null>(null);

  async function extract() {
    setBusy(true);
    setMessage("");
    setPrompts(null);
    setUntranslatedOnly(null);
    try {
      const candidates = isPreviewRuntime()
        ? previewCandidates()
        : await invokeCommand("extract_term_candidates", { sheet, projectKey: projectKey || null });
      setRows(
        candidates
          .filter((candidate) => !registeredSources.has(candidate.source.toLowerCase()))
          .map((candidate) => ({
            ...candidate,
            // Capitalized-only words are often plain rules words; leave them unchecked.
            checked: candidate.kinds.some((kind) => kind !== "capitalized"),
            translation: candidate.suggestions[0]?.text ?? "",
          })),
      );
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function update(source: string, patch: Partial<Row>) {
    setRows((current) => current?.map((row) => (row.source === source ? { ...row, ...patch } : row)) ?? current);
  }

  const checked = rows?.filter((row) => row.checked) ?? [];
  const untranslated = checked.filter((row) => !row.translation.trim());
  const ready = checked.filter((row) => row.translation.trim());
  const allUntranslated = rows?.filter((row) => !row.translation.trim()) ?? [];
  const visibleRows = untranslatedOnly ? (rows ?? []).filter((row) => untranslatedOnly.has(row.source)) : rows ?? [];

  async function buildTermPrompt() {
    setBusy(true);
    setMessage("");
    try {
      const terms = untranslated.map((row) => ({ source: row.source, example: row.example }));
      const parts = isPreviewRuntime()
        ? {
            total_rows: terms.length,
            parts: [{ text: JSON.stringify(Object.fromEntries(terms.map((term) => [term.source, ""])), null, 2), rows: terms.length, chars: 0, terms: terms.length, oversized: false, slots: [] }],
          }
        : await invokeCommand("build_term_prompt", {
            sheet,
            request: { ...context, terms },
          });
      setPrompts(parts);
      setCopied(new Set());
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function copyPrompt(index: number) {
    const part = prompts?.parts[index];
    if (!part) {
      return;
    }
    try {
      await navigator.clipboard.writeText(part.text);
      setCopied((current) => new Set(current).add(index));
    } catch {
      setMessage("클립보드에 쓰지 못했습니다.");
    }
  }

  function applyTermAnswer(text: string) {
    const parsed = parsePastedTranslationJson(text);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      setMessage("용어 답변 JSON({\"원문\": \"번역\"})을 찾지 못했습니다.");
      return;
    }
    const answers = new Map(
      Object.entries(parsed as Record<string, unknown>)
        .filter((entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].trim() !== "")
        .map(([source, target]) => [source.trim().toLowerCase(), target.trim()]),
    );
    if (!rows) {
      return;
    }
    let filled = 0;
    const next = rows.map((row) => {
      const answer = answers.get(row.source.toLowerCase());
      if (!answer) {
        return row;
      }
      filled += 1;
      return { ...row, translation: answer, checked: true };
    });
    setRows(next);
    setUntranslatedOnly(null);
    setMessage(`용어 답변 ${answers.size}개 중 ${filled}개를 채웠습니다. 확인 후 "용어에 추가"를 누르세요.`);
  }

  function addReady() {
    onAddTerms(ready.map((row) => ({ source: row.source, target: row.translation.trim() })));
    const added = new Set(ready.map((row) => row.source));
    setRows((current) => current?.filter((row) => !added.has(row.source)) ?? current);
    setMessage(`${added.size}개를 모드 전용 용어에 추가했습니다.`);
  }

  return (
    <div className="term-candidates">
      <div className="term-candidates-actions">
        <button type="button" className="icon-button-text compact" onClick={() => void extract()} disabled={busy}>
          {busy && !rows ? <Loader2 className="spin-icon" size={14} /> : <Search size={14} />}
          {rows ? "후보 다시 뽑기" : "용어 후보 뽑기"}
        </button>
        {rows && rows.length > 0 && (
          <>
            <span className="term-candidates-count">
              후보 {rows.length} · 선택 {checked.length} (번역어 있음 {ready.length})
            </span>
            <button type="button" className="icon-button-text compact" onClick={() => void buildTermPrompt()} disabled={busy || untranslated.length === 0}>
              <Copy size={14} />
              번역어 없는 {untranslated.length}개 용어 프롬프트
            </button>
            <button
              type="button"
              className={untranslatedOnly ? "term-table-untranslated active" : "term-table-untranslated"}
              aria-pressed={Boolean(untranslatedOnly)}
              disabled={allUntranslated.length === 0 && !untranslatedOnly}
              title="번역어가 빈 후보만 봅니다."
              onClick={() => setUntranslatedOnly((current) => (current ? null : new Set(allUntranslated.map((row) => row.source))))}
            >
              번역어 없는 것만 {allUntranslated.length}
            </button>
            <button type="button" className="primary icon-button-text compact" onClick={addReady} disabled={ready.length === 0}>
              <ListPlus size={14} />
              용어에 추가 {ready.length}
            </button>
          </>
        )}
      </div>
      {message && <p className="term-candidates-message">{message}</p>}

      {prompts && prompts.parts.length > 0 && (
        <div className="term-prompt-parts">
          {prompts.parts.map((part, index) => (
            <button type="button" key={index} className={copied.has(index) ? "copied" : ""} onClick={() => void copyPrompt(index)}>
              {copied.has(index) ? <Check size={13} /> : <Copy size={13} />}
              용어 프롬프트 {index + 1}/{prompts.parts.length} 복사 ({part.rows}개)
            </button>
          ))}
          <textarea
            data-own-paste
            rows={2}
            value=""
            onChange={() => undefined}
            onPaste={(event) => {
              event.preventDefault();
              applyTermAnswer(event.clipboardData.getData("text/plain"));
            }}
            placeholder='용어 답변 붙여넣기: {"Ammo": "탄약", ...}'
          />
        </div>
      )}

      {rows && rows.length === 0 && <p className="term-candidates-message">새로 추가할 용어 후보가 없습니다.</p>}
      {rows && rows.length > 0 && (
        <div className="term-candidate-list">
          {visibleRows.length === 0 && <p className="term-candidates-message">번역어가 빈 후보가 없습니다.</p>}
          {visibleRows.map((row) => {
            const split = row.suggestions.length > 1;
            return (
              <div className={row.checked ? "term-candidate checked" : "term-candidate"} key={row.source}>
                <input
                  type="checkbox"
                  aria-label={`${row.source} 선택`}
                  checked={row.checked}
                  onChange={(event) => update(row.source, { checked: event.target.checked })}
                />
                <div className="term-candidate-source" title={row.example ? `예문: ${row.example}` : undefined}>
                  <strong>{row.source}</strong>
                  <small>
                    {row.count}회 · {row.kinds.map((kind) => KIND_LABELS[kind] ?? kind).join("/")}
                  </small>
                </div>
                <input
                  aria-label={`${row.source} 번역어`}
                  value={row.translation}
                  placeholder="번역어"
                  onChange={(event) => update(row.source, { translation: event.target.value, checked: true })}
                />
                <div className="term-candidate-notes">
                  {split && (
                    <small className="warn">
                      번역 갈림: {row.suggestions.slice(0, 3).map((item) => `${item.text} ${item.count}`).join(" / ")}
                    </small>
                  )}
                  {row.shares_translation_with.length > 0 && (
                    <small className="warn">같은 번역: {row.shares_translation_with.slice(0, 3).join(", ")}</small>
                  )}
                  {!split && row.suggestions.length === 1 && <small>기존 번역 {row.suggestions[0].count}회</small>}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function previewCandidates(): TermCandidate[] {
  return [
    { source: "Ammo", count: 28, kinds: ["highlight"], suggestions: [{ text: "탄약", count: 28 }], shares_translation_with: ["Ammunition"], example: "Gain 2 [gold]Ammo[/gold]." },
    { source: "Ammunition", count: 20, kinds: ["highlight", "title"], suggestions: [{ text: "탄약", count: 13 }], shares_translation_with: ["Ammo"], example: "Spend [gold]Ammunition[/gold]." },
    { source: "Intimidation Aura", count: 7, kinds: ["highlight", "title"], suggestions: [{ text: "위압감", count: 3 }, { text: "위압 오라", count: 2 }], shares_translation_with: [], example: "Gain [gold]Intimidation Aura[/gold]." },
    { source: "Koyuki", count: 11, kinds: ["highlight", "capitalized"], suggestions: [], shares_translation_with: [], example: "Koyuki laughs." },
    { source: "Consumed", count: 10, kinds: ["capitalized"], suggestions: [], shares_translation_with: [], example: "Consumed cards are lost." },
  ];
}
