import type { JsonTranslationEntry, JsonTranslationSheet, LanguageCompareValue, LanguagePreview } from "../../types";

export type TranslationProjectNode = {
  name: string;
  path: string;
  filterPath: string;
  total: number;
  ready: number;
  attention: number;
  children: TranslationProjectNode[];
};

export type StructuredTranslationEntry = {
  id: string;
  translated_value: string;
  compact?: boolean;
  source?: string;
};

export type TranslationSlot = {
  file: string;
  compactFile: string;
  id: string;
  entry: JsonTranslationEntry;
};

export function createCompareValueMap(values: LanguageCompareValue[]) {
  const mapped: Record<string, string> = {};
  for (const entry of values) {
    mapped[entry.key] = entry.value;
    mapped[normalizedLocalizationKey(entry.key)] = entry.value;
    mapped[stableCompareKey(entry.key)] = entry.value;
  }
  return mapped;
}

export function buildTranslationProjectTree(sheet: JsonTranslationSheet | null): TranslationProjectNode | null {
  if (!sheet) {
    return null;
  }
  const root: TranslationProjectNode = {
    name: projectNameFromPath(sheet.source_path),
    path: sheet.source_path,
    filterPath: "",
    total: 0,
    ready: 0,
    attention: 0,
    children: [],
  };
  for (const entry of sheet.entries) {
    const filePath = sheetEntryFilePath(entry.key);
    const parts = filePath.split("/").filter(Boolean);
    let node = root;
    for (const part of parts) {
      let child = node.children.find((item) => item.name === part);
      if (!child) {
        child = {
          name: part,
          path: node.path ? `${node.path}/${part}` : part,
          filterPath: node.filterPath ? `${node.filterPath}/${part}` : part,
          total: 0,
          ready: 0,
          attention: 0,
          children: [],
        };
        node.children.push(child);
        node.children.sort((left, right) => left.name.localeCompare(right.name));
      }
      node = child;
    }
    incrementProjectNode(node, entry);
  }
  rollupProjectNode(root);
  return bestLocalizationRoot(root) ?? root;
}

export function bestLocalizationRoot(root: TranslationProjectNode): TranslationProjectNode | null {
  const candidates: TranslationProjectNode[] = [];
  collectLocalizationNodes(root, candidates);
  return candidates.sort(
    (left, right) =>
      right.total - left.total ||
      right.ready - left.ready ||
      left.filterPath.localeCompare(right.filterPath),
  )[0] ?? null;
}

export function collectLocalizationNodes(node: TranslationProjectNode, output: TranslationProjectNode[]) {
  if (node.name.toLowerCase() === "localization") {
    output.push(node);
  }
  for (const child of node.children) {
    collectLocalizationNodes(child, output);
  }
}

export function sheetEntryFilePath(key: string): string {
  return splitSheetKey(key).file;
}

export function splitSheetKey(key: string): { file: string; key: string } {
  if (!key.startsWith("file://")) {
    return { file: "source.json", key };
  }
  const withoutScheme = key.slice("file://".length);
  const separator = withoutScheme.indexOf("#");
  const file = separator === -1 ? withoutScheme : withoutScheme.slice(0, separator);
  const entryKey = separator === -1 ? "" : withoutScheme.slice(separator + 1);
  return {
    file: file || "source.json",
    key: entryKey,
  };
}

export function languageCodeFromSheetKey(key: string): string {
  return splitSheetKey(key).file
    .replace(/\\/g, "/")
    .match(/(?:^|\/)localization\/([^/]+)/i)?.[1] ?? "";
}

export function languageCodeFromSourcePath(path: string): string {
  return path.replace(/\\/g, "/").match(/(?:^|\/)localization\/([^/]+)/i)?.[1] ?? "";
}

export function normalizedLocalizationKey(key: string): string {
  const { file, key: entryKey } = splitSheetKey(key);
  const normalizedFile = file.replace(/\\/g, "/").replace(/(^|\/)localization\/[^/]+/i, "$1localization/{language}");
  return `file://${normalizedFile}#${entryKey}`;
}

export function stableCompareKey(key: string): string {
  const { file, key: entryKey } = splitSheetKey(key);
  const normalizedFile = file.replace(/\\/g, "/");
  const parts = normalizedFile.split("/").filter(Boolean);
  const localizationIndex = parts.findIndex((part) => part.toLowerCase() === "localization");
  const compactFile =
    localizationIndex >= 0 && localizationIndex + 2 < parts.length
      ? parts.slice(localizationIndex + 2).join("/")
      : normalizedFile;
  return `${compactFile}#${entryKey}`;
}

