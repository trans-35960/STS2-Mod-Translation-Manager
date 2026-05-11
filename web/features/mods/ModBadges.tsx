import { StatusBadge } from "../../components/Common";
import type { LanguagePreview } from "../../types";
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
