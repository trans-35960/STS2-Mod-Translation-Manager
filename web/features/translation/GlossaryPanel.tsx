import React from "react";
import { Save } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { GlossaryInfo, GlossaryTerm, ProjectTranslationSettings } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { TermTableEditor } from "./TermTableEditor";

/**
 * Shows which official game terms are checked and edits the two term lists
 * the validator and AI prompts use: this mod's terms (shared with the AI
 * prompt window) and the user's terms for the target language. Saving
 * re-runs validation so the sheet's glossary warnings update immediately.
 */
export function GlossaryPanel({
  sourcePath,
  targetLanguage,
  projectKey,
  refreshKey,
  disabled,
  onSaved,
}: {
  sourcePath: string;
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
  const [userDirty, setUserDirty] = React.useState(false);
  const [projectSettings, setProjectSettings] = React.useState<ProjectTranslationSettings | null>(null);
  const [modTerms, setModTerms] = React.useState<GlossaryTerm[]>([]);
  const [modDirty, setModDirty] = React.useState(false);
  const [error, setError] = React.useState("");
  const [saving, setSaving] = React.useState(false);

  React.useEffect(() => {
    if (!targetLanguage.trim()) {
      return;
    }
    let cancelled = false;
    loadGlossary(sourcePath, targetLanguage)
      .then((result) => {
        if (!cancelled) {
          setInfo(result);
          setUserTerms(result.user_terms);
          setUserDirty(false);
          setError("");
        }
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [sourcePath, targetLanguage]);

  React.useEffect(() => {
    let cancelled = false;
    loadProjectSettings(projectKey)
      .then((settings) => {
        if (!cancelled) {
          setProjectSettings(settings);
          setModTerms(settings?.terms ?? []);
          setModDirty(false);
        }
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [projectKey, refreshKey]);

  async function save() {
    setSaving(true);
    try {
      if (modDirty && projectKey && projectSettings) {
        const settings = { ...projectSettings, terms: modTerms };
        if (!isPreviewRuntime()) {
          await invokeCommand("save_project_translation_settings", { projectKey, settings });
        }
        setProjectSettings(settings);
        setModDirty(false);
      }
      if (userDirty) {
        const saved = isPreviewRuntime() ? userTerms : await invokeCommand("save_user_glossary", { targetLanguage, terms: userTerms });
        setUserTerms(saved);
        setInfo((current) => (current ? { ...current, user_terms: saved } : current));
        setUserDirty(false);
      }
      setError("");
      onSaved();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  const dirty = userDirty || modDirty;
  return (
    <details className="action-details glossary-details">
      <summary>
        용어집
        <small>
          게임 {info?.game_terms.toLocaleString() ?? "-"}
          {projectKey ? ` · 모드 ${modTerms.length}` : ""} · 사용자 {userTerms.length}
        </small>
      </summary>
      <div className="glossary-body">
        <p className="glossary-note">
          {info?.game_pck_path
            ? info.game_terms > 0
              ? `원문의 [gold] 강조 용어를 게임 공식 ${info.target_language} 명칭과 비교합니다. (${info.source_language} → ${info.target_language})`
              : `게임 PCK에 ${info.source_language} → ${info.target_language} 공식 용어가 없습니다.`
            : "게임 PCK(SlayTheSpire2.pck)를 찾지 못해 사용자 용어만 검사합니다."}
          {" "}모드 용어 → 사용자 용어 → 게임 명칭 순으로 우선합니다.
        </p>
        {projectKey && (
          <section className="glossary-section">
            <h4>
              이 모드 용어 <small>AI 프롬프트 용어 탭과 같은 목록</small>
            </h4>
            <TermTableEditor
              compact
              terms={modTerms}
              onChange={(terms) => {
                setModTerms(terms);
                setModDirty(true);
              }}
              emptyText="AI 프롬프트 ② 용어 탭에서 후보를 뽑아 넣을 수 있습니다."
            />
          </section>
        )}
        <section className="glossary-section">
          <h4>
            사용자 용어 <small>{targetLanguage || "대상 언어"}의 모든 모드에 적용</small>
          </h4>
          <TermTableEditor
            compact
            terms={userTerms}
            onChange={(terms) => {
              setUserTerms(terms);
              setUserDirty(true);
            }}
          />
        </section>
        <div className="compact-action-row">
          <button
            className="toolbar-icon-button dark"
            type="button"
            aria-label="용어집 저장"
            data-tooltip="용어집 저장 후 다시 검증"
            onClick={() => void save()}
            disabled={!dirty || saving || disabled || !targetLanguage.trim()}
          >
            <Save size={15} />
          </button>
          {dirty && <small className="glossary-dirty">저장 안 됨</small>}
        </div>
        {error && <p className="inline-warning">{error}</p>}
      </div>
    </details>
  );
}

async function loadGlossary(sourcePath: string, targetLanguage: string): Promise<GlossaryInfo> {
  if (isPreviewRuntime()) {
    return {
      source_language: "eng",
      target_language: targetLanguage,
      game_pck_path: "Z:/game/sts2/SlayTheSpire2.pck",
      game_terms: 1953,
      user_terms: [{ source: "Artoria", targets: ["아르토리아"] }],
    };
  }
  return invokeCommand("load_translation_glossary", { sourcePath, targetLanguage });
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
      ],
      extra_instructions: "",
    };
  }
  return invokeCommand("load_project_translation_settings", { projectKey });
}
