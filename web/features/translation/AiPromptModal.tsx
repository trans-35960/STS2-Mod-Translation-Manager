import React from "react";
import { Check, CircleCheck, Copy, FileDown, FileText, Files, Loader2, Sparkles, X } from "lucide-react";
import { invokeCommand, saveDialog } from "../../api/tauri";
import type { AiPromptParts, GlossaryTerm, JsonTranslationSheet, ProjectTranslationSettings, TranslationProjectInfo } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { TermCandidatesPanel } from "./TermCandidatesPanel";
import { TermTableEditor } from "./TermTableEditor";
import {
  compactTranslationFile,
  normalizeSlotId,
  parsePastedTranslationJson,
  structuredTranslationEntries,
  translationProjectKey,
  translationSlotEntries,
} from "./translationUtils";

const MAX_CHARS_STORAGE_KEY = "sts2.aiPrompt.maxChars.v1";
const DEFAULT_MAX_CHARS = 5000;
const AUTOSAVE_DELAY_MS = 600;

const TONE_OPTIONS = [
  { value: "official", label: "게임 공식 번역체 (합니다체)" },
  { value: "haeyo", label: "부드러운 해요체" },
  { value: "casual", label: "대사는 반말·구어체, 설명은 합니다체" },
  { value: "source", label: "원문 어조 최대한 유지" },
];

type PasteReport = { partIndex: number | null; message: string; tone: "good" | "warn" };

/**
 * Builds copy-paste prompts for a web AI chat and takes the answers back:
 * pasting an answer here applies it to the sheet and shows which part it
 * covered and which ids it skipped. Everything typed here is remembered per
 * mod and reused next time.
 */
