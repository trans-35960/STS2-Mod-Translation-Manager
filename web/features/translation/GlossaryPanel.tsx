import React from "react";
import { Plus, Save, Trash2 } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { GlossaryInfo, GlossaryTerm } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";

type DraftTerm = { id: number; source: string; targets: string };

const TARGET_SEPARATOR = " / ";

/**
 * Shows which official game terms are checked and edits the user's own term
 * list for the current target language. Saving re-runs validation so the
 * sheet's glossary warnings update immediately.
 */
export function GlossaryPanel({
  sourcePath,
  targetLanguage,
  disabled,
  onSaved,
}: {
  sourcePath: string;
  targetLanguage: string;
  disabled: boolean;
  onSaved: () => void;
}) {
  const [info, setInfo] = React.useState<GlossaryInfo | null>(null);
  const [drafts, setDrafts] = React.useState<DraftTerm[]>([]);
  const [dirty, setDirty] = React.useState(false);
  const [error, setError] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const nextId = React.useRef(0);

  const toDrafts = React.useCallback(
    (terms: GlossaryTerm[]) =>
      terms.map((term) => ({ id: nextId.current++, source: term.source, targets: term.targets.join(TARGET_SEPARATOR) })),
    [],
  );

  React.useEffect(() => {
    if (!targetLanguage.trim()) {
      return;
    }
    let cancelled = false;
    loadGlossary(sourcePath, targetLanguage)
      .then((result) => {
        if (!cancelled) {
          setInfo(result);
          setDrafts(toDrafts(result.user_terms));
          setDirty(false);
          setError("");
        }
      })
      .catch((reason) => !cancelled && setError(String(reason)));
    return () => {
      cancelled = true;
    };
  }, [sourcePath, targetLanguage, toDrafts]);

  function updateDraft(id: number, field: "source" | "targets", value: string) {
    setDrafts((current) => current.map((draft) => (draft.id === id ? { ...draft, [field]: value } : draft)));
    setDirty(true);
  }

  function addDraft() {
    setDrafts((current) => [...current, { id: nextId.current++, source: "", targets: "" }]);
    setDirty(true);
  }

  function removeDraft(id: number) {
    setDrafts((current) => current.filter((draft) => draft.id !== id));
    setDirty(true);
  }

  async function save() {
    const terms = drafts
      .map((draft) => ({
        source: draft.source.trim(),
        targets: draft.targets.split("/").map((target) => target.trim()).filter(Boolean),
      }))
      .filter((term) => term.source && term.targets.length > 0);
    setSaving(true);
    try {
      const saved = isPreviewRuntime() ? terms : await invokeCommand("save_user_glossary", { targetLanguage, terms });
      setDrafts(toDrafts(saved));
      setInfo((current) => (current ? { ...current, user_terms: saved } : current));
      setDirty(false);
      setError("");
      onSaved();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  const userCount = info?.user_terms.length ?? 0;
  return (
    <details className="action-details glossary-details">
      <summary>
        용어집
        <small>
          게임 {info?.game_terms.toLocaleString() ?? "-"} · 사용자 {userCount}
        </small>
      </summary>
      <div className="glossary-body">
        <p className="glossary-note">
          {info?.game_pck_path
            ? info.game_terms > 0
              ? `원문의 [gold] 강조 용어를 게임 공식 ${info.target_language} 명칭과 비교합니다. (${info.source_language} → ${info.target_language})`
              : `게임 PCK에 ${info.source_language} → ${info.target_language} 공식 용어가 없습니다.`
            : "게임 PCK(SlayTheSpire2.pck)를 찾지 못해 사용자 용어만 검사합니다."}
          {" "}사용자 용어는 원문 어디에 있든 검사하고, 같은 단어면 게임 명칭보다 우선합니다.
        </p>
        <div className="glossary-rows">
          {drafts.map((draft) => (
            <div className="glossary-row" key={draft.id}>
              <input
                aria-label="원문 용어"
                placeholder="원문 (예: Artoria)"
                value={draft.source}
                onChange={(event) => updateDraft(draft.id, "source", event.target.value)}
              />
              <input
                aria-label="번역 용어"
                placeholder="번역 (여러 개는 / 로 구분)"
                value={draft.targets}
                onChange={(event) => updateDraft(draft.id, "targets", event.target.value)}
              />
              <button
                className="toolbar-icon-button"
                type="button"
                aria-label="용어 삭제"
                data-tooltip="용어 삭제"
                onClick={() => removeDraft(draft.id)}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </div>
        <div className="compact-action-row">
          <button className="toolbar-icon-button" type="button" aria-label="용어 추가" data-tooltip="용어 추가" onClick={addDraft}>
            <Plus size={15} />
          </button>
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
