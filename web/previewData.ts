import type {
  Dashboard,
  GameLog,
  JsonSheetReport,
  JsonTranslationEntry,
  JsonTranslationSheet,
  JsonValidation,
  ModErrorReport,
  TranslationProjectInfo,
} from "./types";
export const previewDashboard: Dashboard = {
  paths: {
    workspace: "Z:/game/sts2/modmanager",
    game: "Z:/game/sts2",
    game_mods: "Z:/game/sts2/mods",
    save_dir: "C:/Users/angel/AppData/Roaming/SlayTheSpire2/steam/76561198093641030",
    save_backup: "Z:/game/sts2/modmanager/backups",
    disabled: "Z:/game/sts2/mods.disabled",
    presets: "Z:/game/sts2/modmanager/presets",
    translation_work: "Z:/game/sts2/modmanager/translation_work",
    state: "Z:/game/sts2/modmanager/state",
    vendor: "Z:/game/sts2/modmanager/vendor",
    external_manager_dirs: ["Vortex/downloads/slaythespire2", "Nexus Mod Manager/SlayTheSpire2/Mods"],
    workshop_dirs: ["G:/Game/Steam/steamapps/workshop/content/2868840"],
  },
  settings: {
    translation_work_dir: "Z:/game/sts2/modmanager/translation_work",
    target_language: "kor",
      game_exe_path: "",
      game_log_path: "C:/Users/angel/AppData/Roaming/SlayTheSpire2/logs/godot.log",
      save_dir: "C:/Users/angel/AppData/Roaming/SlayTheSpire2/steam/76561198093641030",
      save_backup_dir: "Z:/game/sts2/modmanager/backups",
      save_backup_retention_days: 7,
      save_backup_max_entries: 14,
      deleted_retention_days: 30,
      mod_view_mode: "detail",
  },
  stats: {
    active_mods: 2,
    inactive_mods: 2,
    disabled_mods: 5,
    external_mods: 3,
    workshop_mods: 0,
    presets: 2,
    translations: 1,
    detected_changes: 2,
    vanilla_safe: false,
  },
  setup_issues: [
    { field: "game_exe_path", message: "게임 실행 파일 경로가 설정되지 않았습니다.", blocking: true },
    { field: "save_backup_max_entries", message: "최대 백업 수를 확인하세요.", blocking: true }
  ],
  diagnostics: [
    {
      id: "preview-install-archive",
      category: "install",
      severity: "warn",
      title: "압축 파일이 mods 폴더에 그대로 있습니다",
      detail: "korean-font-fix.zip은 폴더로 풀고 원본 압축은 백업하는 편이 안전합니다.",
      action_label: "설치 자동 정리",
      related_path: "Z:/game/sts2/mods/korean-font-fix.zip",
      mod_key: "korean-font-fix",
      can_auto_fix: true,
    },
    {
      id: "preview-log-mod",
      category: "log",
      severity: "error",
      title: "모드 로딩 실패 로그가 있습니다",
      detail: "[Error] zsproject 모드를 불러오던 중 ReflectionTypeLoadException 예외가 발생했습니다.",
      action_label: "로그 위치 열기",
      related_path: "C:/Users/angel/AppData/Roaming/SlayTheSpire2/logs/godot.log",
      mod_key: null,
      can_auto_fix: false,
    },
  ],
  mods: [
    {
      key: "korean-font-fix",
      name: "Korean Font Fix",
      manifest_id: "korean-font-fix",
      group_name: "Korean Font Fix",
      active: true,
      managed: true,
      external: false,
      workshop: false,
      workshop_item_id: null,
      source_label: "게임 폴더",
      kind: "archive",
      version_hint: "v1.2.0",
      bytes: 812339,
      modified_epoch: 1777730000,
      registered_epoch: 1777200000,
      updated_epoch: 1777730000,
      path: "Z:/game/sts2/mods/korean-font-fix.zip",
      update_state: "clean",
      change_reasons: [],
      previous_version: null,
      translation_state: "추출 가능",
      translation_applied: true,
      translation_applied_epoch: 1777740000,
      translation_patch_count: 0,
      translation_patch_active_count: 0,
      translation_patch_names: [],
      needs_recheck: false,
      translation_review_required: false,
      safety_warnings: [],
      extraction_hint: "Windows ZIP 추출 사용",
      extraction_source_path: "Z:/game/sts2/mods/korean-font-fix.zip",
      extraction_target: "한국어, English 언어 파일 2개",
      is_translation_patch: false,
      translation_target_id: null,
      translation_target_key: null,
      translation_target_name: null,
      translation_target_version: null,
      dependencies: [],
      language_preview: [
        { code: "ko", label: "한국어", files: 2, sample_path: "localization/ko.json" },
        { code: "en", label: "English", files: 2, sample_path: "localization/en.json" },
      ],
      extraction_tree: [
        {
          name: "localization",
          path: "",
          kind: "dir",
          children: [
            { name: "ko.json", path: "localization/ko.json", kind: "language", children: [] },
            { name: "en.json", path: "localization/en.json", kind: "language", children: [] },
          ],
        },
      ],
    },
    {
      key: "relic-expansion",
      name: "Relic Expansion",
      manifest_id: "relic-expansion",
      group_name: "Relic Expansion",
      active: false,
      managed: true,
      external: true,
      workshop: false,
      workshop_item_id: null,
      source_label: "Nexus/Vortex",
      kind: "archive",
      version_hint: "v0.9.4",
      bytes: 1823012,
      modified_epoch: 1777730100,
      registered_epoch: 1777100000,
      updated_epoch: 1777730100,
      path: "Z:/game/sts2/mods.disabled/relic-expansion.zip",
      update_state: "updated",
      change_reasons: ["파일 크기", "수정일"],
      previous_version: null,
      translation_state: "추출 가능",
      translation_applied: false,
      translation_applied_epoch: null,
      translation_patch_count: 0,
      translation_patch_active_count: 0,
      translation_patch_names: [],
      needs_recheck: true,
      translation_review_required: false,
      safety_warnings: [],
      extraction_hint: "업데이트 후 번역 검토 필요",
      extraction_source_path: "Z:/game/sts2/mods.disabled/relic-expansion.zip",
      extraction_target: "English 언어 파일 1개",
      is_translation_patch: false,
      translation_target_id: null,
      translation_target_key: null,
      translation_target_name: null,
      translation_target_version: null,
      dependencies: [{ id: "BaseLib", key: "baselib", name: "BaseLib", active: true, available: true, version_required: "3.3.0", version_minimum: true, version_current: "v3.4.7", version_matches: true }],
      compat_issue: {
        game_version: "v0.111.0",
        min_game_version: null,
        missing: [
          {
            assembly: "sts2",
            kind: "method",
            display: "CreatureCmd.Damage(PlayerChoiceContext, Creature, decimal, ValueProp, Creature, CardModel)",
            available: ["CreatureCmd.Damage(PlayerChoiceContext, Creature, decimal, ValueProp, Creature, CardModel, CardPlay)"],
            message: "CreatureCmd.Damage(PlayerChoiceContext, Creature, decimal, ValueProp, Creature, CardModel) — 게임에서 시그니처가 바뀜",
          },
          {
            assembly: "sts2",
            kind: "type",
            display: "SavedPropertiesTypeCache",
            available: [],
            message: "SavedPropertiesTypeCache 타입 — 게임에 없음",
          },
        ],
        message: "현재 게임에 없는 코드 2개를 사용합니다. 해당 기능이 쓰이는 순간 게임이 오류로 튕길 수 있습니다.",
      },
      language_preview: [
        { code: "en", label: "English", files: 4, sample_path: "localization/en/relics.json" },
      ],
      extraction_tree: [
        {
          name: "localization",
          path: "",
          kind: "dir",
          children: [
            {
              name: "en",
              path: "",
              kind: "dir",
              children: [
                { name: "relics.json", path: "localization/en/relics.json", kind: "language", children: [] },
              ],
            },
          ],
        },
      ],
    },
    {
      key: "challenge-pack",
      name: "Challenge Pack",
      manifest_id: "challenge-pack",
      group_name: "Challenge Pack",
      active: false,
      managed: false,
      external: true,
      workshop: false,
      workshop_item_id: null,
      source_label: "Nexus/Vortex",
      kind: "archive",
      version_hint: "v2.1.0",
      bytes: 992012,
      modified_epoch: 1777730200,
      registered_epoch: 1777730200,
      updated_epoch: 1777730200,
      path: "Z:/downloads/challenge-pack.7z",
      update_state: "new",
      change_reasons: ["새로 감지"],
      previous_version: null,
      translation_state: "도구 필요",
      translation_applied: false,
      translation_applied_epoch: null,
      translation_patch_count: 0,
      translation_patch_active_count: 0,
      translation_patch_names: [],
      needs_recheck: false,
      translation_review_required: false,
    safety_warnings: [],
      extraction_hint: "vendor/7zip/7z.exe가 있으면 추출 가능",
      extraction_source_path: "Z:/downloads/challenge-pack.7z",
      extraction_target: "압축 내부 언어 파일, 7-Zip 도구 필요",
      is_translation_patch: false,
      translation_target_id: null,
      translation_target_key: null,
      translation_target_name: null,
      translation_target_version: null,
      dependencies: [{ id: "MissingLib", key: null, name: "MissingLib", active: false, available: false, version_required: null, version_current: null, version_matches: null }],
      dll_files: ["Z:/downloads/challenge-pack/ChallengePack.dll"],
      language_preview: [],
      extraction_tree: [
        {
          name: "ChallengePack",
          path: "",
          kind: "dir",
          children: [
            { name: "ChallengePack.dll", path: "ChallengePack/ChallengePack.dll", kind: "file", children: [] },
            { name: "ChallengePack.pck", path: "ChallengePack/ChallengePack.pck", kind: "file", children: [] },
          ],
        },
      ],
    },
  ],
  presets: [
    {
      name: "korean-balanced",
      mod_count: 2,
      mods: [
        { key: "korean-font-fix", version_hint: "v1.2.0", bytes: 812339, modified_epoch: 1777730000, file_name: "korean-font-fix.zip" },
        { key: "relic-expansion", version_hint: "v0.9.4", bytes: 1823012, modified_epoch: 1777730100, file_name: "relic-expansion.zip" },
      ],
    },
    {
      name: "vanilla-plus",
      mod_count: 1,
      mods: [
        { key: "korean-font-fix", version_hint: "v1.2.0", bytes: 812339, modified_epoch: 1777730000, file_name: "korean-font-fix.zip" },
      ],
    },
  ],
  translations: [
    {
      mod_key: "relic-expansion",
      version_id: "v0.9.4-1777730100",
      review_required: true,
      path: "Z:/game/sts2/modmanager/translation_work/relic-expansion/v0.9.4-1777730100",
    },
  ],
  deleted_mods: [
    {
      id: "preview-deleted-1",
      key: "old-mod",
      name: "Old Deleted Mod",
      original_path: "Z:/game/sts2/mods/Old Deleted Mod.zip",
      backup_path: "Z:/game/sts2/modmanager/state/deleted_mods/preview/Old Deleted Mod.zip",
      deleted_epoch: Math.floor(Date.now() / 1000) - 86400,
      expires_epoch: Math.floor(Date.now() / 1000) + 29 * 86400,
      bytes: 52428800,
    },
  ],
  save_backups: [
    {
      id: "vanilla:1777731000",
      kind: "vanilla",
      kind_label: "바닐라",
      created_epoch: 1777731000,
      path: "Z:/game/sts2/modmanager/backups/vanilla/1777731000",
      bytes: 20480,
    },
    {
      id: "modded:1777731001",
      kind: "modded",
      kind_label: "모드",
      created_epoch: 1777731001,
      path: "Z:/game/sts2/modmanager/backups/modded/1777731001",
      bytes: 24576,
    },
  ],
  cache_usage: {
    bytes: 482344960,
    files: 1842,
    dirs: 96,
  },
  tools: [
    {
      name: "7-Zip",
      available: false,
      purpose: "extract .7z and .rar archives",
      expected_path: "Z:/game/sts2/modmanager/vendor/7zip/7z.exe",
    },
    {
      name: "GodotPCKExplorer.Console",
      available: false,
      purpose: "extract and repack Godot .pck files",
      expected_path: "Z:/game/sts2/modmanager/vendor/godot-pck-explorer/GodotPCKExplorer.Console.exe",
    },
  ],
  launch: {
    ready: false,
    game_exe: null,
    steam_exe: null,
    target_label: "not found",
    running: false,
    game_branch: null,
    game_version: null,
  },
  bridge: {
    installed: true,
    active: true,
    installed_version: "0.2.2",
    bundled_version: "0.2.2",
    shared_dir: "C:/Users/preview/AppData/Roaming/STS2ModTranslationManager/shared",
    status: null,
  },
};

