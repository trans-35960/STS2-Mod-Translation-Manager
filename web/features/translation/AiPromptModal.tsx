import React from "react";
import { Check, Copy, Loader2, Sparkles, X } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { AiPromptParts, JsonTranslationSheet, ProjectTranslationSettings, TranslationProjectInfo } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { translationProjectKey } from "./translationUtils";

const MAX_CHARS_STORAGE_KEY = "sts2.aiPrompt.maxChars.v1";
const DEFAULT_MAX_CHARS = 5000;

const TONE_OPTIONS = [
  { value: "official", label: "게임 공식 번역체 (합니다체)" },
  { value: "haeyo", label: "부드러운 해요체" },
  { value: "casual", label: "대사는 반말·구어체, 설명은 합니다체" },
  { value: "source", label: "원문 어조 최대한 유지" },
];

/**
 * Builds copy-paste prompts for a web AI chat: instructions, mod background,
 * the glossary terms each part needs, and the data, split to a character
 * budget. Answers are pasted back into the sheet, which matches them by id.
 */
export function AiPromptModal({
  sheet,
  projectInfo,
  onlyEmptyDefault,
  onClose,
  onTermsSaved,
}: {
  sheet: JsonTranslationSheet;
  projectInfo: TranslationProjectInfo | null;
  onlyEmptyDefault: boolean;
  onClose: () => void;
  onTermsSaved: () => void;
}) {
  const projectKey = translationProjectKey(projectInfo);
  const [description, setDescription] = React.useState("");
  const [tone, setTone] = React.useState("official");
  const [termsText, setTermsText] = React.useState("");
  const [savedTermsText, setSavedTermsText] = React.useState("");
  const [includePrompt, setIncludePrompt] = React.useState(true);
  const [onlyEmpty, setOnlyEmpty] = React.useState(onlyEmptyDefault);
  const [maxChars, setMaxChars] = React.useState(readStoredMaxChars);
  const [result, setResult] = React.useState<AiPromptParts | null>(null);
  const [selectedPart, setSelectedPart] = React.useState(0);
  const [copiedParts, setCopiedParts] = React.useState<Set<number>>(() => new Set());
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const previewRef = React.useRef<HTMLTextAreaElement>(null);

  React.useEffect(() => {
    let cancelled = false;
    loadSettings(projectKey)
      .then((settings) => {
        if (cancelled) {
          return;
        }
        const text = formatTerms(settings.terms);
        setDescription(settings.mod_description || projectInfo?.description || "");
        setTone(settings.tone || "official");
        setTermsText(text);
        setSavedTermsText(text);
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [projectKey, projectInfo?.description]);

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  async function buildParts() {
    setBusy(true);
    setError("");
    try {
      const budget = Math.max(500, Math.round(maxChars) || DEFAULT_MAX_CHARS);
      try {
        window.localStorage.setItem(MAX_CHARS_STORAGE_KEY, String(budget));
      } catch {
        // Remembering the budget is only a convenience.
      }
      const settings: ProjectTranslationSettings = { mod_description: description.trim(), tone, terms: parseTerms(termsText) };
      if (projectKey && !isPreviewRuntime()) {
        await invokeCommand("save_project_translation_settings", { projectKey, settings });
      }
      const parts = isPreviewRuntime()
        ? previewParts(sheet, includePrompt, budget)
        : await invokeCommand("build_ai_prompt_parts", {
            sheet,
            request: {
              projectKey: projectKey || null,
              modName: projectInfo?.modName ?? "",
              modDescription: settings.mod_description,
              tone,
              includePrompt,
              maxChars: budget,
              onlyEmpty,
            },
          });
      setResult(parts);
      setSelectedPart(0);
      setCopiedParts(new Set());
      if (termsText !== savedTermsText) {
        setSavedTermsText(termsText);
        onTermsSaved();
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
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
      // Fall back to a selected preview the user can copy by hand.
      window.setTimeout(() => {
        previewRef.current?.focus();
        previewRef.current?.select();
      });
      setError("클립보드에 쓰지 못했습니다. 미리보기 내용을 선택해 두었으니 Ctrl+C로 복사하세요.");
    }
  }

  const preview = result?.parts[selectedPart];
  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section className="modal ai-prompt-modal" role="dialog" aria-modal="true" aria-label="AI 프롬프트 만들기">
        <header>
          <h2>AI 프롬프트 만들기</h2>
          <p>웹 AI 채팅에 파트별로 붙여넣고, 받은 JSON 답변을 시트의 번역 칸에 붙여넣으면 ID로 매칭됩니다.</p>
        </header>

        <label className="modal-field">
          <span>모드 배경 (컨셉, 캐릭터, 세계관, 주의할 표현)</span>
          <textarea
            rows={3}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="예: Fate 시리즈 크로스오버. 아르토리아는 기사왕으로 위엄 있는 말투."
          />
        </label>
        <label className="modal-field">
          <span>어조</span>
          <select value={tone} onChange={(event) => setTone(event.target.value)}>
            {TONE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label className="modal-field">
          <span>이 모드 전용 용어 (한 줄에 `원문 = 번역`, 여러 번역은 `/`로 구분 · 검증에도 사용)</span>
          <textarea
            rows={4}
            value={termsText}
            onChange={(event) => setTermsText(event.target.value)}
            placeholder={"Artoria = 아르토리아\nNoble Phantasm = 보구"}
          />
        </label>

        <div className="ai-prompt-options">
          <label className="inline-check">
            <input type="checkbox" checked={includePrompt} onChange={(event) => setIncludePrompt(event.target.checked)} />
            <span>프롬프트 포함</span>
          </label>
          <label className="inline-check">
            <input type="checkbox" checked={onlyEmpty} onChange={(event) => setOnlyEmpty(event.target.checked)} />
            <span>빈 값만</span>
          </label>
          <label className="ai-prompt-max">
            <span>파트당 최대</span>
            <input
              type="number"
              min={500}
              step={500}
              value={maxChars}
              onChange={(event) => setMaxChars(Number(event.target.value))}
            />
            <span>자</span>
          </label>
          <button type="button" className="primary icon-button-text compact" onClick={() => void buildParts()} disabled={busy}>
            {busy ? <Loader2 className="spin-icon" size={15} /> : <Sparkles size={15} />}
            파트 만들기
          </button>
        </div>
        {!projectKey && <p className="inline-warning">프로젝트 정보가 없어 배경과 모드 용어를 저장하지 못합니다.</p>}
        {error && <p className="inline-warning">{error}</p>}

        {result && (
          <div className="ai-prompt-result">
            {result.parts.length === 0 ? (
              <p className="inline-warning">내보낼 항목이 없습니다.</p>
            ) : (
              <>
                <div className="ai-prompt-parts" role="list">
                  {result.parts.map((part, index) => (
                    <div
                      className={["ai-prompt-part", index === selectedPart ? "selected" : "", copiedParts.has(index) ? "copied" : ""].filter(Boolean).join(" ")}
                      key={index}
                      role="listitem"
                    >
                      <button type="button" className="ai-prompt-part-label" onClick={() => setSelectedPart(index)}>
                        <strong>
                          파트 {index + 1}/{result.parts.length}
                        </strong>
                        <small>
                          {part.chars.toLocaleString()}자 · {part.rows}행{part.terms > 0 ? ` · 용어 ${part.terms}` : ""}
                          {part.oversized ? " · 한도 초과(단일 항목)" : ""}
                        </small>
                      </button>
                      <button
                        type="button"
                        className="toolbar-icon-button"
                        aria-label={`파트 ${index + 1} 복사`}
                        data-tooltip="복사"
                        onClick={() => void copyPart(index)}
                      >
                        {copiedParts.has(index) ? <Check size={14} /> : <Copy size={14} />}
                      </button>
                    </div>
                  ))}
                </div>
                {preview && <textarea ref={previewRef} className="ai-prompt-preview" readOnly value={preview.text} aria-label="선택한 파트 미리보기" />}
                <p className="ai-prompt-summary">
                  전체 {result.total_rows.toLocaleString()}행 · {result.parts.length}개 파트 · 복사함 {copiedParts.size}/{result.parts.length}
                </p>
              </>
            )}
          </div>
        )}

        <footer>
          <button type="button" className="icon-button-text compact" onClick={onClose}>
            <X size={15} />
            닫기
          </button>
        </footer>
      </section>
    </div>
  );
}

function readStoredMaxChars() {
  try {
    const stored = Number(window.localStorage.getItem(MAX_CHARS_STORAGE_KEY));
    return Number.isFinite(stored) && stored >= 500 ? stored : DEFAULT_MAX_CHARS;
  } catch {
    return DEFAULT_MAX_CHARS;
  }
}

async function loadSettings(projectKey: string): Promise<ProjectTranslationSettings> {
  if (!projectKey || isPreviewRuntime()) {
    return { mod_description: "", tone: "official", terms: [] };
  }
  return invokeCommand("load_project_translation_settings", { projectKey });
}

function formatTerms(terms: ProjectTranslationSettings["terms"]) {
  return terms.map((term) => `${term.source} = ${term.targets.join(" / ")}`).join("\n");
}

function parseTerms(text: string): ProjectTranslationSettings["terms"] {
  return text
    .split("\n")
    .map((line) => {
      const separator = line.indexOf("=");
      if (separator < 0) {
        return null;
      }
      const source = line.slice(0, separator).trim();
      const targets = line
        .slice(separator + 1)
        .split("/")
        .map((target) => target.trim())
        .filter(Boolean);
      return source && targets.length > 0 ? { source, targets } : null;
    })
    .filter((term): term is ProjectTranslationSettings["terms"][number] => term !== null);
}

function previewParts(sheet: JsonTranslationSheet, includePrompt: boolean, budget: number): AiPromptParts {
  const text = `${includePrompt ? "너는 Slay the Spire 2 모드 전문 번역가다.\n\n## 번역할 JSON\n```json\n" : ""}${JSON.stringify(
    { "cards.json": Object.fromEntries(sheet.entries.slice(0, 3).map((entry, index) => [`k00${index + 1}`, entry.source_value])) },
    null,
    2,
  )}${includePrompt ? "\n```" : ""}`;
  return {
    total_rows: sheet.entries.length,
    parts: [
      { text, rows: 3, chars: text.length, terms: 1, oversized: false },
      { text: `${text}\n(미리보기 파트 2)`, rows: Math.max(0, sheet.entries.length - 3), chars: Math.min(budget, text.length + 12), terms: 0, oversized: false },
    ],
  };
}
