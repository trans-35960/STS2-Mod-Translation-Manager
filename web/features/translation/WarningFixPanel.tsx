import React from "react";
import { Check, CircleCheck, Copy, Loader2, RefreshCw, Sparkles } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { AiPromptParts, JsonTranslationSheet, JsonValidation, TranslationProjectInfo } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import {
  budgetOf,
  fixableValidationIssues,
  loadPromptSettings,
  readStoredMaxChars,
  slotKey,
  validationIssuesByKey,
} from "./aiPromptShared";
import {
  parsePastedTranslationJson,
  structuredTranslationEntries,
  translationProjectKey,
  translationSlotEntries,
} from "./translationUtils";

const EXCLUDED_KINDS_STORAGE_KEY = "sts2.warningFix.excludedKinds.v1";
/** Term warnings are often false positives, so they start unchecked. */
const DEFAULT_EXCLUDED_KINDS = ["glossary"];

/**
 * Fix prompts built right in the warning list: the flagged entries (of the
 * checked problem kinds) become copyable parts, and a pasted answer goes to the paste
 * review list instead of overwriting, so each change is compared before it
 * is kept.
 */
export function WarningFixPanel({
  sheet,
  projectInfo,
  validation,
  focusKind,
  kindLabel,
  pendingKeys,
  onPasteAnswer,
}: {
  sheet: JsonTranslationSheet;
  projectInfo: TranslationProjectInfo | null;
  validation: JsonValidation;
  /** Kind picked in the warning list; picking a fixable one checks only it. */
  focusKind: string | null;
  kindLabel: (kind: string) => string;
  /** Entries with an answer waiting in the review list. */
  pendingKeys: Set<string>;
  onPasteAnswer: (text: string) => boolean;
}) {
  const projectKey = translationProjectKey(projectInfo);
  // Entries flagged per problem kind, in first-seen order.
  const kindCounts = React.useMemo(() => {
    const keysByKind = new Map<string, Set<string>>();
    for (const issue of fixableValidationIssues(validation)) {
      keysByKind.set(issue.kind, (keysByKind.get(issue.kind) ?? new Set()).add(issue.key));
    }
    return [...keysByKind.entries()].map(([kind, keys]) => ({ kind, count: keys.size }));
  }, [validation]);
  const [excludedKinds, setExcludedKinds] = React.useState<Set<string>>(readExcludedKinds);
  React.useEffect(() => {
    if (focusKind && kindCounts.some((item) => item.kind === focusKind)) {
      setExcludedKinds(new Set(kindCounts.map((item) => item.kind).filter((kind) => kind !== focusKind)));
    }
    // Only a new pick in the warning list narrows the selection.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusKind]);
  const selectedKinds = React.useMemo(
    () => new Set(kindCounts.map((item) => item.kind).filter((kind) => !excludedKinds.has(kind))),
    [excludedKinds, kindCounts],
  );
  const selectionSignature = [...selectedKinds].sort().join(",");
  const issues = React.useMemo(() => validationIssuesByKey(validation, selectedKinds), [validation, selectedKinds]);
  const issueSignature = React.useMemo(() => Object.keys(issues).sort().join("\n"), [issues]);
  const issueCount = Object.keys(issues).length;
  const [result, setResult] = React.useState<AiPromptParts | null>(null);
  const [builtSignature, setBuiltSignature] = React.useState("");
  const [copiedParts, setCopiedParts] = React.useState<Set<number>>(() => new Set());
  const [selectedPart, setSelectedPart] = React.useState<number | null>(null);
  const [report, setReport] = React.useState<{ message: string; tone: "good" | "warn" } | null>(null);
  const [error, setError] = React.useState("");
  const [building, setBuilding] = React.useState(false);

  const entryKeyBySlot = React.useMemo(
    () => new Map(translationSlotEntries(sheet).map((slot) => [slotKey(slot.compactFile, slot.id), slot.entry.key])),
    [sheet],
  );
  const allIssueKeys = React.useMemo(() => new Set(Object.keys(validationIssuesByKey(validation))), [validation]);

  const buildRequestRef = React.useRef(0);

  async function build() {
    const request = ++buildRequestRef.current;
    if (issueCount === 0) {
      setResult(null);
      setBuilding(false);
      return;
    }
    setBuilding(true);
    setError("");
    try {
      const settings = await loadPromptSettings(projectKey);
      const budget = budgetOf(settings.max_chars ?? readStoredMaxChars());
      const parts = isPreviewRuntime()
        ? previewFixParts(sheet, issues, budget)
        : await invokeCommand("build_ai_prompt_parts", {
            sheet,
            request: {
              projectKey: projectKey || null,
              modName: projectInfo?.modName ?? "",
              modDescription: settings.mod_description || projectInfo?.description || "",
              tone: settings.tone || "official",
              extraInstructions: settings.extra_instructions ?? "",
              includePrompt: true,
              maxChars: budget,
              onlyEmpty: false,
              focus: "warnings",
              issues,
            },
          });
      if (request !== buildRequestRef.current) {
        return;
      }
      setResult(parts);
      setBuiltSignature(issueSignature);
      setCopiedParts(new Set());
      setSelectedPart(null);
      setReport(null);
    } catch (reason) {
      if (request === buildRequestRef.current) {
        setError(String(reason));
      }
    } finally {
      if (request === buildRequestRef.current) {
        setBuilding(false);
      }
    }
  }

  // Built when opened and whenever the checked kinds change. Warnings that
  // change while answers come in only offer a rebuild, so the parts being
  // copied keep their numbers.
  React.useEffect(() => {
    void build();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectionSignature]);

  function toggleKind(kind: string) {
    setExcludedKinds((current) => {
      const next = new Set(current);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      try {
        window.localStorage.setItem(EXCLUDED_KINDS_STORAGE_KEY, JSON.stringify([...next]));
      } catch {
        // Remembering the choice is only a convenience.
      }
      return next;
    });
  }

  async function copyPart(index: number) {
    const part = result?.parts[index];
    if (!part) {
      return;
    }
    setSelectedPart(index);
    try {
      if (!navigator.clipboard) {
        throw new Error("clipboard unavailable");
      }
      await navigator.clipboard.writeText(part.text);
      setCopiedParts((current) => new Set(current).add(index));
      setError("");
    } catch {
      setError("클립보드에 쓰지 못했습니다. 아래 미리보기를 선택해 Ctrl+C로 복사하세요.");
    }
  }

  function applyAnswer(text: string) {
    const parsed = parsePastedTranslationJson(text);
    const answered = parsed === null ? [] : structuredTranslationEntries(parsed).filter((entry) => entry.id && entry.translated_value?.trim());
    if (answered.length === 0) {
      setReport({ message: "붙여넣은 내용에서 번역 JSON(k001-ab 형식 id)을 찾지 못했습니다.", tone: "warn" });
      return;
    }
    onPasteAnswer(text);
    setReport({
      message: `답변 ${answered.length}개를 받았습니다. 바뀐 번역은 아래 "붙여넣기 검토"에서 비교한 뒤 허가하세요.`,
      tone: "good",
    });
  }

  /** A part's entries that have an answer waiting or no longer have a warning. */
  function partProgress(part: AiPromptParts["parts"][number]) {
    let pending = 0;
    let fixed = 0;
    for (const slot of part.slots) {
      const key = entryKeyBySlot.get(slotKey(slot.file, slot.id));
      if (!key) continue;
      if (pendingKeys.has(key)) pending += 1;
      else if (!allIssueKeys.has(key)) fixed += 1;
    }
    return { pending, fixed };
  }

  const stale = result !== null && builtSignature !== issueSignature;
  const preview = selectedPart === null ? null : result?.parts[selectedPart];
  return (
    <div className="warning-fix-panel" aria-label="경고 AI 수정">
      <div className="warning-fix-head">
        <strong>
          <Sparkles size={13} aria-hidden="true" />
          AI 수정 · {issueCount}개
        </strong>
        <span>파트를 복사해 AI에 보내고, 답변을 여기에 붙여넣으면 아래 검토 목록에서 비교합니다.</span>
        <button type="button" className="icon-button-text compact" disabled={building || issueCount === 0} onClick={() => void build()}>
          {building ? <Loader2 className="spin-icon" size={13} /> : <RefreshCw size={13} />}
          {stale ? "남은 경고로 다시 만들기" : "다시 만들기"}
        </button>
      </div>
      <div className="warning-fix-kinds" role="group" aria-label="보낼 문제 종류">
        {kindCounts.map((item) => (
          <label key={item.kind} className={selectedKinds.has(item.kind) ? "active" : ""}>
            <input type="checkbox" checked={selectedKinds.has(item.kind)} onChange={() => toggleKind(item.kind)} />
            {kindLabel(item.kind)} <b>{item.count}</b>
          </label>
        ))}
      </div>
      {issueCount === 0 && <p className="warning-fix-note">보낼 문제 종류를 하나 이상 고르세요.</p>}
      {stale && <p className="warning-fix-note">경고가 바뀌었습니다. 남은 경고만 다시 보내려면 다시 만드세요.</p>}
      {issueCount > 0 && result && result.parts.length > 0 && (
        <div className="warning-fix-parts" role="list">
          {result.parts.map((part, index) => {
            const { pending, fixed } = partProgress(part);
            const done = pending + fixed === part.slots.length;
            return (
              <div
                className={[
                  "warning-fix-part",
                  index === selectedPart ? "selected" : "",
                  copiedParts.has(index) ? "copied" : "",
                  done ? "done" : "",
                ].filter(Boolean).join(" ")}
                key={index}
                role="listitem"
              >
                <button type="button" className="warning-fix-part-label" onClick={() => setSelectedPart(index === selectedPart ? null : index)}>
                  <b>
                    파트 {index + 1}/{result.parts.length}
                    {done && <CircleCheck size={12} aria-label="완료" />}
                  </b>
                  <small>
                    {part.slots.length}개{pending > 0 ? ` · 검토 ${pending}` : ""}{fixed > 0 ? ` · 해결 ${fixed}` : ""}
                  </small>
                </button>
                <button
                  type="button"
                  className="toolbar-icon-button"
                  aria-label={`파트 ${index + 1} 복사`}
                  data-tooltip={copiedParts.has(index) ? "복사함" : "복사"}
                  onClick={() => void copyPart(index)}
                >
                  {copiedParts.has(index) ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </div>
            );
          })}
          <textarea
            className="warning-fix-answer"
            data-own-paste
            rows={1}
            value=""
            onChange={() => undefined}
            onPaste={(event) => {
              event.preventDefault();
              applyAnswer(event.clipboardData.getData("text/plain"));
            }}
            placeholder="AI 답변 붙여넣기 (Ctrl+V)"
            aria-label="AI 수정 답변 붙여넣기"
          />
        </div>
      )}
      {result && result.parts.length === 0 && <p className="warning-fix-note">보낼 항목이 없습니다.</p>}
      {report && <p className={`warning-fix-note ${report.tone}`}>{report.message}</p>}
      {error && <p className="warning-fix-note warn">{error}</p>}
      {preview && <textarea className="warning-fix-preview" readOnly value={preview.text} aria-label="선택한 파트 미리보기" />}
    </div>
  );
}

function readExcludedKinds(): Set<string> {
  try {
    const stored = window.localStorage.getItem(EXCLUDED_KINDS_STORAGE_KEY);
    const parsed: unknown = stored ? JSON.parse(stored) : DEFAULT_EXCLUDED_KINDS;
    return new Set(Array.isArray(parsed) ? parsed.filter((kind): kind is string => typeof kind === "string") : DEFAULT_EXCLUDED_KINDS);
  } catch {
    return new Set(DEFAULT_EXCLUDED_KINDS);
  }
}

function previewFixParts(sheet: JsonTranslationSheet, issues: Record<string, string[]>, budget: number): AiPromptParts {
  const slots = translationSlotEntries(sheet).filter((slot) => issues[slot.entry.key]);
  const size = Math.max(1, Math.ceil(slots.length / 2));
  const parts = [slots.slice(0, size), slots.slice(size)].filter((group) => group.length > 0).map((group) => {
    const data: Record<string, Record<string, string>> = {};
    for (const slot of group) {
      (data[slot.compactFile] ??= {})[slot.id] = slot.entry.source_value;
    }
    const text = `너는 Slay the Spire 2 모드 전문 번역가다.\n\n## 고칠 JSON\n\`\`\`json\n${JSON.stringify(data, null, 2)}\n\`\`\``;
    return {
      text,
      rows: group.length,
      chars: Math.min(budget, text.length),
      terms: 0,
      oversized: false,
      slots: group.map((slot) => ({ file: slot.compactFile, id: slot.id })),
    };
  });
  return { total_rows: slots.length, parts };
}