previewDashboard.mods.push(
  {
    ...previewDashboard.mods[1],
    key: "sts2modmanagerbridge",
    name: "STS2ModManagerBridge",
    manifest_id: "STS2ModManagerBridge",
    group_name: "STS2ModManagerBridge",
    active: true,
    source_label: "게임 mods",
    path: "Z:/game/sts2/mods/STS2ModManagerBridge",
    update_state: "clean",
    change_reasons: [],
    needs_recheck: false,
    compat_issue: null,
    dependencies: [],
    system_mod: true,
    language_preview: [],
    extraction_tree: [],
  },
  {
    ...previewDashboard.mods[1],
    key: "workshop:3747559888",
    name: "CuteSakikoMod",
    manifest_id: "CuteSakikoMod",
    group_name: "CuteSakikoMod",
    active: true,
    workshop: true,
    workshop_item_id: "3747559888",
    workshop_title: "Cute Sakiko Mod",
    source_label: "창작마당",
    path: "Z:/steam/workshop/content/2868840/3747559888",
    update_state: "clean",
    change_reasons: [],
    needs_recheck: false,
    compat_issue: null,
    dependencies: [],
    dll_files: ["Z:/steam/workshop/content/2868840/3747559888/CuteSakikoMod.dll"],
    bridge_stored: [{ language: "kor", entries: 812, dll_strings: 40 }],
    language_preview: [{ code: "zhs", label: "Chinese", files: 6, keys: 900, sample_path: "CuteSakikoMod/localization/zhs/cards.json" }],
  },
);