export function pathMatchesProjectNode(filePath: string, activePath: string): boolean {
  if (!activePath) {
    return true;
  }
  const normalizedFile = filePath.replace(/\\/g, "/").replace(/^\/+/, "");
  const normalizedActive = activePath.replace(/\\/g, "/").replace(/^\/+/, "");
  return normalizedFile === normalizedActive || normalizedFile.startsWith(`${normalizedActive}/`);
}

export function parsePastedTranslationJson(text: string): unknown | null {
  const trimmed = text.trim();
  if (!trimmed) {
    return null;
  }
  const candidates = [trimmed];
  const fenced = trimmed.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/i);
  if (fenced) {
    candidates.push(fenced[1].trim());
  }
  for (const candidate of candidates) {
    try {
      return JSON.parse(candidate);
    } catch {
      // Try the next clipboard-friendly shape.
    }
  }
  return null;
}

export function looksLikeJsonPaste(text: string): boolean {
  const trimmed = text.trim();
  return trimmed.startsWith("{") || trimmed.startsWith("[") || /^```(?:json)?\s*[\[{]/i.test(trimmed);
}

export function isStructuredTranslationJsonPaste(text: string): boolean {
  const parsed = parsePastedTranslationJson(text);
  if (parsed === null) {
    return false;
  }
  return structuredTranslationEntries(parsed).some((entry) => isTranslationSlotId(entry.id));
}

export function isTabularTranslationPaste(text: string): boolean {
  const rows = text.replace(/\r/g, "").split("\n").filter((line) => line.length > 0);
  return rows.length > 0 && rows.every((line) => line.includes("\t"));
}

export function structuredTranslationEntries(value: unknown): StructuredTranslationEntry[] {
  if (!value || typeof value !== "object") {
    return [];
  }
  const entries = Array.isArray(value) ? value : (value as { entries?: unknown }).entries;
  if (!Array.isArray(entries)) {
    return compactTranslationEntries(value);
  }
  return entries
    .map((entry): StructuredTranslationEntry | null => {
      if (!entry || typeof entry !== "object") {
        return null;
      }
      const record = entry as { id?: unknown; translated_value?: unknown; translation?: unknown; translated?: unknown; source?: unknown; file?: unknown };
      const id = typeof record.id === "string" ? record.id : "";
      const source = typeof record.source === "string" ? record.source : typeof record.file === "string" ? record.file : undefined;
      const translated =
        typeof record.translated_value === "string"
          ? record.translated_value
          : typeof record.translation === "string"
            ? record.translation
            : typeof record.translated === "string"
              ? record.translated
              : "";
      if (!id) {
        return null;
      }
      return source ? { id, translated_value: translated, source } : { id, translated_value: translated };
    })
    .filter((entry): entry is StructuredTranslationEntry => Boolean(entry));
}

export function compactTranslationEntries(value: unknown): StructuredTranslationEntry[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return [];
  }
  const result: StructuredTranslationEntry[] = [];
  for (const [key, entryValue] of Object.entries(value as Record<string, unknown>)) {
    if (typeof entryValue === "string" && isTranslationSlotId(key)) {
      result.push({ id: key, translated_value: entryValue, compact: true });
      continue;
    }
    if (!entryValue || typeof entryValue !== "object" || Array.isArray(entryValue)) {
      continue;
    }
    for (const [nestedKey, nestedValue] of Object.entries(entryValue as Record<string, unknown>)) {
      if (typeof nestedValue === "string" && isTranslationSlotId(nestedKey)) {
        result.push({ id: nestedKey, translated_value: nestedValue, compact: true, source: key });
      }
    }
  }
  return result;
}

export function isTranslationSlotId(value: string): boolean {
  return /^k\d+-[0-9a-z]{2}$/i.test(value);
}

export function translationSlotEntries(sheet: JsonTranslationSheet): TranslationSlot[] {
  const fileCounts = new Map<string, number>();
  for (const entry of sheet.entries) {
    if (!isTranslatableEntry(entry)) {
      continue;
    }
    const file = splitSheetKey(entry.key).file;
    const compactFile = compactTranslationFile(file);
    fileCounts.set(compactFile, (fileCounts.get(compactFile) ?? 0) + 1);
  }
  const fileIndexes = new Map<string, number>();
  return sheet.entries.flatMap((entry) => {
    if (!isTranslatableEntry(entry)) {
      return [];
    }
    const file = splitSheetKey(entry.key).file;
    const compactFile = compactTranslationFile(file);
    const index = (fileIndexes.get(compactFile) ?? 0) + 1;
    fileIndexes.set(compactFile, index);
    const width = Math.max(3, String(fileCounts.get(compactFile) ?? 0).length);
    return [{
      file,
      compactFile,
      id: isTranslationSlotId(entry.slot_id ?? "") ? entry.slot_id! : translationSlotId(index, width, entry.key),
      entry,
    }];
  });
}

export function compactTranslationFile(file: string): string {
  const normalized = file.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  const index = parts.findIndex((part) => part.toLowerCase() === "localization");
  if (index < 0 || index + 2 >= parts.length) {
    return normalized;
  }
  return parts.slice(index + 2).join("/");
}

export function translationSlotId(index: number, width: number, key: string): string {
  return `k${String(index).padStart(width, "0")}-${slotChecksum(stableSlotKey(key))}`;
}

export function translationSlotKey(file: string, id: string): string {
  return `${file}\u0000${id}`;
}

export function stableSlotKey(key: string): string {
  const { file, key: entryKey } = splitSheetKey(key);
  return `${compactTranslationFile(file)}#${entryKey}`;
}

