import React from "react";
import { Pill } from "../../components/Common";
import { labels } from "../../i18n";
import type { ExtractionTreeNode, ModRow, TranslationWorkspace } from "../../types";
import {
  buildModGroups,
  displayModVersion,
  hasLocalizationBranch,
  joinResourcePath,
  languageResourceRoot,
} from "../mods/modUtils";
import { languageFolderCode, languageLabel } from "./translationUtils";

export function TranslationPage({
  labels: t,
  items,
  mods,
  targetLanguage,
  onStartModTranslation,
}: {
  labels: typeof labels.ko;
  items: TranslationWorkspace[];
  mods: ModRow[];
  targetLanguage: string;
  onStartModTranslation: (mod: ModRow, resourcePath?: string) => void;
}) {
  const [manualPaths, setManualPaths] = React.useState<Record<string, string>>({});
  const [modSearch, setModSearch] = React.useState("");
  const [activeOnly, setActiveOnly] = React.useState(false);
  const [expandedGroups, setExpandedGroups] = React.useState<Record<string, boolean>>({});
  const translatableMods = React.useMemo(
    () => mods.filter((mod) => mod.translation_state !== "검토 필요" || mod.language_preview.length > 0),
    [mods],
  );
  const groupedMods = React.useMemo(
    () => buildModGroups(translatableMods, modSearch, "all", activeOnly ? "enabled" : "all", "all", "all", "name"),
    [activeOnly, modSearch, translatableMods],
  );

  function toggleGroup(groupId: string) {
    setExpandedGroups((current) => ({ ...current, [groupId]: !current[groupId] }));
  }

  return (
    <div className="translation-start-layout">
      <section className="panel-list compact translation-main-list">
        <article className="panel">
          <div>
            <h3>번역 시작</h3>
            <p>대상 언어: {languageLabel(targetLanguage)} ({targetLanguage})</p>
          </div>
        </article>
        <div className="toolbar mod-toolbar translation-search-toolbar">
          <input value={modSearch} onChange={(event) => setModSearch(event.target.value)} placeholder={t.search} />
          <label className="check-filter">
            <input type="checkbox" checked={activeOnly} onChange={(event) => setActiveOnly(event.target.checked)} />
            <span>활성 모드만</span>
          </label>
        </div>
        {groupedMods.map((group) => {
          const isGrouped = group.mods.length > 1;
          const isExpanded = expandedGroups[group.id] ?? activeOnly;
          if (!isGrouped) {
            return (
              <TranslationStartCard
                key={group.mods[0].key}
                mod={group.mods[0]}
                manualPath={manualPaths[group.mods[0].key] ?? ""}
                onManualPathChange={(value) => setManualPaths((current) => ({ ...current, [group.mods[0].key]: value }))}
                onStartModTranslation={onStartModTranslation}
              />
            );
          }
          return (
            <React.Fragment key={group.id}>
              <div
                className="panel translation-start-group"
                role="button"
                tabIndex={0}
                onClick={() => toggleGroup(group.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    toggleGroup(group.id);
                  }
                }}
              >
                <div>
                  <h3>{group.name}</h3>
                  <p>{group.mods.length}개 버전 · 활성 {group.activeCount} · 변경 {group.updateCount}</p>
                </div>
                <Pill tone={group.activeCount > 0 ? "good" : "warn"}>{isExpanded ? "접기" : "버전 보기"}</Pill>
              </div>
              {isExpanded &&
                group.mods.map((mod) => (
                  <TranslationStartCard
                    key={mod.key}
                    mod={mod}
                    manualPath={manualPaths[mod.key] ?? ""}
                    onManualPathChange={(value) => setManualPaths((current) => ({ ...current, [mod.key]: value }))}
                    onStartModTranslation={onStartModTranslation}
                    child
                  />
                ))}
            </React.Fragment>
          );
        })}
        {groupedMods.length === 0 && <div className="empty compact">조건에 맞는 모드가 없습니다.</div>}
        <details className="translation-workspace-history">
          <summary>작업 이력 {items.length}개</summary>
          <div className="panel-list compact">
            {items.length === 0 && <div className="empty">No translation workspaces.</div>}
            {items.map((item) => (
              <article className="panel" key={`${item.mod_key}-${item.version_id}`}>
                <div>
                  <h3>{item.mod_key}</h3>
                  <p>{item.path}</p>
                </div>
                <Pill tone={item.review_required ? "warn" : "good"}>{item.review_required ? t.review : t.ready}</Pill>
              </article>
            ))}
          </div>
        </details>
      </section>
    </div>
  );
}

