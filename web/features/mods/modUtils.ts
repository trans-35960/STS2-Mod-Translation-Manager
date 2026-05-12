import type {
  ActiveFilter,
  ChangeFilter,
  DashboardStatFilter,
  ExtractionTreeNode,
  LanguagePreview,
  ModDependency,
  ModGroup,
  ModRow,
  ModSort,
  Preset,
  TranslationApplyFilter,
} from "../../types";

export function presetPreviewSummary(preset: Preset, mods: ModRow[]): string {
  const active = mods.filter((mod) => mod.active);
  const targetKeys = new Set(preset.mods.map((mod) => mod.key));
  const availableKeys = new Set(mods.filter((mod) => mod.managed || mod.active || mod.external).map((mod) => mod.key));
  const missing = preset.mods.filter((mod) => !availableKeys.has(mod.key));
  const willEnable = preset.mods.filter((mod) => !active.some((row) => row.key === mod.key));
  const willDisable = active.filter((mod) => !targetKeys.has(mod.key));
  const versionWarnings = preset.mods.filter((presetMod) => {
    const current = mods.find((mod) => mod.key === presetMod.key);
    if (!current) return false;
    return Boolean(
      (presetMod.bytes && current.bytes && presetMod.bytes !== current.bytes)
      || (presetMod.modified_epoch && current.modified_epoch && presetMod.modified_epoch !== current.modified_epoch),
    );
  });
  const inactiveDependencies = preset.mods.flatMap((presetMod) => {
    const mod = mods.find((row) => row.key === presetMod.key);
    return (mod?.dependencies ?? []).filter((dependency) => dependency.available && !dependency.active).map((dependency) => dependency.name);
  });
  return [
    `'${preset.name}' 프리셋을 적용합니다.`,
    "",
    `활성화 예정: ${willEnable.length}개${willEnable.length ? ` (${willEnable.slice(0, 5).map((mod) => mod.key).join(", ")})` : ""}`,
    `비활성화 예정: ${willDisable.length}개${willDisable.length ? ` (${willDisable.slice(0, 5).map((mod) => mod.name).join(", ")})` : ""}`,
    missing.length ? `누락 모드: ${missing.map((mod) => mod.key).join(", ")}` : "누락 모드: 없음",
    versionWarnings.length ? `버전/파일 차이: ${versionWarnings.map((mod) => mod.key).join(", ")}` : "버전/파일 차이: 없음",
    inactiveDependencies.length ? `함께 켤 선행 모드: ${Array.from(new Set(inactiveDependencies)).join(", ")}` : "함께 켤 선행 모드: 없음",
    "",
    "계속 적용할까요?",
  ].join("\n");
}

export function shortPath(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.length > 3 ? `.../${parts.slice(-3).join("/")}` : normalized;
}

export function languageKeyCount(language: LanguagePreview): number {
  return language.keys ?? language.files;
}

export function uniqueLanguagePreviews(languages: LanguagePreview[]): LanguagePreview[] {
  const byCode = new Map<string, LanguagePreview>();
  for (const language of languages) {
    const code = normalizeLanguageTag(languageFolderCode(language));
    const existing = byCode.get(code);
    if (!existing || languageKeyCount(language) > languageKeyCount(existing) || (languageKeyCount(language) === languageKeyCount(existing) && language.files > existing.files)) {
      byCode.set(code, language);
    }
  }
  return Array.from(byCode.values()).sort(
    (left, right) =>
      languageKeyCount(right) - languageKeyCount(left) ||
      right.files - left.files ||
      languageFolderCode(left).localeCompare(languageFolderCode(right)),
  );
}

export function normalizeLanguageTag(code: string): string {
  const normalized = code.trim().toLowerCase();
  if (normalized === "ko") {
    return "kor";
  }
  if (normalized === "en") {
    return "eng";
  }
  if (normalized === "ja") {
    return "jpn";
  }
  if (normalized === "ru") {
    return "rus";
  }
  if (normalized === "zh-cn") {
    return "zhs";
  }
  if (normalized === "zh-tw") {
    return "zht";
  }
  return normalized;
}

