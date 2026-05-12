import React from "react";
import { Check, PackagePlus, X } from "lucide-react";
import type { DroppedModPreview, ModRow } from "../../types";
import {
  displayModVersion,
  formatBytes,
  formatFullDateTime,
  modGroupName,
  shortPath,
} from "./modUtils";

export type DroppedModDecision =
  | { path: string; mode: "new" }
  | { path: string; mode: "skip" }
  | { path: string; mode: "replace"; replacePath: string };

type DecisionValue = "new" | "skip" | `replace:${string}`;

function DroppedModConfirmModal(props: {
  items: DroppedModPreview[];
  mods: ModRow[];
  busy: string | null;
  onCancel: () => void;
  onConfirm: (decisions: DroppedModDecision[]) => void;
}) {
  const [decisions, setDecisions] = React.useState<Record<string, DecisionValue>>({});

  const rows = React.useMemo(
    () => props.items.map((item) => buildDroppedRow(item, props.mods)),
    [props.items, props.mods],
  );

  React.useEffect(() => {
    const next: Record<string, DecisionValue> = {};
    for (const row of rows) {
      next[row.item.path] = row.replaceableMatches[0] ? `replace:${row.replaceableMatches[0].path}` : "new";
    }
    setDecisions(next);
  }, [rows]);

  function setDecision(path: string, value: DecisionValue) {
    setDecisions((current) => ({ ...current, [path]: value }));
  }

  function confirm() {
    props.onConfirm(
      rows.map((row): DroppedModDecision => {
        const decision = decisions[row.item.path] ?? "new";
        if (decision === "skip") {
          return { path: row.item.path, mode: "skip" };
        }
        if (decision.startsWith("replace:")) {
          return {
            path: row.item.path,
            mode: "replace",
            replacePath: decision.slice("replace:".length),
          };
        }
        return { path: row.item.path, mode: "new" };
      }),
    );
  }

  return (
    <div className="modal-backdrop">
      <div className="modal dropped-mod-modal">
        <header>
          <h2>드롭한 모드 추가</h2>
          <p>기존 모드와 이름/버전이 비슷한 항목을 찾았습니다. 각 모드를 어떻게 처리할지 선택하세요.</p>
        </header>
        <div className="dropped-mod-list">
          {rows.map((row) => (
            <section className="dropped-mod-item" key={row.item.path}>
              <div className="dropped-mod-summary">
                <PackagePlus size={18} />
                <div>
                  <strong>{row.item.name}</strong>
                  <small>
                    {kindLabel(row.item.kind)} · {displayModVersion(row.previewRow)} · {formatBytes(row.item.bytes)}
                  </small>
                  <code>{row.item.display_path ? shortPath(row.item.display_path) : shortPath(row.item.path)}</code>
                </div>
              </div>
              <label className="modal-field compact">
                <span>처리 방식</span>
                <select
                  value={decisions[row.item.path] ?? "new"}
                  onChange={(event) => setDecision(row.item.path, event.target.value as DecisionValue)}
                >
                  <option value="new">새 모드로 등록</option>
                  {row.replaceableMatches.map((mod) => (
                    <option key={mod.path} value={`replace:${mod.path}`}>
                      덮어쓰기: {modGroupName(mod)} / {displayModVersion(mod)} / {sourceLabel(mod)}
                    </option>
                  ))}
                  <option value="skip">건너뛰기</option>
                </select>
              </label>
              {row.relatedMatches.length > 0 ? (
                <div className="dropped-mod-candidates">
                  <span>업데이트 후보</span>
                  {row.relatedMatches.slice(0, 4).map((mod) => (
                    <small key={mod.path}>
                      {modGroupName(mod)} · {displayModVersion(mod)} · {sourceLabel(mod)}
                      {mod.modified_epoch ? ` · ${formatFullDateTime(mod.modified_epoch)}` : ""}
                    </small>
                  ))}
                </div>
              ) : (
                <p className="dropped-mod-empty">비슷한 기존 모드를 찾지 못했습니다.</p>
              )}
            </section>
          ))}
        </div>
        <footer>
          <button type="button" className="icon-button-text" onClick={props.onCancel} disabled={Boolean(props.busy)}>
            <X size={15} />
            취소
          </button>
          <button type="button" className="primary icon-button-text" onClick={confirm} disabled={Boolean(props.busy)}>
            <Check size={15} />
            진행
          </button>
        </footer>
      </div>
    </div>
  );
}