export const previewModErrors: ModErrorReport = {
  log_path: "C:/Users/me/AppData/Roaming/SlayTheSpire2/logs/godot.log",
  exists: true,
  modified_epoch: 1777730400,
  errors: [
    {
      id: "2086-a",
      mod_key: "workshop:3747559888",
      matched_by: "path",
      message: "No loader found for resource: res://CuteSakikoMod/images/ui/map/StarAnon_outline.png (expected type: unknown)",
      text: `ERROR: No loader found for resource: res://CuteSakikoMod/images/ui/map/StarAnon_outline.png (expected type: unknown)
   at: _load (core/io/resource_loader.cpp:328)
   C# backtrace (most recent call first):
       [0] Godot.Resource <unknown>.MegaCrit.Sts2.Core.Assets.AssetCache.LoadAsset_Patch1(MegaCrit.Sts2.Core.Assets.AssetCache, string)`,
      count: 4,
      first_line: 2086,
      patched_game_code: true,
    },
    {
      id: "11289-b",
      mod_key: null,
      matched_by: null,
      message: "System.NullReferenceException: Object reference not set to an instance of an object.",
      text: `[ERROR] System.NullReferenceException: Object reference not set to an instance of an object.
   at MegaCrit.Sts2.Core.Hooks.Hook.AfterEnergyReset_Patch3(ICombatState combatState, Player player)`,
      count: 2,
      first_line: 11289,
      patched_game_code: true,
    },
  ],
};