export function recommendedSourceLanguage(languages: LanguagePreview[]): LanguagePreview | undefined {
  return uniqueLanguagePreviews(languages).sort(
    (left, right) =>
      languageKeyCount(right) - languageKeyCount(left) ||
      right.files - left.files ||
      languageFolderCode(left).localeCompare(languageFolderCode(right)),
  )[0];
}

export function representativeLanguage(languages: LanguagePreview[], targetLanguage: string): LanguagePreview | null {
  const unique = uniqueLanguagePreviews(languages);
  const normalizedTarget = normalizeLanguageTag(targetLanguage);
  return (
    unique.find((language) => normalizeLanguageTag(languageFolderCode(language)) === normalizedTarget) ??
    recommendedSourceLanguage(unique) ??
    unique[0] ??
    null
  );
}

export function buildModGroups(
  mods: ModRow[],
  search: string,
  statFilter: DashboardStatFilter,
  activeFilter: ActiveFilter,
  changeFilter: ChangeFilter,
  translationApplyFilter: TranslationApplyFilter,
  sort: ModSort,
): ModGroup[] {
  const normalizedSearch = search.trim().toLowerCase();
  const targetGroupNames = new Map(
    mods
      .filter((mod) => !mod.is_translation_patch)
      .map((mod) => [mod.key, modGroupName(mod)]),
  );
  const targetGroupsByToken = buildTargetGroupsByToken(mods);
  const groups = new Map<string, ModGroup>();
  for (const mod of mods) {
    if (!matchesStatFilter(mod, statFilter) || !matchesModFilters(mod, activeFilter, changeFilter, translationApplyFilter) || !matchesModSearch(mod, normalizedSearch)) {
      continue;
    }
    const name = modDisplayGroupName(mod, targetGroupNames, targetGroupsByToken);
    const id = name.toLowerCase();
    const group = groups.get(id) ?? {
      id,
      name,
      mods: [],
      activeCount: 0,
      updateCount: 0,
    };
    const existingIndex = findMergeIndex(group.mods, mod);
    if (existingIndex >= 0) {
      group.mods[existingIndex] = mergeModRows(group.mods[existingIndex], mod);
    } else {
      group.mods.push(mod);
    }
    groups.set(id, group);
  }
  return Array.from(groups.values())
    .map((group) => {
      const sortedMods = [...group.mods].sort((a, b) => compareMods(a, b, sort));
      return {
        ...group,
        mods: sortedMods,
        activeCount: sortedMods.filter((mod) => mod.active).length,
        updateCount: sortedMods.filter((mod) => mod.update_state !== "clean").length,
      };
    })
    .sort((a, b) => compareGroups(a, b, sort));
}

function modDisplayGroupName(
  mod: ModRow,
  targetGroupNames: Map<string, string>,
  targetGroupsByToken: Map<string, string>,
): string {
  if (!mod.is_translation_patch) {
    return modGroupName(mod);
  }
  if (mod.translation_target_key && targetGroupNames.has(mod.translation_target_key)) {
    return targetGroupNames.get(mod.translation_target_key) ?? modGroupName(mod);
  }
  const dependencyTarget = mod.dependencies
    .map((dependency) => dependency.key)
    .find((key): key is string => Boolean(key && targetGroupNames.has(key)));
  if (dependencyTarget) {
    return targetGroupNames.get(dependencyTarget) ?? modGroupName(mod);
  }
  for (const token of translationPatchTargetTokens(mod)) {
    const groupName = targetGroupsByToken.get(token);
    if (groupName) {
      return groupName;
    }
  }
  return modGroupName(mod);
}

function buildTargetGroupsByToken(mods: ModRow[]): Map<string, string> {
  const tokens = new Map<string, string>();
  for (const mod of mods) {
    if (mod.is_translation_patch) {
      continue;
    }
    const groupName = modGroupName(mod);
    for (const token of modTargetTokens(mod, groupName)) {
      if (token && !tokens.has(token)) {
        tokens.set(token, groupName);
      }
    }
  }
  return tokens;
}

function modTargetTokens(mod: ModRow, groupName: string): string[] {
  return [
    mod.key,
    mod.name,
    mod.manifest_id ?? "",
    mod.group_name ?? "",
    groupName,
    modActivationGroupName(mod),
  ].flatMap(groupingTokenVariants);
}