export function AiPromptModal({
  sheet,
  projectInfo,
  onlyEmptyDefault,
  onClose,
  onTermsSaved,
  onPasteAnswer,
}: {
  sheet: JsonTranslationSheet;
  projectInfo: TranslationProjectInfo | null;
  onlyEmptyDefault: boolean;
  onClose: () => void;
  onTermsSaved: () => void;
  onPasteAnswer: (text: string) => boolean;
}) {
  const projectKey = translationProjectKey(projectInfo);
  const [loaded, setLoaded] = React.useState(false);
  const [description, setDescription] = React.useState("");
  const [tone, setTone] = React.useState("official");
  const [terms, setTerms] = React.useState<GlossaryTerm[]>([]);
  const [extraInstructions, setExtraInstructions] = React.useState("");
  const [includePrompt, setIncludePrompt] = React.useState(true);
  const [onlyEmpty, setOnlyEmpty] = React.useState(onlyEmptyDefault);
  const [maxChars, setMaxChars] = React.useState(readStoredMaxChars);
  const [saveState, setSaveState] = React.useState<"" | "saving" | "saved">("");
  const savedTermsRef = React.useRef("");
  const [result, setResult] = React.useState<AiPromptParts | null>(null);
  const [selectedPart, setSelectedPart] = React.useState(0);
  const [copiedParts, setCopiedParts] = React.useState<Set<number>>(() => new Set());
  const [pasteReport, setPasteReport] = React.useState<PasteReport | null>(null);
  const [exportMessage, setExportMessage] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [tab, setTab] = React.useState<"settings" | "terms" | "parts">("settings");
  const [error, setError] = React.useState("");
  const previewRef = React.useRef<HTMLTextAreaElement>(null);

  React.useEffect(() => {
    let cancelled = false;
    setLoaded(false);
    loadSettings(projectKey)
      .then((settings) => {
        if (cancelled) {
          return;
        }
        setDescription(settings.mod_description || projectInfo?.description || "");
        setTone(settings.tone || "official");
        setTerms(settings.terms);
        savedTermsRef.current = JSON.stringify(settings.terms);
        setExtraInstructions(settings.extra_instructions ?? "");
        if (settings.include_prompt != null) setIncludePrompt(settings.include_prompt);
        if (settings.only_empty != null) setOnlyEmpty(settings.only_empty);
        if (settings.max_chars != null && settings.max_chars >= 500) setMaxChars(settings.max_chars);
        setTab(settings.terms.length > 0 ? "parts" : settings.mod_description || settings.extra_instructions ? "terms" : "settings");
        setLoaded(true);
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [projectKey, projectInfo?.description]);

  const settings = React.useMemo(
    (): ProjectTranslationSettings => ({
      mod_description: description.trim(),
      tone,
      terms,
      extra_instructions: extraInstructions.trim(),
      include_prompt: includePrompt,
      only_empty: onlyEmpty,
      max_chars: budgetOf(maxChars),
    }),
    [description, extraInstructions, includePrompt, maxChars, onlyEmpty, terms, tone],
  );

  // Remember everything per mod as it is typed.
  React.useEffect(() => {
    if (!loaded || !projectKey || isPreviewRuntime()) {
      return;
    }
    setSaveState("saving");
    const timer = window.setTimeout(() => {
      invokeCommand("save_project_translation_settings", { projectKey, settings })
        .then(() => {
          setSaveState("saved");
          const savedTerms = JSON.stringify(terms);
          if (savedTerms !== savedTermsRef.current) {
            savedTermsRef.current = savedTerms;
            onTermsSaved();
          }
        })
        .catch((reason) => {
          setSaveState("");
          setError(String(reason));
        });
    }, AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
    // onTermsSaved is a fresh closure every render; only settings changes matter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loaded, projectKey, settings]);

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  // Live progress: how many of each part's entries the sheet has translated.
  const translatedSlots = React.useMemo(() => {
    const done = new Set<string>();
    for (const slot of translationSlotEntries(sheet)) {
      if (slot.entry.translated_value.trim()) {
        done.add(slotKey(slot.compactFile, slot.id));
      }
    }
    return done;
  }, [sheet]);
  const partProgress = React.useMemo(
    () => result?.parts.map((part) => part.slots.filter((slot) => translatedSlots.has(slotKey(slot.file, slot.id))).length) ?? [],
    [result, translatedSlots],
  );

  const registeredSources = React.useMemo(
    () => new Set(settings.terms.map((term) => term.source.toLowerCase())),
    [settings.terms],
  );

  /** Adds terms to the mod glossary, replacing entries for the same source. */
  function addTerms(added: Array<{ source: string; target: string }>) {
    setTerms((current) => {
      const sources = new Set(added.map((term) => term.source.toLowerCase()));
      return [
        ...current.filter((term) => !sources.has(term.source.toLowerCase())),
        ...added.map((term) => ({
          source: term.source,
          targets: term.target.split("/").map((target) => target.trim()).filter(Boolean),
        })),
      ];
    });
  }

  async function buildParts() {
    setBusy(true);
    setError("");
    try {
      const budget = budgetOf(maxChars);
      try {
        window.localStorage.setItem(MAX_CHARS_STORAGE_KEY, String(budget));
      } catch {
        // Remembering the budget is only a convenience.
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
              extraInstructions: settings.extra_instructions,
              includePrompt,
              maxChars: budget,
              onlyEmpty,
            },
          });
      setResult(parts);
      setSelectedPart(0);
      setCopiedParts(new Set());
      setPasteReport(null);
      setExportMessage("");
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

  async function exportParts(format: "txt" | "txt_parts" | "pdf") {
    if (!result || result.parts.length === 0) {
      return;
    }
    const extension = format === "pdf" ? "pdf" : "txt";
    const base = `${projectInfo?.modName || "translation"}.${sheet.target_language || "kor"}.prompt`;
    setError("");
    setExportMessage("");
    try {
      const outputPath = isPreviewRuntime()
        ? `translation_work/${base}.${extension}`
        : await saveDialog({
            title: format === "txt" ? "프롬프트 파트를 한 파일로 내보내기" : "프롬프트 파트별 파일 내보내기 (이름 뒤에 .partNN이 붙습니다)",
            defaultPath: `${base}.${extension}`,
            filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
          });
      if (!outputPath || Array.isArray(outputPath)) {
        return;
      }
      setBusy(true);
      const written = isPreviewRuntime()
        ? format === "txt"
          ? [outputPath]
          : result.parts.map((_, index) => outputPath.replace(/\.(txt|pdf)$/i, `.part${String(index + 1).padStart(2, "0")}.$1`))
        : await invokeCommand("export_ai_prompt_parts", {
            outputPath,
            parts: result.parts.map((part) => part.text),
            format,
          });
      setExportMessage(
        written.length === 1
          ? `내보냈습니다: ${written[0]}`
          : `파일 ${written.length}개를 내보냈습니다: ${written[0]} … ${fileName(written[written.length - 1])}`,
      );
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function applyAnswer(text: string) {
    const parsed = parsePastedTranslationJson(text);
    const answered = parsed === null ? [] : structuredTranslationEntries(parsed).filter((entry) => entry.id && entry.translated_value?.trim());
    if (answered.length === 0) {
      setPasteReport({ partIndex: null, message: "붙여넣은 내용에서 번역 JSON(k001-ab 형식 id)을 찾지 못했습니다.", tone: "warn" });
      return;
    }
    const answeredKeys = new Set(
      answered.map((entry) => slotKey(entry.source ? compactTranslationFile(entry.source) : "", entry.id ?? "")),
    );
    const answeredIds = new Set(answered.map((entry) => normalizeSlotId(entry.id ?? "") ?? entry.id ?? ""));
    const covers = (slot: { file: string; id: string }) =>
      answeredKeys.has(slotKey(slot.file, slot.id)) || answeredKeys.has(slotKey("", slot.id)) ||
      (!answered.some((entry) => entry.source) && answeredIds.has(normalizeSlotId(slot.id) ?? slot.id));

    // The part this answer belongs to is the one it overlaps most.
    let partIndex: number | null = null;
    let bestOverlap = 0;
    result?.parts.forEach((part, index) => {
      const overlap = part.slots.filter(covers).length;
      if (overlap > bestOverlap) {
        bestOverlap = overlap;
        partIndex = index;
      }
    });

    onPasteAnswer(text);

    if (partIndex === null || !result) {
      setPasteReport({ partIndex: null, message: `답변 ${answered.length}개를 시트에 붙여넣었습니다. (자세한 결과는 로그)`, tone: "good" });
      return;
    }
    const part = result.parts[partIndex];
    const missing = part.slots.filter((slot) => !covers(slot)).map((slot) => slot.id);
    setSelectedPart(partIndex);
    setPasteReport({
      partIndex,
      tone: missing.length === 0 ? "good" : "warn",
      message:
        `파트 ${partIndex + 1}/${result.parts.length} 답변: ${part.slots.length - missing.length}/${part.slots.length}개` +
        (missing.length > 0
          ? ` · 빠진 id ${missing.length}개: ${missing.slice(0, 8).join(", ")}${missing.length > 8 ? " ..." : ""}`
          : " 모두 받음") +
        ". 기존 번역과 다른 값은 시트의 충돌 후보로 들어갑니다.",
    });
  }

  const preview = result?.parts[selectedPart];
  const untranslatedTerms = terms.filter((term) => term.targets.length === 0).length;
  const doneParts = partProgress.filter((count, index) => result && count === result.parts[index].slots.length).length;
  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section className="modal ai-prompt-modal" role="dialog" aria-modal="true" aria-label="AI 프롬프트 만들기">
        <header>
          <h2>AI 프롬프트 만들기</h2>
          <p>
            ① 모드 정보를 정하고 ② 용어를 먼저 확정한 뒤 ③ 본문을 파트별로 번역하세요.
            {projectKey && (
              <span className="ai-prompt-save-state">
                {saveState === "saving" ? " · 저장 중" : saveState === "saved" ? " · 이 모드 설정 저장됨" : " · 입력 내용은 이 모드에 자동 저장"}
              </span>
            )}
          </p>
        </header>
        <div className="ai-prompt-tabs" role="tablist">
          {(
            [
              ["settings", "① 설정", description.trim() ? "배경 있음" : "배경 없음"],
              ["terms", "② 용어", `모드 용어 ${settings.terms.length}개${untranslatedTerms > 0 ? ` · 빈 번역 ${untranslatedTerms}` : ""}`],
              ["parts", "③ 본문 파트", result ? `완료 ${doneParts}/${result.parts.length}` : "파트 만들기 전"],
            ] as const
          ).map(([value, label, status]) => (
            <button
              type="button"
              role="tab"
              aria-selected={tab === value}
              className={tab === value ? "active" : ""}
              key={value}
              onClick={() => setTab(value)}
            >
              <strong>{label}</strong>
              <small>{status}</small>
            </button>
          ))}
        </div>

        <div className="ai-prompt-tab" role="tabpanel" hidden={tab !== "settings"}>
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
            <span>추가 지시 (한 줄에 하나, 모든 파트에 들어감)</span>
            <textarea
              rows={2}
              value={extraInstructions}
              onChange={(event) => setExtraInstructions(event.target.value)}
              placeholder={"기술 이름은 처음 나올 때 원문을 괄호로 병기\n캐릭터 이름은 음역"}
            />
          </label>
        </div>
        <div className="ai-prompt-tab" role="tabpanel" hidden={tab !== "terms"}>
          <p className="ai-prompt-tab-hint">후보를 뽑아 번역어를 정하고 "용어에 추가"하면, 본문 파트마다 용어집으로 들어가고 검증에도 쓰입니다.</p>
          <div className="modal-field">
            <span>이 모드 전용 용어 {terms.length}개 (본문 파트와 검증에 사용 · 왼쪽 용어집에도 표시)</span>
            <TermTableEditor terms={terms} onChange={setTerms} keepUntranslated emptyText="아직 모드 용어가 없습니다." />
          </div>
          <TermCandidatesPanel
            sheet={sheet}
            projectKey={projectKey}
            context={{
              modName: projectInfo?.modName ?? "",
              modDescription: settings.mod_description,
              tone,
              extraInstructions: settings.extra_instructions,
              maxChars: budgetOf(maxChars),
            }}
            registeredSources={registeredSources}
            onAddTerms={addTerms}
          />
        </div>
        <div className="ai-prompt-tab" role="tabpanel" hidden={tab !== "parts"}>
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
              <input type="number" min={500} step={500} value={maxChars} onChange={(event) => setMaxChars(Number(event.target.value))} />
              <span>자</span>
            </label>
            <button type="button" className="primary icon-button-text compact" onClick={() => void buildParts()} disabled={busy}>
              {busy ? <Loader2 className="spin-icon" size={15} /> : <Sparkles size={15} />}
              파트 만들기
            </button>
          </div>
          {result && (
            <div className="ai-prompt-result">
              {result.parts.length === 0 ? (
                <p className="inline-warning">내보낼 항목이 없습니다.</p>
              ) : (
                <>
                  <div className="ai-prompt-parts" role="list">
                    {result.parts.map((part, index) => {
                      const translated = partProgress[index] ?? 0;
                      const done = translated === part.slots.length;
                      return (
                        <div
                          className={[
                            "ai-prompt-part",
                            index === selectedPart ? "selected" : "",
                            done ? "done" : translated > 0 ? "partial" : "",
                            copiedParts.has(index) ? "copied" : "",
                          ].filter(Boolean).join(" ")}
                          key={index}
                          role="listitem"
                        >
                          <button type="button" className="ai-prompt-part-label" onClick={() => setSelectedPart(index)}>
                            <strong>
                              파트 {index + 1}/{result.parts.length}
                              {done && <CircleCheck size={13} aria-label="완료" />}
                            </strong>
                            <small>
                              번역 {translated}/{part.slots.length} · {part.chars.toLocaleString()}자
                              {part.terms > 0 ? ` · 용어 ${part.terms}` : ""}
                              {part.oversized ? " · 한도 초과" : ""}
                            </small>
                          </button>
                          <button
                            type="button"
                            className="toolbar-icon-button"
                            aria-label={`파트 ${index + 1} 복사`}
                            data-tooltip={copiedParts.has(index) ? "복사함" : "복사"}
                            onClick={() => void copyPart(index)}
                          >
                            {copiedParts.has(index) ? <Check size={14} /> : <Copy size={14} />}
                          </button>
                        </div>
                      );
                    })}
                  </div>
                  <p className="ai-prompt-summary">
                    전체 {result.total_rows.toLocaleString()}행 · 완료 파트 {doneParts}/{result.parts.length} · 복사함 {copiedParts.size}/{result.parts.length}
                  </p>
                  <div className="ai-prompt-export">
                    <span>모두 내보내기</span>
                    <button type="button" className="icon-button-text compact" disabled={busy} onClick={() => void exportParts("txt")}>
                      <FileText size={14} />
                      TXT 한 파일
                    </button>
                    <button type="button" className="icon-button-text compact" disabled={busy} onClick={() => void exportParts("txt_parts")}>
                      <Files size={14} />
                      TXT 파트별
                    </button>
                    <button
                      type="button"
                      className="icon-button-text compact"
                      disabled={busy}
                      title="파트마다 작은 글씨(7pt) PDF를 만듭니다. Gemini처럼 PDF를 페이지 단위로 계산하는 AI에 파일로 올리면 입력 토큰이 줄 수 있습니다."
                      onClick={() => void exportParts("pdf")}
                    >
                      <FileDown size={14} />
                      PDF 파트별
                    </button>
                  </div>
                  {exportMessage && <p className="ai-prompt-paste-report good">{exportMessage}</p>}
                  <label className="modal-field ai-prompt-answer">
                    <span>답변 붙여넣기 (Ctrl+V 하면 바로 시트에 적용)</span>
                    <textarea
                      data-own-paste
                      rows={2}
                      value=""
                      onChange={() => undefined}
                      onPaste={(event) => {
                        event.preventDefault();
                        applyAnswer(event.clipboardData.getData("text/plain"));
                      }}
                      placeholder="AI가 준 ```json ... ``` 답변을 그대로 붙여넣으세요."
                    />
                  </label>
                  {pasteReport && <p className={`ai-prompt-paste-report ${pasteReport.tone}`}>{pasteReport.message}</p>}
                  {preview && <textarea ref={previewRef} className="ai-prompt-preview" readOnly value={preview.text} aria-label="선택한 파트 미리보기" />}
                </>
              )}
            </div>
          )}
        </div>
        {!projectKey && <p className="inline-warning">프로젝트 정보가 없어 이 모드 설정을 저장하지 못합니다.</p>}
        {error && <p className="inline-warning">{error}</p>}

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

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function slotKey(file: string, id: string) {
  return `${file}\u0000${normalizeSlotId(id) ?? id}`;
}

function budgetOf(value: number) {
  return Math.max(500, Math.round(value) || DEFAULT_MAX_CHARS);
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
    return { mod_description: "", tone: "official", terms: [], extra_instructions: "" };
  }
  return invokeCommand("load_project_translation_settings", { projectKey });
}

function previewParts(sheet: JsonTranslationSheet, includePrompt: boolean, budget: number): AiPromptParts {
  const slots = translationSlotEntries(sheet);
  const size = Math.max(1, Math.ceil(slots.length / 2));
  const parts = [slots.slice(0, size), slots.slice(size)].filter((group) => group.length > 0).map((group) => {
    const data: Record<string, Record<string, string>> = {};
    for (const slot of group) {
      (data[slot.compactFile] ??= {})[slot.id] = slot.entry.source_value;
    }
    const json = JSON.stringify(data, null, 2);
    const text = includePrompt ? `너는 Slay the Spire 2 모드 전문 번역가다.\n\n## 번역할 JSON\n\`\`\`json\n${json}\n\`\`\`` : json;
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
