import { StatusBadge } from "../../components/Common";
import type { LanguagePreview, ModRow } from "../../types";
import {
  languageFolderCode,
  languageKeyCount,
  normalizeLanguageTag,
  recommendedSourceLanguage,
  representativeLanguage,
  uniqueLanguagePreviews,
} from "./modUtils";

export function RepresentativeLanguageBadge({ languages, fallback, targetLanguage }: { languages: LanguagePreview[]; fallback: string; targetLanguage: string }) {
  const unique = uniqueLanguagePreviews(languages);
  const selected = representativeLanguage(unique, targetLanguage);
  if (!selected) {
    return <StatusBadge>{fallback}</StatusBadge>;
  }
  const mainLanguage = recommendedSourceLanguage(unique);
  const code = languageFolderCode(selected);
  const keys = languageKeyCount(selected);
  const korean = ["kor", "ko"].includes(code.toLowerCase());
  const main = mainLanguage && selected.sample_path === mainLanguage.sample_path;
  const target = normalizeLanguageTag(code) === normalizeLanguageTag(targetLanguage);
  const badgeClass = ["language-badge", "single", korean ? "korean" : "", main ? "main" : "", target ? "target" : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={badgeClass} title={`${keys} keys / ${selected.files} files: ${selected.sample_path}`}>
      {code}
      <small>{keys}</small>
    </span>
  );
}

/** Languages the translation bridge holds for this mod and applies in game, next to the mod's own languages. */
export function BridgeLanguageBadges({ stored }: { stored: ModRow["bridge_stored"] }) {
  if (!stored || stored.length === 0) {
    return null;
  }
  return (
    <>
      {stored.map(({ language, entries, dll_strings }) => {
        const parts = [entries > 0 ? `텍스트 ${entries.toLocaleString()}개` : "", dll_strings > 0 ? `DLL 문자열 ${dll_strings.toLocaleString()}개` : ""];
        return (
          <span
            className="language-badge bridge"
            data-tooltip={`번역 브리지에 저장된 ${language} 번역 (${parts.filter(Boolean).join(" · ")}), 게임 안에서 적용됩니다`}
            key={language}
          >
            <em className="bridge">브리지</em>
            {language}
            <small>{entries + dll_strings}</small>
          </span>
        );
      })}
    </>
  );
}

export function LanguageBadges({ languages, fallback, targetLanguage }: { languages: LanguagePreview[]; fallback: string; targetLanguage: string }) {
  if (languages.length === 0) {
    return <StatusBadge>{fallback}</StatusBadge>;
  }
  const mainLanguage = recommendedSourceLanguage(languages);
  const normalizedTarget = normalizeLanguageTag(targetLanguage);
  return (
    <div className="language-badges">
      {languages.slice(0, 5).map((language) => {
        const code = languageFolderCode(language);
        const korean = ["kor", "ko"].includes(code.toLowerCase());
        const keys = languageKeyCount(language);
        const main = mainLanguage && language.sample_path === mainLanguage.sample_path;
        const target = normalizeLanguageTag(code) === normalizedTarget;
        const badgeClass = ["language-badge", korean ? "korean" : "", main ? "main" : "", target ? "target" : ""]
          .filter(Boolean)
          .join(" ");
        return (
          <span
            className={badgeClass}
            title={`${keys} keys / ${language.files} files: ${language.sample_path}`}
            key={`${language.code}-${language.sample_path}`}
          >
            {code}
            <small>{keys}</small>
            {target && <em className="me">ME</em>}
            {main && <em className="main">MAIN</em>}
          </span>
        );
      })}
      {languages.length > 5 && <span className="language-badge">+{languages.length - 5}</span>}
    </div>
  );
}