function translationPatchTargetTokens(mod: ModRow): string[] {
  const dependencyValues = mod.dependencies.flatMap((dependency) => [
    dependency.key ?? "",
    dependency.id,
    dependency.name,
  ]);
  return [
    mod.translation_target_key ?? "",
    mod.translation_target_id ?? "",
    mod.translation_target_name ?? "",
    ...dependencyValues,
  ].flatMap(groupingTokenVariants);
}

function groupingTokenVariants(value: string): string[] {
  const withoutTranslationSuffix = value.replace(/\s+korean\s+translation$/i, "").replace(/[_-]?tr$/i, "");
  const withoutArchiveSuffix = withoutTranslationSuffix.replace(/\.(zip|rar|7z|pck)$/i, "");
  return [
    normalizeGroupingToken(value),
    normalizeGroupingToken(withoutTranslationSuffix),
    normalizeGroupingToken(withoutArchiveSuffix),
  ].filter(Boolean);
}

function normalizeGroupingToken(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9가-힣\u3400-\u9fff]/g, "");
}

function matchesStatFilter(mod: ModRow, statFilter: DashboardStatFilter): boolean {
  switch (statFilter) {
    case "active":
      return mod.active;
    case "inactive":
      return !mod.active;
    case "external":
      return mod.external;
    case "changed":
      return mod.update_state !== "clean";
    case "all":
    default:
      return true;
  }
}

export function activeSiblingMods(mods: ModRow[], mod: ModRow): ModRow[] {
  const group = modActivationGroupName(mod).toLowerCase();
  return mods.filter((item) => item.key !== mod.key && item.active && modActivationGroupName(item).toLowerCase() === group);
}

export function preferredGroupActivationTarget(mods: ModRow[]): ModRow | undefined {
  return mods.filter((mod) => !isDownloadingMod(mod)).sort(compareGroupActivationTarget)[0];
}

function compareGroupActivationTarget(left: ModRow, right: ModRow): number {
  return (
    Number(!right.is_translation_patch) - Number(!left.is_translation_patch) ||
    Number(right.managed) - Number(left.managed) ||
    Number(right.external) - Number(left.external) ||
    compareEpochDesc(left.updated_epoch, right.updated_epoch) ||
    compareEpochDesc(left.modified_epoch, right.modified_epoch) ||
    versionCollator.compare(modVersionLabel(right), modVersionLabel(left)) ||
    left.name.localeCompare(right.name)
  );
}

function modIdentityKey(mod: ModRow): string {
  const kind = mod.is_translation_patch ? `translation|${mod.key}` : "mod";
  return `${modGroupName(mod).toLowerCase()}|${kind}|${displayModVersion(mod).toLowerCase()}`;
}

function findMergeIndex(mods: ModRow[], mod: ModRow): number {
  const identity = modIdentityKey(mod);
  const exactIndex = mods.findIndex((item) => modIdentityKey(item) === identity);
  if (exactIndex >= 0) {
    return exactIndex;
  }
  const version = displayModVersion(mod);
  if (version !== "-") {
    const noVersionIndex = mods.findIndex((item) => modGroupName(item).toLowerCase() === modGroupName(mod).toLowerCase() && displayModVersion(item) === "-");
    if (noVersionIndex >= 0) {
      return noVersionIndex;
    }
    return -1;
  }
  const sameGroup = mods
    .map((item, index) => ({ item, index }))
    .filter(({ item }) => modGroupName(item).toLowerCase() === modGroupName(mod).toLowerCase());
  return sameGroup.length === 1 ? sameGroup[0].index : -1;
}

