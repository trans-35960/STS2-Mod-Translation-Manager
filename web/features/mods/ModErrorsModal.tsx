import React from "react";
import { Check, Copy, FolderOpen, RefreshCw, X } from "lucide-react";
import type { ModError, ModRow } from "../../types";
import { displayModVersion, formatFullDateTime } from "./modUtils";
import { errorOccurrences, type ModErrorState } from "./useModErrors";

const MATCHED_BY_LABELS: Record<NonNullable<ModError["matched_by"]>, string> = {
  mod_id: "모드 로더가 이 모드 이름으로 기록",
  log_tag: "모드가 남긴 로그",
  path: "이 모드의 파일 경로가 나옴",
  stack: "이 모드의 코드에서 발생",
};

type GameInfo = { gameVersion: string | null; gameBranch: string | null };

/**
 * Errors the game logged during its latest run, grouped by the mod they were
 * traced to. Each error copies as a ready-to-paste report (game version, mod,
 * active mods, the logged stack), which is what an AI assistant needs to
 * explain it.
 */
export function ModErrorsModal({
  state,
  mods,
  modKey,
  game,
  onClose,
  onOpenPath,
}: {
  state: ModErrorState;
  mods: ModRow[];
  /** Only this mod's errors; all errors when null. */
  modKey: string | null;
  game: GameInfo;
  onClose: () => void;
  onOpenPath: (path: string) => void;
}) {
  const modsByKey = React.useMemo(() => new Map(mods.map((mod) => [mod.key, mod])), [mods]);
  const report = state.report;
  const sections = React.useMemo(() => {
    const list: Array<{ mod: ModRow | null; errors: ModError[] }> = [];
    for (const [key, errors] of state.byMod) {
      if (modKey && key !== modKey) continue;
      list.push({ mod: modsByKey.get(key) ?? null, errors });
    }
    list.sort((left, right) => errorOccurrences(right.errors) - errorOccurrences(left.errors));
    if (!modKey && state.unattributed.length > 0) {
      list.push({ mod: null, errors: state.unattributed });
    }
    return list;
  }, [state.byMod, state.unattributed, modKey, modsByKey]);

  React.useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const focusedMod = modKey ? modsByKey.get(modKey) : undefined;
  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <section className="modal mod-errors-modal" role="dialog" aria-modal="true" aria-label="게임 오류" onClick={(event) => event.stopPropagation()}>
        <header className="mod-errors-header">
          <div>
            <h2>{focusedMod ? `${modTitle(focusedMod)} 오류` : "게임 실행 중 오류"}</h2>
            <p>
              {report?.exists
                ? `최근 게임 로그 기준 · ${formatFullDateTime(report.modified_epoch)}`
                : "게임 로그를 찾지 못했습니다. 게임을 한 번 실행한 뒤 다시 확인하세요."}
            </p>
          </div>
          <div className="mod-errors-header-actions">
            <button type="button" className="icon-only-button" aria-label="다시 읽기" data-tooltip="다시 읽기" onClick={state.reload} disabled={state.loading}>
              <RefreshCw size={15} className={state.loading ? "spin-icon" : undefined} />
            </button>
            {report?.exists && (
              <button type="button" className="icon-only-button" aria-label="로그 위치 열기" data-tooltip="로그 위치 열기" onClick={() => onOpenPath(report.log_path)}>
                <FolderOpen size={15} />
              </button>
            )}
            <button type="button" className="icon-only-button" aria-label="닫기" data-tooltip="닫기" onClick={onClose}>
              <X size={16} />
            </button>
          </div>
        </header>
        <p className="mod-errors-hint">
          복사 버튼을 누르면 게임 버전, 모드 정보, 켜진 모드 목록과 오류 내용이 함께 복사됩니다. AI에게 그대로 붙여 넣어 물어볼 수 있습니다.
        </p>
        {state.error && <p className="mod-errors-failed">로그를 읽지 못했습니다: {state.error}</p>}
        <div className="mod-errors-body">
          {sections.length === 0 && <p className="mod-errors-empty">기록된 오류가 없습니다.</p>}
          {sections.map(({ mod, errors }) => (
            <section className="mod-errors-section" key={mod?.key ?? "unattributed"}>
              <div className="mod-errors-section-head">
                <strong>{mod ? modTitle(mod) : "원인 모드를 특정하지 못한 오류"}</strong>
                <span className="mod-errors-count">{errorOccurrences(errors)}회</span>
                {mod && !mod.active && <span className="mod-errors-muted">지금은 꺼져 있음</span>}
                <CopyButton label="모두 복사" text={() => errorReport(errors, mod, mods, game)} />
              </div>
              {!mod && (
                <p className="mod-errors-muted">
                  게임 코드에서 났지만 어느 모드 때문인지 로그에 나오지 않습니다. 최근에 켠 모드나 게임 코드를 바꾸는(하모니 패치) 모드를 의심해 보세요.
                </p>
              )}
              <ul>
                {errors.map((item) => (
                  <li key={item.id}>
                    <details>
                      <summary>
                        <span className="mod-error-message">{item.message}</span>
                        {item.count > 1 && <span className="mod-errors-count">×{item.count}</span>}
                      </summary>
                      <pre>{item.text}</pre>
                      <small className="mod-errors-muted">
                        로그 {item.first_line}번째 줄
                        {item.matched_by ? ` · ${MATCHED_BY_LABELS[item.matched_by]}` : ""}
                        {item.patched_game_code ? " · 모드가 패치한 게임 코드를 거침" : ""}
                      </small>
                    </details>
                    <CopyButton label="복사" text={() => errorReport([item], mod, mods, game)} />
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      </section>
    </div>
  );
}

function CopyButton({ label, text }: { label: string; text: () => string }) {
  const [state, setState] = React.useState<"idle" | "done" | "failed">("idle");
  React.useEffect(() => {
    if (state === "idle") return;
    const timer = window.setTimeout(() => setState("idle"), 1600);
    return () => window.clearTimeout(timer);
  }, [state]);
  return (
    <button
      type="button"
      className={`mod-error-copy${state === "done" ? " done" : ""}`}
      onClick={() => {
        if (!navigator.clipboard) {
          setState("failed");
          return;
        }
        navigator.clipboard.writeText(text()).then(
          () => setState("done"),
          () => setState("failed"),
        );
      }}
    >
      {state === "done" ? <Check size={13} /> : <Copy size={13} />}
      {state === "done" ? "복사됨" : state === "failed" ? "복사 실패" : label}
    </button>
  );
}

function modTitle(mod: ModRow): string {
  return mod.workshop_title?.trim() || mod.name;
}

/** A self-contained report to paste into an AI chat or a bug report. */
function errorReport(errors: ModError[], mod: ModRow | null, mods: ModRow[], game: GameInfo): string {
  const lines = ["Slay the Spire 2 모드 오류 보고"];
  lines.push(`게임: ${game.gameVersion ?? "버전 미상"}${game.gameBranch ? ` (Steam 브랜치 ${game.gameBranch})` : ""}`);
  if (mod) {
    const details = [
      mod.manifest_id && mod.manifest_id !== modTitle(mod) ? `id ${mod.manifest_id}` : "",
      displayModVersion(mod),
      mod.workshop_item_id ? `창작마당 ${mod.workshop_item_id}` : mod.source_label,
    ].filter(Boolean);
    lines.push(`모드: ${modTitle(mod)} (${details.join(", ")})`);
    if (mod.dependencies.length > 0) {
      lines.push(`선행 모드: ${mod.dependencies.map((dependency) => `${dependency.name}${dependency.version_current ? ` ${dependency.version_current}` : ""}${dependency.available ? "" : " (없음)"}`).join(", ")}`);
    }
  } else {
    lines.push("모드: 로그에서 원인 모드를 특정하지 못함");
  }
  const active = mods.filter((item) => item.active && !item.is_translation_patch && !item.system_mod);
  lines.push(`켜진 모드 ${active.length}개: ${active.map((item) => `${modTitle(item)} ${displayModVersion(item)}`).join(", ")}`);
  for (const item of errors) {
    lines.push("");
    const notes = [
      item.count > 1 ? `${item.count}회 발생` : "",
      item.patched_game_code ? "하모니로 패치된 게임 코드를 거침" : "",
    ].filter(Boolean);
    if (notes.length > 0) {
      lines.push(`(${notes.join(", ")})`);
    }
    lines.push("```", item.text, "```");
  }
  return lines.join("\n");
}