function buildDroppedRow(item: DroppedModPreview, mods: ModRow[]) {
  const previewRow = previewAsModRow(item);
  const itemTokens = droppedMatchTokens(previewRow);
  const exactMatches = mods.filter((mod) => mod.key === item.key && isReplaceable(mod));
  const relatedMatches = mods.filter(
    (mod) => mod.key === item.key || hasSharedDroppedMatchToken(itemTokens, droppedMatchTokens(mod)),
  );
  const replaceableMatches = [
    ...exactMatches,
    ...relatedMatches.filter((mod) => !exactMatches.some((exact) => exact.path === mod.path) && isReplaceable(mod)),
  ];
  return {
    item,
    previewRow,
    exactMatches,
    relatedMatches,
    replaceableMatches,
  };
}

function hasSharedDroppedMatchToken(left: Set<string>, right: Set<string>): boolean {
  for (const token of left) {
    if (right.has(token)) {
      return true;
    }
  }
  return false;
}

function droppedMatchTokens(mod: ModRow): Set<string> {
  return new Set(
    [
      mod.key,
      mod.name,
      mod.group_name ?? "",
      mod.manifest_id ?? "",
      modGroupName(mod),
    ].flatMap(droppedTokenVariants).filter(Boolean),
  );
}

function droppedTokenVariants(value: string): string[] {
  const withoutArchive = value.replace(/\.(zip|rar|7z|pck|pak|jar)$/i, "");
  const withoutTranslation = withoutArchive.replace(/\s+korean\s+translation$/i, "").replace(/[_\-\s]?tr$/i, "");
  const withoutVersionSuffix = withoutTranslation
    .replace(/\s+v?\d[\w.\- ]*$/i, "")
    .replace(/[-_]+v?\d[\w.\-]*$/i, "");
  const leadingName = withoutTranslation.match(/^(.+?)[\s_-]+v?\d/i)?.[1] ?? withoutVersionSuffix;
  return [value, withoutArchive, withoutTranslation, withoutVersionSuffix, leadingName].map(normalizeDroppedToken);
}

function normalizeDroppedToken(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9가-힣\u3400-\u9fff]/g, "");
}

function previewAsModRow(item: DroppedModPreview): ModRow {
  return {
    key: item.key,
    name: item.name,
    manifest_id: null,
    group_name: null,
    active: false,
    managed: false,
    external: false,
    source_label: "drop",
    kind: item.kind,
    version_hint: item.version_hint,
    bytes: item.bytes,
    modified_epoch: item.modified_epoch,
    registered_epoch: null,
    updated_epoch: null,
    path: item.path,
    update_state: "clean",
    change_reasons: [],
    translation_state: "",
    translation_applied: false,
    translation_applied_epoch: null,
    translation_patch_count: 0,
    translation_patch_active_count: 0,
    translation_patch_names: [],
    needs_recheck: false,
    translation_review_required: false,
    safety_warnings: [],
    extraction_hint: "",
    extraction_source_path: item.path,
    extraction_target: "",
    is_translation_patch: false,
    translation_target_id: null,
    translation_target_key: null,
    translation_target_name: null,
    translation_target_version: null,
    dependencies: [],
    language_preview: [],
    extraction_tree: [],
  };
}

function isReplaceable(mod: ModRow): boolean {
  return mod.active || mod.managed;
}

function sourceLabel(mod: ModRow): string {
  if (mod.active) return "활성";
  if (mod.source_label) return mod.source_label;
  if (mod.managed) return "기존 등록";
  return "기존 모드";
}

function kindLabel(kind: string): string {
  switch (kind) {
    case "folder":
      return "폴더";
    case "archive":
      return "압축파일";
    case "package":
      return "패키지";
    default:
      return kind || "모드";
  }
}

export { DroppedModConfirmModal };