function mergeModRows(left: ModRow, right: ModRow): ModRow {
  const primary = modDisplayPriority(right) > modDisplayPriority(left) ? right : left;
  const secondary = primary === right ? left : right;
  return {
    ...primary,
    version_hint: primary.version_hint ?? secondary.version_hint,
    active: left.active || right.active,
    managed: left.managed || right.managed,
    external: left.external || right.external,
    source_label: mergeTextLabels(left.source_label, right.source_label),
    update_state: mergeUpdateState(left.update_state, right.update_state),
    translation_state: primary.translation_state || secondary.translation_state,
    translation_patch_count: Math.max(left.translation_patch_count, right.translation_patch_count),
    translation_patch_active_count: Math.max(left.translation_patch_active_count, right.translation_patch_active_count),
    translation_patch_names: Array.from(new Set([...left.translation_patch_names, ...right.translation_patch_names])),
    extraction_hint: primary.extraction_hint || secondary.extraction_hint,
    extraction_source_path: primary.extraction_source_path || secondary.extraction_source_path,
    extraction_target: primary.extraction_target || secondary.extraction_target,
    dependencies: mergeDependencies(left.dependencies, right.dependencies),
    language_preview: primary.language_preview.length > 0 ? primary.language_preview : secondary.language_preview,
    extraction_tree: primary.extraction_tree.length > 0 ? primary.extraction_tree : secondary.extraction_tree,
  };
}

function modDisplayPriority(mod: ModRow): number {
  return (mod.active ? 8 : 0) + (mod.managed ? 4 : 0) + (mod.external ? 2 : 0) + (mod.extraction_tree.length > 0 ? 1 : 0);
}

function mergeTextLabels(left: string, right: string): string {
  return Array.from(new Set([left, right].flatMap((value) => value.split(" + ").map((part) => part.trim()).filter(Boolean)))).join(" + ");
}

function mergeUpdateState(left: string, right: string): string {
  if (left === "updated" || right === "updated") return "updated";
  if (left === "new" || right === "new") return "new";
  return left !== "clean" ? left : right;
}

function mergeDependencies(left: ModDependency[], right: ModDependency[]): ModDependency[] {
  const byId = new Map<string, ModDependency>();
  for (const dependency of [...left, ...right]) {
    const id = dependency.id.toLowerCase();
    const existing = byId.get(id);
    byId.set(
      id,
      existing
        ? {
            ...dependency,
            active: existing.active || dependency.active,
            available: existing.available || dependency.available,
            key: dependency.key ?? existing.key,
            version_required: dependency.version_required ?? existing.version_required,
            version_current: dependency.version_current ?? existing.version_current,
            version_matches: dependency.version_matches ?? existing.version_matches,
          }
        : dependency,
    );
  }
  return Array.from(byId.values());
}

function compareGroups(left: ModGroup, right: ModGroup, sort: ModSort): number {
  const leftRepresentative = groupRepresentative(left, sort);
  const rightRepresentative = groupRepresentative(right, sort);
  return compareMods(leftRepresentative, rightRepresentative, sort) || left.name.localeCompare(right.name);
}

function groupRepresentative(group: ModGroup, sort: ModSort): ModRow {
  return [...group.mods].sort((left, right) => compareMods(left, right, sort))[0];
}

function compareMods(left: ModRow, right: ModRow, sort: ModSort): number {
  const byName = modGroupName(left).localeCompare(modGroupName(right)) || modVersionLabel(left).localeCompare(modVersionLabel(right));
  switch (sort) {
    case "registered":
      return compareEpochDesc(left.registered_epoch, right.registered_epoch) || byName;
    case "updated":
      return compareEpochDesc(left.updated_epoch, right.updated_epoch) || byName;
    case "modified":
      return compareEpochDesc(left.modified_epoch, right.modified_epoch) || byName;
    case "translationApplied":
      return compareEpochDesc(left.translation_applied_epoch, right.translation_applied_epoch) || Number(right.translation_applied) - Number(left.translation_applied) || byName;
    case "active":
      return Number(right.active) - Number(left.active) || byName;
    case "change":
      return updateRank(right.update_state) - updateRank(left.update_state) || byName;
    case "source":
      return left.source_label.localeCompare(right.source_label) || byName;
    case "version":
      return modVersionLabel(left).localeCompare(modVersionLabel(right)) || modGroupName(left).localeCompare(modGroupName(right));
    case "name":
    default:
      return byName;
  }
}

const versionCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

function compareEpochDesc(left: number | null, right: number | null): number {
  return (right ?? 0) - (left ?? 0);
}

function updateRank(value: string): number {
  if (value === "updated") return 3;
  if (value === "new") return 2;
  if (value !== "clean") return 1;
  return 0;
}