export function slotChecksum(stableKey: string): string {
  const hash = fnv64(stableKey);
  return (hash % 1296n).toString(36).padStart(2, "0");
}

export function fnv64(value: string): bigint {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= BigInt(byte);
    hash = (hash * prime) & mask;
  }
  return hash;
}

export function isTranslatableEntry(entry: JsonTranslationEntry): boolean {
  return entry.status !== "removed" && entry.source_value.trim().length > 0;
}

export function hasTranslationValue(value: string): boolean {
  return value.length > 0;
}

export function whitespaceValueLabel(value: string): string | null {
  return value.length > 0 && value.trim().length === 0 ? `공백 ${value.length}자` : null;
}

export function languageFolderCode(language: LanguagePreview): string {
  const fromPath = language.sample_path
    .replace(/\\/g, "/")
    .match(/(?:^|\/)localization\/([^/]+)/i)?.[1];
  return fromPath || language.code;
}

export function languageLabel(code: string): string {
  const labelsByCode: Record<string, string> = {
    kor: "한국어",
    ko: "한국어",
    eng: "English",
    en: "English",
    zhs: "简体中文",
    "zh-cn": "简体中文",
    zht: "繁體中文",
    "zh-tw": "繁體中文",
    jpn: "日本語",
    ja: "日本語",
    rus: "Русский",
    ru: "Русский",
  };
  return labelsByCode[code] ?? code;
}

export function normalizeTranslationLanguageCode(code: string): string {
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

export function translationLanguagesMatch(left: string, right: string): boolean {
  return normalizeTranslationLanguageCode(left) === normalizeTranslationLanguageCode(right);
}

export function retargetTranslationSheetPath(path: string, language: string): string {
  if (!path || !language) {
    return path;
  }
  return path.replace(/\.([A-Za-z0-9_-]+)\.translation\.json$/i, `.${language}.translation.json`);
}

export function inferPckTargetPath(sheet: JsonTranslationSheet | null): string {
  if (!sheet) {
    return "";
  }
  const source = sheet.source_path.replace(/\\/g, "/");
  const entryFiles = Array.from(new Set(sheet.entries.map((entry) => splitSheetKey(entry.key).file).filter(Boolean)));
  const isDirectorySheet = entryFiles.length > 1 || !/\.[A-Za-z0-9_-]+$/.test(source.split("/").pop() ?? "");
  const afterSource = source.match(/\/source\/(.+)$/i)?.[1];
  if (afterSource) {
    const target = replaceLocalizationLanguageInPath(afterSource, sheet.target_language);
    return isDirectorySheet ? stripFileNameFromPckTarget(target) : target;
  }
  const firstFile = entryFiles[0];
  if (!firstFile) {
    return "";
  }
  const target = replaceLocalizationLanguageInPath(firstFile, sheet.target_language);
  return isDirectorySheet ? stripFileNameFromPckTarget(target) : target;
}

export function replaceLocalizationLanguageInPath(path: string, language: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  const index = parts.findIndex((part) => part.toLowerCase() === "localization");
  if (index >= 0 && index + 1 < parts.length) {
    parts[index + 1] = language;
  }
  return parts.join("/");
}

export function stripFileNameFromPckTarget(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  const last = parts[parts.length - 1] ?? "";
  if (/\.[A-Za-z0-9_-]+$/.test(last)) {
    parts.pop();
  }
  return parts.join("/");
}

export function incrementProjectNode(node: TranslationProjectNode, entry: JsonTranslationEntry) {
  if (entry.status === "removed") {
    return;
  }
  node.total += 1;
  if (entry.status === "ready" && hasTranslationValue(entry.translated_value)) {
    node.ready += 1;
  } else {
    node.attention += 1;
  }
}

export function rollupProjectNode(node: TranslationProjectNode) {
  for (const child of node.children) {
    rollupProjectNode(child);
    node.total += child.total;
    node.ready += child.ready;
    node.attention += child.attention;
  }
}

export function projectNameFromPath(path: string): string {
  return path.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? "translation";
}
