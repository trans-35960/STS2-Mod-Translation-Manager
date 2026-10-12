import React from "react";
import { BookOpen, X } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { GlossaryInfo, GlossaryTerm, ProjectTranslationSettings } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { TermTableEditor } from "./TermTableEditor";

const AUTOSAVE_DELAY_MS = 600;

type Tab = "mod" | "user";
type SaveState = "" | "saving" | "saved" | "error";

/**
 * A short summary of the term lists the validator and AI prompts use, with
 * the lists themselves edited in a dialog: this mod's terms (shared with the
 * AI prompt window) and the user's terms for the target language. Edits are
 * saved as they are made; closing the dialog re-runs validation.
 */
export function GlossaryPanel({
  sourcePath,
  sampleKey,
  targetLanguage,
  projectKey,
  refreshKey,
  disabled,
  onSaved,
}: {
  sourcePath: string;
  /** An entry key of the open sheet, which names the source language. */
  sampleKey: string;
  targetLanguage: string;
  /** Mod whose terms to show; empty when no project is open. */
  projectKey: string;
  /** Changes when the mod terms were edited elsewhere (AI prompt window). */
  refreshKey: number;
  disabled: boolean;
  onSaved: () => void;
}) {
  const [info, setInfo] = React.useState<GlossaryInfo | null>(null);
  const [userTerms, setUserTerms] = React.useState<GlossaryTerm[]>([]);
  const [projectSettings, setProjectSettings] = React.useState<ProjectTranslationSettings | null>(null);
  const [modTerms, setModTerms] = React.useState<GlossaryTerm[]>([]);
  const [error, setError] = React.useState("");
  const [open, setOpen] = React.useState(false);

  React.useEffect(() => {
    if (!targetLanguage.trim()) {
      return;
    }
    let cancelled = false;
    loadGlossary(sourcePath, targetLanguage, sampleKey)
      .then((result) => {
        if (!cancelled) {
          setInfo(result);
          setUserTerms(result.user_terms);
          setError("");
        }
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [sampleKey, sourcePath, targetLanguage]);

  React.useEffect(() => {
    let cancelled = false;
    loadProjectSettings(projectKey)
      .then((settings) => {
        if (!cancelled) {
          setProjectSettings(settings);
          setModTerms(settings?.terms ?? []);
        }
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [projectKey, refreshKey]);

  const untranslated = modTerms.filter((term) => term.targets.length === 0).length;
  return (
    <section className="glossary-card">
      <div className="glossary-card-head">
        <strong>용어집</strong>
        <button
          className="icon-button-text compact"
          type="button"
          onClick={() => setOpen(true)}
          disabled={disabled || !targetLanguage.trim()}
        >
          <BookOpen size={14} />
          편집
        </button>
      </div>
      <p className="glossary-card-counts">
        게임 {info?.game_terms.toLocaleString() ?? "-"}
        {projectKey && (
          <>
            {" · "}모드 {modTerms.length}
            {untranslated > 0 && <em> (빈 번역 {untranslated})</em>}
          </>
        )}
        {" · "}사용자 {userTerms.length}
      </p>
      {info && (
        <p className="glossary-note">
          {info.game_pck_path
            ? info.game_terms > 0
              ? `${info.source_language} → ${info.target_language} 게임 공식 명칭과 비교`
              : `게임에 ${info.source_language} → ${info.target_language} 공식 용어 없음`
            : "게임 PCK를 찾지 못해 모드·사용자 용어만 검사"}
        </p>
      )}
      {error && <p className="inline-warning">{error}</p>}
      {open && (
        <GlossaryModal
          info={info}
          targetLanguage={targetLanguage}
          projectKey={projectKey}
          projectSettings={projectSettings}
          modTerms={modTerms}
          userTerms={userTerms}
          onModTermsSaved={(terms, settings) => {
            setModTerms(terms);
            setProjectSettings(settings);
          }}
          onUserTermsSaved={(terms) => {
            setUserTerms(terms);
            setInfo((current) => (current ? { ...current, user_terms: terms } : current));
          }}
          onClose={(changed) => {
            setOpen(false);
            if (changed) {
              onSaved();
            }
          }}
        />
      )}
    </section>
  );
}

function GlossaryModal({
  info,
  targetLanguage,
  projectKey,
  projectSettings,
  modTerms: initialModTerms,
  userTerms: initialUserTerms,
  onModTermsSaved,
  onUserTermsSaved,
  onClose,
}: {
  info: GlossaryInfo | null;
  targetLanguage: string;
  projectKey: string;
  projectSettings: ProjectTranslationSettings | null;
  modTerms: GlossaryTerm[];
  userTerms: GlossaryTerm[];
  onModTermsSaved: (terms: GlossaryTerm[], settings: ProjectTranslationSettings) => void;
  onUserTermsSaved: (terms: GlossaryTerm[]) => void;
  onClose: (changed: boolean) => void;
}) {
  const [tab, setTab] = React.useState<Tab>(projectKey ? "mod" : "user");
  const [modTerms, setModTerms] = React.useState(initialModTerms);
  const [userTerms, setUserTerms] = React.useState(initialUserTerms);
  const [modDirty, setModDirty] = React.useState(false);
  const [userDirty, setUserDirty] = React.useState(false);
  const [saveState, setSaveState] = React.useState<SaveState>("");
  const [error, setError] = React.useState("");
  const changedRef = React.useRef(false);
  // Latest lists, so a save does not clear the dirty flag of an edit made while it ran.
  const latestRef = React.useRef({ modTerms, userTerms });
  latestRef.current = { modTerms, userTerms };
  const savingRef = React.useRef<Promise<void>>(Promise.resolve());

  const save = React.useCallback(async () => {
    setSaveState("saving");
    try {
      if (modDirty && projectKey && projectSettings) {
        const settings = { ...projectSettings, terms: modTerms };
        if (!isPreviewRuntime()) {
          await invokeCommand("save_project_translation_settings", { projectKey, settings });
        }
        setModDirty(latestRef.current.modTerms !== modTerms);
        onModTermsSaved(modTerms, settings);
      }
      if (userDirty) {
        const saved = isPreviewRuntime() ? userTerms : await invokeCommand("save_user_glossary", { targetLanguage, terms: userTerms });
        setUserDirty(latestRef.current.userTerms !== userTerms);
        onUserTermsSaved(saved);
      }
      changedRef.current = true;
      setSaveState("saved");
      setError("");
    } catch (reason) {
      setSaveState("error");
      setError(String(reason));
    }
  }, [modDirty, modTerms, onModTermsSaved, onUserTermsSaved, projectKey, projectSettings, targetLanguage, userDirty, userTerms]);

  // Save shortly after each edit, one save at a time.
  React.useEffect(() => {
    if (!modDirty && !userDirty) {
      return;
    }
    setSaveState("saving");
    const timer = window.setTimeout(() => {
      savingRef.current = savingRef.current.then(save);
    }, AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [modDirty, userDirty, modTerms, userTerms, save]);

  const close = React.useCallback(async () => {
    if (modDirty || userDirty) {
      await (savingRef.current = savingRef.current.then(save));
    } else {
      await savingRef.current;
    }
    onClose(changedRef.current);
  }, [modDirty, onClose, save, userDirty]);

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => event.key === "Escape" && void close();
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close]);

  const modUntranslated = modTerms.filter((term) => term.targets.length === 0).length;
  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && void close()}>
      <section className="modal glossary-modal" role="dialog" aria-modal="true" aria-label="용어집">
        <header>
          <h2>용어집</h2>
          <p>
            {info?.game_pck_path
              ? info.game_terms > 0
                ? `원문의 [gold] 강조 용어를 게임 공식 ${info.target_language} 명칭 ${info.game_terms.toLocaleString()}개와 비교합니다 (${info.source_language} → ${info.target_language}). `
                : `게임 PCK에 ${info?.source_language} → ${info?.target_language} 공식 용어가 없습니다. `
              : "게임 PCK(SlayTheSpire2.pck)를 찾지 못해 모드·사용자 용어만 검사합니다. "}
            같은 원문이면 모드 용어 → 사용자 용어 → 게임 명칭 순으로 우선합니다.
            <span className="glossary-save-state">
              {saveState === "saving" ? " · 저장 중" : saveState === "saved" ? " · 저장됨" : saveState === "error" ? " · 저장 실패" : " · 입력하면 자동 저장"}
            </span>
          </p>
        </header>
        <div className="ai-prompt-tabs glossary-tabs" role="tablist">
          {projectKey && (
            <button type="button" role="tab" aria-selected={tab === "mod"} className={tab === "mod" ? "active" : ""} onClick={() => setTab("mod")}>
              <strong>이 모드 용어</strong>
              <small>
                {modTerms.length}개{modUntranslated > 0 ? ` · 빈 번역 ${modUntranslated}` : ""} · AI 프롬프트 용어 탭과 같은 목록
              </small>
            </button>
          )}
          <button type="button" role="tab" aria-selected={tab === "user"} className={tab === "user" ? "active" : ""} onClick={() => setTab("user")}>
            <strong>사용자 용어</strong>
            <small>
              {userTerms.length}개 · {targetLanguage}의 모든 모드에 적용
            </small>
          </button>
        </div>
        {tab === "mod" && projectKey ? (
          <TermTableEditor
            key="mod"
            keepUntranslated
            terms={modTerms}
            onChange={(terms) => {
              setModTerms(terms);
              setModDirty(true);
            }}
            emptyText="AI 프롬프트 ② 용어 탭에서 후보를 뽑아 넣을 수 있습니다."
          />
        ) : (
          <TermTableEditor
            key="user"
            terms={userTerms}
            onChange={(terms) => {
              setUserTerms(terms);
              setUserDirty(true);
            }}
          />
        )}
        {error && <p className="inline-warning">{error}</p>}
        <footer>
          <button type="button" className="icon-button-text compact" onClick={() => void close()}>
            <X size={15} />
            닫기
          </button>
        </footer>
      </section>
    </div>
  );
}

async function loadGlossary(sourcePath: string, targetLanguage: string, sampleKey: string): Promise<GlossaryInfo> {
  if (isPreviewRuntime()) {
    return {
      source_language: "eng",
      target_language: targetLanguage,
      game_pck_path: "Z:/game/sts2/SlayTheSpire2.pck",
      game_terms: 1953,
      user_terms: [{ source: "Artoria", targets: ["아르토리아"] }],
    };
  }
  return invokeCommand("load_translation_glossary", { sourcePath, targetLanguage, sampleKey: sampleKey || null });
}

async function loadProjectSettings(projectKey: string): Promise<ProjectTranslationSettings | null> {
  if (!projectKey) {
    return null;
  }
  if (isPreviewRuntime()) {
    return {
      mod_description: "",
      tone: "official",
      terms: [
        { source: "Noble Phantasm", targets: ["보구"] },
        { source: "Excalibur", targets: ["엑스칼리버"] },
        { source: "Avalon", targets: [] },
      ],
      extra_instructions: "",
    };
  }
  return invokeCommand("load_project_translation_settings", { projectKey });
}