function matchesModFilters(mod: ModRow, activeFilter: ActiveFilter, changeFilter: ChangeFilter, translationApplyFilter: TranslationApplyFilter): boolean {
  if (activeFilter === "enabled" && !mod.active) return false;
  if (activeFilter === "disabled" && mod.active) return false;
  if (changeFilter === "changed" && mod.update_state === "clean") return false;
  if (changeFilter === "new" && mod.update_state !== "new") return false;
  if (changeFilter === "updated" && mod.update_state !== "updated") return false;
  if (changeFilter === "clean" && mod.update_state !== "clean") return false;
  const hasTranslation = mod.translation_applied || mod.translation_patch_active_count > 0 || mod.translation_patch_count > 0;
  if (translationApplyFilter === "applied" && !hasTranslation) return false;
  if (translationApplyFilter === "notApplied" && hasTranslation) return false;
  return true;
}

export function canDeleteMod(mod: ModRow): boolean {
  const source = mod.source_label.toLowerCase();
  return mod.active || mod.managed || mod.external || source.includes("게임") || source.includes("game") || source.includes("vault") || source.includes("nexus") || source.includes("vortex");
}

export function isDownloadingMod(mod: ModRow): boolean {
  return mod.download_state === "downloading";
}

function matchesModSearch(mod: ModRow, normalizedSearch: string): boolean {
  if (!normalizedSearch) {
    return true;
  }
  return modSearchTokens(mod).some((value) => value.toLowerCase().includes(normalizedSearch));
}

function modSearchTokens(mod: ModRow): string[] {
  return [
    mod.name,
    mod.key,
    modGroupName(mod),
    mod.version_hint ?? "",
    mod.path,
    mod.source_label,
    isDownloadingMod(mod) ? "다운로드 중 vortex download downloading" : "",
    mod.change_reasons.join(" "),
    mod.active ? "활성 enabled active" : "비활성 disabled inactive",
    mod.update_state === "clean" ? "변경 없음 clean" : `변경 있음 ${mod.update_state}`,
    mod.translation_applied || mod.translation_patch_count > 0 ? "번역 적용됨 번역모드 translated applied patch" : "번역 미적용 not applied",
    mod.translation_patch_names.join(" "),
    formatDateToken(mod.registered_epoch),
    formatDateToken(mod.updated_epoch),
    formatDateToken(mod.modified_epoch),
    formatDateToken(mod.translation_applied_epoch),
    ...mod.language_preview.flatMap((language) => [language.code, language.label, languageFolderCode(language)]),
  ];
}

export function modGroupName(mod: ModRow): string {
  if (mod.is_translation_patch && mod.translation_target_name?.trim()) {
    return titleModName(mod.translation_target_name.trim());
  }
  if (mod.group_name?.trim()) {
    return titleModName(mod.group_name.trim());
  }
  if (mod.is_translation_patch && mod.translation_target_id?.trim()) {
    return titleModName(mod.translation_target_id.trim());
  }
  return modActivationGroupName(mod);
}

function modActivationGroupName(mod: ModRow): string {
  if (mod.group_name?.trim()) {
    return titleModName(mod.group_name.trim());
  }
  const original = (mod.name || mod.key).trim();
  const withoutArchiveSuffix = original.replace(/\.(zip|rar|7z|pck)$/i, "");
  const parts = withoutArchiveSuffix.split(/[-_]+/).filter(Boolean);
  if (parts.length < 3) {
    return titleModName(withoutArchiveSuffix);
  }
  const firstNumeric = parts.findIndex((part) => /^\d+$/.test(part));
  if (firstNumeric > 0) {
    return titleModName(parts.slice(0, firstNumeric).join(" "));
  }
  return titleModName(withoutArchiveSuffix.replace(/[-_]+(?:v?\d+(?:[-_.]\d+){1,}.*)$/i, ""));
}