export const previewGameLogs: GameLog[] = [
  {
    path: "C:/Users/angel/AppData/Roaming/SlayTheSpire2/logs/godot.log",
    exists: true,
    modified_epoch: Math.floor(Date.now() / 1000),
    bytes: 147538872,
    lines: [
      "[ModLoader] Loaded mods: BaseLib, ModManagerSettings, AscensionUnlockMod, Unified Save Path",
      "[Error] zsproject 모드를 불러오던 중 ReflectionTypeLoadException 예외가 발생했습니다.",
      "[Hint] The Reaper zip은 활성화되어 있지만 게임 로더에서 ZSproject.dll 로딩에 실패했습니다.",
    ],
  },
];

export const previewJsonReport: JsonSheetReport = {
  sheet_path: "Z:/game/sts2/modmanager/translation_work/json_sheets/cards.kor.translation.json",
  entries: 4,
  new_entries: 1,
  updated_entries: 1,
  missing_entries: 1,
  removed_entries: 0,
};

const basePreviewJsonSheet: JsonTranslationSheet = {
  source_path: "Z:/game/sts2/mods/sample/localization/eng",
  target_language: "kor",
  updated_epoch: 1777730000,
  entries: [
    {
      key: "file://cards.json#/cards/strike/name",
      source_value: "Strike",
      translated_value: "타격",
      status: "ready",
    },
    {
      key: "file://cards.json#/cards/strike/description",
      source_value: "Deal 6 damage.",
      translated_value: "피해를 6 줍니다.",
      status: "ready",
    },
    {
      key: "file://cards.json#/cards/guard/name",
      source_value: "Guard",
      translated_value: "",
      status: "new",
    },
    {
      key: "file://powers.json#/bash/description",
      previous_source_value: "Apply 1 [gold]Weak[/gold].",
      source_value: "Apply 2 Vulnerable.",
      translated_value: "[gold]취약을[/gold] 2 부여합니다.",
      status: "updated",
    },
  ],
};