function TranslationStartCard({
  mod,
  manualPath,
  onManualPathChange,
  onStartModTranslation,
  child = false,
}: {
  mod: ModRow;
  manualPath: string;
  onManualPathChange: (value: string) => void;
  onStartModTranslation: (mod: ModRow, resourcePath?: string) => void;
  child?: boolean;
}) {
  const [showTree, setShowTree] = React.useState(false);
  return (
    <article className={child ? "panel translation-start-card child" : "panel translation-start-card"}>
      <div>
        <h3>{child ? displayModVersion(mod) : mod.name}</h3>
        <p>
          {mod.extraction_hint}
          {displayModVersion(mod) !== "-" && <span className="inline-version">버전 {displayModVersion(mod)}</span>}
        </p>
        {mod.language_preview.length > 0 ? (
          <div className="language-toggle-group">
            {mod.language_preview.map((language) => {
              const resourcePath = languageResourceRoot(language.sample_path);
              return (
                <button
                  type="button"
                  className="language-toggle compact"
                  key={`${mod.key}-${language.code}-${language.sample_path}`}
                  onClick={() => onStartModTranslation(mod, resourcePath)}
                >
                  <strong>{languageFolderCode(language)}</strong>
                  <small>{language.files}</small>
                </button>
              );
            })}
          </div>
        ) : (
          <div className="manual-language-picker">
            <div className="manual-picker-actions">
              <button type="button" onClick={() => setShowTree((current) => !current)}>
                {showTree ? "파일 트리 닫기" : "PCK 트리에서 선택"}
              </button>
              <button type="button" onClick={() => onStartModTranslation(mod, manualPath)} disabled={!manualPath.trim()}>
                선택 경로로 번역
              </button>
            </div>
            {manualPath && <code className="selected-resource-path">{manualPath}</code>}
            {showTree && (
              <LanguageCandidateTree
                nodes={mod.extraction_tree}
                selectedPath={manualPath}
                onSelect={onManualPathChange}
              />
            )}
            {showTree && mod.extraction_tree.length === 0 && <p className="tree-empty">표시할 PCK 파일 트리가 없습니다. 먼저 모드 관리에서 추출을 눌러 파일 트리를 갱신하세요.</p>}
          </div>
        )}
      </div>
      <Pill tone={mod.language_preview.length > 0 ? "good" : "warn"}>{mod.language_preview.length > 0 ? "감지됨" : "수동"}</Pill>
    </article>
  );
}

function LanguageCandidateTree({
  nodes,
  selectedPath,
  onSelect,
}: {
  nodes: ExtractionTreeNode[];
  selectedPath: string;
  onSelect: (path: string) => void;
}) {
  return (
    <div className="language-candidate-tree">
      <ul className="file-tree">
        {nodes.map((node) => (
          <LanguageCandidateNode node={node} depth={0} basePath="" selectedPath={selectedPath} onSelect={onSelect} key={`${node.kind}-${node.name}-${node.path}`} />
        ))}
      </ul>
    </div>
  );
}

function LanguageCandidateNode({
  node,
  depth,
  basePath,
  selectedPath,
  onSelect,
}: {
  node: ExtractionTreeNode;
  depth: number;
  basePath: string;
  selectedPath: string;
  onSelect: (path: string) => void;
}) {
  const isDirectory = node.children.length > 0 || node.kind === "dir";
  const nodePath = node.path || joinResourcePath(basePath, node.name);
  const selectable = isDirectory || node.kind === "language";
  const [isOpen, setIsOpen] = React.useState(depth < 2 || hasLocalizationBranch(node));
  const isSelected = selectedPath === nodePath;

  return (
    <li>
      <button
        type="button"
        className={isSelected ? `tree-node tree-select ${node.kind} selected` : `tree-node tree-select ${node.kind}`}
        title={nodePath || node.name}
        onClick={() => {
          if (isDirectory) {
            setIsOpen((current) => !current);
          }
          if (selectable) {
            onSelect(nodePath);
          }
        }}
      >
        <span className="tree-icon">{isDirectory ? (isOpen ? "v" : ">") : node.kind === "language" ? "T" : "."}</span>
        <span>{node.name}</span>
        {selectable && (
          <small>
            {isSelected ? "선택됨" : "선택"}
          </small>
        )}
      </button>
      {isDirectory && isOpen && node.children.length > 0 && (
        <ul className="file-tree nested">
          {node.children.map((child) => (
            <LanguageCandidateNode
              node={child}
              depth={depth + 1}
              basePath={nodePath}
              selectedPath={selectedPath}
              onSelect={onSelect}
              key={`${child.kind}-${child.name}-${child.path}`}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