function inferredVersionFromName(mod: ModRow): string {
  const name = (mod.name || mod.key).replace(/\.(zip|rar|7z|pck)$/i, "");
  const group = modGroupName(mod).toLowerCase().replace(/\s+/g, "-");
  const normalized = name.toLowerCase();
  if (normalized.startsWith(group)) {
    const suffix = name.slice(group.length).replace(/^[-_\s]+/, "");
    if (suffix && suffix.toLowerCase() !== normalized) {
      return suffix;
    }
  }
  const numeric = name.match(/(?:^|[-_])((?:v)?\d+(?:[-_.]\d+){1,}(?:[-_.][A-Za-z0-9]+)*)$/i);
  return numeric?.[1]?.replace(/^[-_]+/, "") ?? "";
}

export function displayModVersion(mod: ModRow): string {
  return mod.version_hint || inferredVersionFromName(mod) || "-";
}

function titleModName(value: string): string {
  const cleaned = value.replace(/[-_]+/g, " ").trim();
  if (!cleaned) {
    return "Unknown Mod";
  }
  return cleaned.replace(/\b([a-z])/g, (letter) => letter.toUpperCase());
}

function modVersionLabel(mod: ModRow): string {
  if (mod.version_hint) {
    return mod.version_hint;
  }
  return inferredVersionFromName(mod) || mod.key || mod.name;
}

export function compactSourceSummary(mods: ModRow[]): string {
  const labels = Array.from(new Set(mods.flatMap((mod) => mod.source_label.split(" + ").filter(Boolean))));
  return labels.length > 2 ? `${labels.slice(0, 2).join(" + ")} +${labels.length - 2}` : labels.join(" + ") || "-";
}

export function compactVersionSummary(mods: ModRow[]): string {
  const versions = Array.from(new Set(mods.map(displayModVersion).filter((value) => value && value !== "-")));
  if (versions.length === 0) {
    return "-";
  }
  return versions.length <= 2 ? versions.join(", ") : `${versions.slice(0, 2).join(", ")} +${versions.length - 2}`;
}

export function compactLanguageSummary(mods: ModRow[]): string {
  const languages = uniqueLanguagePreviews(mods.flatMap((mod) => mod.language_preview)).map((language) => language.label);
  return languages.length > 0 ? `${languages.slice(0, 3).join(", ")}${languages.length > 3 ? " ..." : ""}` : "언어 없음";
}

export function activeModSummary(mods: ModRow[]): string {
  const active = mods.filter((mod) => mod.active);
  if (active.length === 0) {
    return "활성 없음";
  }
  const labels = active.map((mod) => `${displayModVersion(mod)} · ${compactSourceSummary([mod])}`);
  return `활성 ${labels.slice(0, 2).join(", ")}${labels.length > 2 ? ` +${labels.length - 2}` : ""}`;
}

export function activeModVersionSummary(mods: ModRow[]): string {
  const active = mods.find((mod) => mod.active);
  return active ? `ACTIVE ${displayModVersion(active)}` : "ACTIVE";
}

export function compactDateSummary(mods: ModRow[]): string {
  return `등록 ${formatShortDate(maxEpoch(mods.map((mod) => mod.registered_epoch)))}`;
}

export function compactModifiedSummary(mods: ModRow[]): string {
  return `업데이트 ${formatShortDate(maxEpoch(mods.map((mod) => mod.updated_epoch)))}`;
}

export function groupTranslationSummary(mods: ModRow[]): string {
  const appliedCount = mods.filter((mod) => mod.translation_applied).length;
  const patchCount = mods.reduce((sum, mod) => sum + mod.translation_patch_count, 0);
  const activePatchCount = mods.reduce((sum, mod) => sum + mod.translation_patch_active_count, 0);
  if (appliedCount > 0) return `적용됨 ${appliedCount}`;
  if (activePatchCount > 0) return `번역모드 활성 ${activePatchCount}`;
  if (patchCount > 0) return `번역모드 ${patchCount}`;
  return "미적용";
}

export function compactTranslationApplyDate(mods: ModRow[]): string {
  const latest = maxEpoch(mods.map((mod) => mod.translation_applied_epoch));
  if (latest) return formatShortDate(latest);
  const patchCount = mods.reduce((sum, mod) => sum + mod.translation_patch_count, 0);
  return patchCount > 0 ? "별도 번역 모드 있음" : "직접 적용 없음";
}