// `?previewEntries=3000` fills the browser preview with a large synthetic sheet
// so scrolling and typing performance can be checked without the desktop app.
export const previewJsonSheet: JsonTranslationSheet = withPreviewEntries(basePreviewJsonSheet);

function withPreviewEntries(sheet: JsonTranslationSheet): JsonTranslationSheet {
  const requested = Number(
    typeof window === "undefined" ? 0 : new URLSearchParams(window.location.search).get("previewEntries"),
  );
  if (!Number.isFinite(requested) || requested <= sheet.entries.length) {
    return sheet;
  }
  const extra = Array.from({ length: requested - sheet.entries.length }, (_, index): JsonTranslationEntry => {
    const file = `file://generated_${Math.floor(index / 400)}.json`;
    const longText = index % 7 === 0;
    return {
      key: `${file}#/entries/item_${index}/description`,
      source_value: longText
        ? `Deal {Damage} damage to ALL enemies.NLApply [gold]${index % 5}[/gold] Weak. ${"Lorem ipsum dolor sit amet. ".repeat(4)}`
        : `Generated preview text ${index}.`,
      translated_value: index % 3 === 0 ? "" : `생성된 미리보기 문장 ${index}.`,
      status: index % 3 === 0 ? "new" : "ready",
    };
  });
  return { ...sheet, entries: [...sheet.entries, ...extra] };
}

export const previewJsonValidation: JsonValidation = {
  valid: false,
  total_entries: 325,
  missing_entries: ["file://cards.json#/cards/guard/name"],
  updated_entries: ["file://powers.json#/bash/description"],
  removed_entries: [],
  glossary_issues: [
    {
      key: "file://powers.json#/bash/description",
      kind: "glossary",
      message: "용어집: 'Vulnerable' → '취약함' · 치환: 취약 → 취약함",
      original: "[gold]취약을[/gold] 2 부여합니다.",
      suggestion: "[gold]취약함을[/gold] 2 부여합니다.",
    },
    {
      key: "file://powers.json#/bash/description",
      kind: "josa",
      message: "조사가 강조 태그 안에 있습니다: [gold]취약을[/gold] → [gold]취약[/gold]을",
      original: "[gold]취약을[/gold] 2 부여합니다.",
      suggestion: "[gold]취약[/gold]을 2 부여합니다.",
    },
  ],
  format_issues: [
    {
      key: "file://cards.json#/cards/strike/description",
      kind: "bang_tokens",
      message: "느낌표 토큰 구성이 원본과 다릅니다.",
    },
    {
      key: "file://powers.json#/bash/description",
      kind: "angle_tags",
      message: "꺾쇠 태그 구성이 원본과 다릅니다.",
    },
    ...Array.from({ length: 12 }, (_, index) => ({
      key: `file://cards.json#/cards/guard/${index % 2 ? "description" : "name"}`,
      kind: index % 2 ? "line_break_marker" : "square_tags",
      message: index % 2 ? "NL 줄바꿈 마커 수가 다릅니다." : "대괄호 태그 구성이 원본과 다릅니다.",
    })),
    // With `?previewEntries`, point one warning far down the sheet so jumping
    // to an entry that is not rendered yet can be exercised.
    ...previewJsonSheet.entries.slice(-12, -11).map((entry) => ({
      key: entry.key,
      kind: "placeholders",
      message: "플레이스홀더 구성이 원본과 다릅니다.",
    })),
  ],
};

export const previewTranslationProject: TranslationProjectInfo = {
  modKey: "sample-character",
  modPath: "Z:/game/sts2/mods/Sample Character.zip",
  modName: "Sample Character",
  version: "v1.0.0",
  author: "Preview",
  description: "Preview translation project.",
  languages: [
    { code: "eng", label: "English", files: 2, sample_path: "res://Sample/localization/eng/cards.json" },
    { code: "zhs", label: "简体中文", files: 2, sample_path: "res://Sample/localization/zhs/cards.json" },
  ],
};