export function formatShortDate(epoch: number | null): string {
  if (!epoch) {
    return "-";
  }
  return new Date(epoch * 1000).toLocaleDateString("ko-KR", {
    year: "2-digit",
    month: "2-digit",
    day: "2-digit",
  });
}

function formatDateToken(epoch: number | null): string {
  if (!epoch) {
    return "";
  }
  return new Date(epoch * 1000).toISOString().slice(0, 10);
}

export function formatFullDateTime(epoch: number | null): string {
  if (!epoch) {
    return "-";
  }
  return new Date(epoch * 1000).toLocaleString("ko-KR");
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(value >= 10 ? 1 : 2)} ${units[index]}`;
}

function maxEpoch(values: Array<number | null>): number | null {
  const numbers = values.filter((value): value is number => typeof value === "number" && value > 0);
  return numbers.length > 0 ? Math.max(...numbers) : null;
}

export function joinResourcePath(basePath: string, name: string): string {
  if (!basePath) {
    return name;
  }
  if (basePath === "res://") {
    return `${basePath}${name}`;
  }
  return `${basePath.replace(/\/$/, "")}/${name}`;
}

export function languageResourceRoot(samplePath: string): string {
  const normalized = samplePath.replace(/\\/g, "/").replace(/\/$/, "");
  const parts = normalized.split("/");
  const last = parts[parts.length - 1] ?? "";
  if (/\.(json|loc)$/i.test(last)) {
    parts.pop();
  }
  return parts.join("/");
}

export function defaultTranslationResourcePath(mod: ModRow): string {
  const languages = mod.language_preview ?? [];
  const found = recommendedSourceLanguage(languages) ?? languages.find((language) => language.sample_path) ?? null;
  return found ? languageResourceRoot(found.sample_path) : "";
}

export function defaultTranslatableResourcePath(mod: ModRow): string {
  return defaultTranslationResourcePath(mod) || firstHardcodedResourcePath(mod.extraction_tree);
}

export function isHardcodedResourcePath(path: string): boolean {
  return /\.(dll|exe)$/i.test(path.replace(/\\/g, "/").split("/").pop() ?? "");
}

export function translationTargetOptions(current: string, settingsTarget: string, languages: LanguagePreview[]): Array<{ code: string; label: string }> {
  const common = ["kor", "eng", "zhs", "zht", "jpn", "rus"];
  const codes = [
    current,
    settingsTarget,
    ...common,
    ...languages.map(languageFolderCode),
  ]
    .map((code) => normalizeLanguageTag(code))
    .filter(Boolean);
  return Array.from(new Set(codes)).map((code) => ({ code, label: languageLabel(code) }));
}

export function parentPath(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/$/, "");
  if (!normalized) {
    return path;
  }
  if (!/\.[^/]+$/.test(normalized.split("/").pop() ?? "")) {
    return path;
  }
  return normalized.slice(0, normalized.lastIndexOf("/")) || normalized;
}

export function languageResourceName(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/$/, "");
  return normalized.split("/").pop() || normalized || "localization";
}

function firstHardcodedResourcePath(nodes: ExtractionTreeNode[] = []): string {
  for (const node of nodes) {
    if (node.kind === "hardcoded" && node.path) {
      return node.path;
    }
    const child = firstHardcodedResourcePath(node.children);
    if (child) {
      return child;
    }
  }
  return "";
}

export function hasLocalizationBranch(node: ExtractionTreeNode): boolean {
  if (node.name.toLowerCase() === "localization" || node.kind === "hardcoded") {
    return true;
  }
  return node.children.some(hasLocalizationBranch);
}

export function languageFolderCode(language: LanguagePreview): string {
  return language.sample_path.replace(/\\/g, "/").match(/(?:^|\/)localization\/([^/]+)/i)?.[1] || language.code || language.label;
}

export function languageLabel(code: string): string {
  const normalized = normalizeLanguageTag(code);
  switch (normalized) {
    case "kor":
      return "한국어";
    case "eng":
    case "en":
      return "English";
    case "jpn":
    case "ja":
      return "日本語";
    case "zhs":
    case "zh-cn":
      return "简体中文";
    case "zht":
    case "zh-tw":
      return "繁體中文";
    default:
      return code || "언어";
  }
}
