use super::sheet::epoch_now;
use super::slots::translation_slot_entries;
use super::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[test]
fn carries_unchanged_values_and_marks_updates() {
    let fixture = TestDir::create("json_translation_updates");
    fixture.write_file("source.json", r#"{"a":"Hello","b":"Old"}"#);
    let sheet_path = fixture.path().join("sheet.json");

    create_or_update_sheet(&fixture.path().join("source.json"), "ko", None, &sheet_path)
        .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    for entry in &mut sheet.entries {
        if entry.key == "/a" {
            entry.translated_value = "안녕".to_string();
        }
    }
    write_sheet(&sheet_path, &sheet).expect("write edited sheet");

    fixture.write_file("source.json", r#"{"a":"Hello","b":"New","c":"Added"}"#);
    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "ko",
        Some(&sheet_path),
        &sheet_path,
    )
    .expect("update sheet");
    let updated = read_sheet(&sheet_path).expect("read updated sheet");

    assert!(updated.entries.iter().any(|entry| {
        entry.key == "/a"
            && entry.translated_value == "안녕"
            && entry.status == JsonTranslationStatus::Ready
    }));
    assert!(updated.entries.iter().any(|entry| {
        entry.key == "/b"
            && entry.status == JsonTranslationStatus::Updated
            && entry.previous_source_value.as_deref() == Some("Old")
            && entry.source_value == "New"
    }));
    assert!(
        updated
            .entries
            .iter()
            .any(|entry| { entry.key == "/c" && entry.status == JsonTranslationStatus::New })
    );
}

#[test]
fn different_target_languages_do_not_reuse_existing_sheet_values() {
    let fixture = TestDir::create("json_translation_separate_language_sheets");
    fixture.write_file("source.json", r#"{"name":"Strike"}"#);
    let source_path = fixture.path().join("source.json");
    let kor_sheet_path = fixture.path().join("cards.kor.translation.json");
    let eng_sheet_path = fixture.path().join("cards.eng.translation.json");

    create_or_update_sheet(&source_path, "kor", None, &kor_sheet_path).expect("create kor sheet");
    let mut kor_sheet = read_sheet(&kor_sheet_path).expect("read kor sheet");
    kor_sheet.entries[0].translated_value = "타격".to_string();
    kor_sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&kor_sheet_path, &kor_sheet).expect("write kor sheet");

    create_or_update_sheet(&source_path, "eng", Some(&kor_sheet_path), &eng_sheet_path)
        .expect("create eng sheet");
    let eng_sheet = read_sheet(&eng_sheet_path).expect("read eng sheet");

    assert_eq!(eng_sheet.target_language, "eng");
    assert_eq!(eng_sheet.entries[0].translated_value, "");
    assert_eq!(eng_sheet.entries[0].status, JsonTranslationStatus::New);
}

#[test]
fn skips_blank_source_strings_when_creating_sheet() {
    let fixture = TestDir::create("json_translation_skip_blank_sources");
    fixture.write_file(
        "source.json",
        r#"{"blank":"","spaces":"   ","name":"Strike","nested":{"empty":""}}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");

    let report =
        create_or_update_sheet(&fixture.path().join("source.json"), "ko", None, &sheet_path)
            .expect("create sheet");
    let sheet = read_sheet(&sheet_path).expect("read sheet");

    assert_eq!(report.entries, 1);
    assert_eq!(report.missing_entries, 0);
    assert_eq!(sheet.entries.len(), 1);
    assert_eq!(sheet.entries[0].key, "/name");
    assert_eq!(sheet.entries[0].source_value, "Strike");
}

#[test]
fn drops_removed_hardcoded_candidates_when_updating_sheet() {
    let fixture = TestDir::create("json_translation_hardcoded_removed_candidates");
    fixture.write_bytes("source.dll", &utf16le_test_bytes("Profile Name"));
    let source_path = fixture.path().join("source.dll");
    let sheet_path = fixture.path().join("sheet.json");
    let stale_value = "旧噪声候补";
    let existing = JsonTranslationSheet {
        source_path: source_path.to_string_lossy().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 0,
        entries: vec![JsonTranslationEntry {
            key: "dll://2:16".to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: stale_value.to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::Ready,
        }],
    };
    write_sheet(&sheet_path, &existing).expect("write existing sheet");

    let report = create_or_update_sheet(&source_path, "kor", Some(&sheet_path), &sheet_path)
        .expect("update sheet");
    let updated = read_sheet(&sheet_path).expect("read updated sheet");

    assert_eq!(report.removed_entries, 0);
    assert!(
        updated
            .entries
            .iter()
            .all(|entry| entry.status != JsonTranslationStatus::Removed)
    );
    assert!(
        !updated
            .entries
            .iter()
            .any(|entry| entry.source_value == stale_value)
    );
    assert!(
        updated
            .entries
            .iter()
            .any(|entry| entry.source_value == "Profile Name")
    );
}

#[test]
fn applies_translated_values_to_json() {
    let fixture = TestDir::create("json_translation_apply");
    fixture.write_file("source.json", r#"{"card":{"name":"Strike","cost":1}}"#);
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(&fixture.path().join("source.json"), "ko", None, &sheet_path)
        .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    sheet.entries[0].translated_value = "타격".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let output_path = fixture.path().join("translated.json");
    let report = apply_sheet(&sheet_path, &output_path).expect("apply");

    assert_eq!(report.applied_entries, 1);
    assert_eq!(
        fs::read_to_string(output_path).expect("translated"),
        "{\n  \"card\": {\n    \"cost\": 1,\n    \"name\": \"타격\"\n  }\n}"
    );
}

#[test]
fn whitespace_only_translations_are_valid_values() {
    let fixture = TestDir::create("json_translation_whitespace_values");
    fixture.write_file("source.json", r#"{"blankable":"Hidden text"}"#);
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    sheet.entries[0].translated_value = " ".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let validation = validate_sheet(&sheet_path).expect("validate sheet");
    let output_path = fixture.path().join("translated.json");
    let report = apply_sheet(&sheet_path, &output_path).expect("apply");
    let translated = fs::read_to_string(output_path).expect("translated");

    assert!(validation.missing_entries.is_empty());
    assert_eq!(report.applied_entries, 1);
    assert!(translated.contains("\"blankable\": \" \""));
}

#[test]
fn validation_reports_lost_tags_and_line_breaks() {
    let fixture = TestDir::create("json_translation_format_validation");
    fixture.write_file(
        "source.json",
        r#"{"desc":"Deal !D! damage.\n<color=red>Exhaust</color> [E] {count} NL Draw."}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    sheet.entries[0].translated_value =
        "피해를 줍니다. <color=red>소멸 [E] 카드를 뽑습니다.".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let validation = validate_sheet(&sheet_path).expect("validate sheet");
    let issue_kinds = validation
        .format_issues
        .iter()
        .map(|issue| issue.kind.as_str())
        .collect::<Vec<_>>();

    assert!(!validation.valid);
    assert!(issue_kinds.contains(&"line_breaks"));
    assert!(issue_kinds.contains(&"line_break_marker"));
    assert!(issue_kinds.contains(&"angle_tags"));
    assert!(issue_kinds.contains(&"placeholders"));
    assert!(issue_kinds.contains(&"bang_tokens"));
}

#[test]
fn validation_allows_translated_show_placeholder_text() {
    let fixture = TestDir::create("json_translation_show_placeholder_text");
    fixture.write_file(
        "source.json",
        r#"{"desc":"{IfUpgraded:show:[gold]Upgraded[/gold]} Deal {Damage:diff()} damage."}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    sheet.entries[0].translated_value =
        "{IfUpgraded:show:[gold]강화됨[/gold]} 피해를 {Damage:diff()} 줍니다.".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let validation = validate_sheet(&sheet_path).expect("validate sheet");

    assert!(
        validation
            .format_issues
            .iter()
            .all(|issue| issue.kind != "placeholders")
    );
}

#[test]
fn validation_allows_translated_placeholder_variant_text() {
    let fixture = TestDir::create("json_translation_placeholder_variant_text");
    fixture.write_file(
        "source.json",
        r#"{"desc":"At {OnPlayer:your turn|their turn} lose {Amount}."}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    sheet.entries[0].translated_value =
        "{OnPlayer:당신의 턴|그의 턴} 시작 시 {Amount} 잃습니다.".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let validation = validate_sheet(&sheet_path).expect("validate sheet");

    assert!(
        validation
            .format_issues
            .iter()
            .all(|issue| issue.kind != "placeholders")
    );
}

#[test]
fn validation_allows_translated_placeholder_variant_text_with_other_tokens() {
    let fixture = TestDir::create("json_translation_placeholder_variant_text_with_other_tokens");
    fixture.write_file(
        "source.json",
        r#"{"desc":"造成的攻击伤害减少[blue]15%[/blue]。\n在{OnPlayer:你的回合|其回合}开始时失去[blue]{Amount}[/blue]点生命，然后将[gold]枯萎[/gold]层数减少[blue]1/5[/blue]。"}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    sheet.entries[0].translated_value =
        "가하는 공격 피해량이 [blue]15%[/blue] 감소합니다.\n{OnPlayer:당신의 턴|그의 턴} 시작 시 체력을[blue]{Amount}[/blue] 잃고, [gold]고귀[/gold] 중첩을[blue]1/5[/blue] 감소시킵니다.".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let validation = validate_sheet(&sheet_path).expect("validate sheet");

    assert!(
        validation
            .format_issues
            .iter()
            .all(|issue| issue.kind != "placeholders")
    );
}

#[test]
fn directory_sources_create_one_sheet_and_apply_all_json_files() {
    let fixture = TestDir::create("json_translation_directory");
    fixture.write_file("source/cards.json", r#"{"name":"Strike"}"#);
    fixture.write_file("source/relics.json", r#"{"name":"Anchor"}"#);
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(&fixture.path().join("source"), "kor", None, &sheet_path)
        .expect("create directory sheet");

    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    assert!(
        sheet
            .entries
            .iter()
            .any(|entry| entry.key == "file://cards.json#/name")
    );
    assert!(
        sheet
            .entries
            .iter()
            .any(|entry| entry.key == "file://relics.json#/name")
    );
    for entry in &mut sheet.entries {
        if entry.key == "file://cards.json#/name" {
            entry.translated_value = "타격".to_string();
        }
        if entry.key == "file://relics.json#/name" {
            entry.translated_value = "닻".to_string();
        }
        entry.status = JsonTranslationStatus::Ready;
    }
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let output = fixture.path().join("translated");
    let report = apply_sheet(&sheet_path, &output).expect("apply directory");

    assert_eq!(report.applied_entries, 2);
    assert!(
        fs::read_to_string(output.join("cards.json"))
            .expect("cards")
            .contains("타격")
    );
    assert!(
        fs::read_to_string(output.join("relics.json"))
            .expect("relics")
            .contains("닻")
    );
}

#[test]
fn directory_sources_include_loc_files() {
    let fixture = TestDir::create("json_translation_loc_directory");
    fixture.write_file(
        "source/localization/eng/all.loc",
        r#"{"BLIGHT_BUTTON.title":"Blight Mode"}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(&fixture.path().join("source"), "kor", None, &sheet_path)
        .expect("create directory sheet");

    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    let entry = sheet
        .entries
        .iter_mut()
        .find(|entry| entry.key == "file://localization/eng/all.loc#/BLIGHT_BUTTON.title")
        .expect("loc entry");
    entry.translated_value = "Blight Mode KR".to_string();
    entry.status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let output = fixture.path().join("translated");
    let report = apply_sheet(&sheet_path, &output).expect("apply directory");

    assert_eq!(report.applied_entries, 1);
    assert!(
        fs::read_to_string(output.join("all.loc"))
            .expect("loc output")
            .contains("Blight Mode KR")
    );
}

#[test]
fn target_language_folder_prefills_matching_values() {
    let fixture = TestDir::create("json_translation_target_language_match");
    fixture.write_file(
        "localization/eng/cards.json",
        r#"{"name":"Strike","desc":"Deal damage."}"#,
    );
    fixture.write_file(
        "localization/kor/cards.json",
        r#"{"name":"타격","desc":"피해를 줍니다."}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");

    create_or_update_sheet(
        &fixture.path().join("localization/eng"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let sheet = read_sheet(&sheet_path).expect("read sheet");

    assert!(sheet.entries.iter().any(|entry| {
        entry.key == "file://cards.json#/name"
            && entry.translated_value == "타격"
            && entry.status == JsonTranslationStatus::Ready
    }));
    assert!(sheet.entries.iter().any(|entry| {
        entry.key == "file://cards.json#/desc"
            && entry.translated_value == "피해를 줍니다."
            && entry.status == JsonTranslationStatus::Ready
    }));
}

#[test]
fn slot_ids_are_preserved_when_source_language_changes() {
    let fixture = TestDir::create("json_translation_slot_ids_cross_language");
    fixture.write_file(
        "work/source/AkiSister/localization/zhs/cards.json",
        r#"{"name":"探索之旅","desc":"造成伤害。"}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");

    create_or_update_sheet(
        &fixture.path().join("work/source/AkiSister"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create zhs sheet");
    let zhs_sheet = read_sheet(&sheet_path).expect("read zhs sheet");
    let zhs_name_id = zhs_sheet
        .entries
        .iter()
        .find(|entry| entry.key == "file://localization/zhs/cards.json#/name")
        .and_then(|entry| entry.slot_id.clone())
        .expect("zhs name slot id");
    fs::remove_file(
        fixture
            .path()
            .join("work/source/AkiSister/localization/zhs/cards.json"),
    )
    .expect("remove zhs source");
    fixture.write_file(
        "work/source/AkiSister/localization/eng/cards.json",
        r#"{"name":"Journey","desc":"Deal damage."}"#,
    );

    create_or_update_sheet(
        &fixture.path().join("work/source/AkiSister"),
        "kor",
        Some(&sheet_path),
        &sheet_path,
    )
    .expect("update eng sheet");
    let eng_sheet = read_sheet(&sheet_path).expect("read eng sheet");
    let eng_name = eng_sheet
        .entries
        .iter()
        .find(|entry| entry.key == "file://localization/eng/cards.json#/name")
        .expect("eng name");

    assert_eq!(eng_name.slot_id.as_deref(), Some(zhs_name_id.as_str()));
    assert_eq!(eng_name.source_value, "Journey");
    assert!(
        !eng_sheet
            .entries
            .iter()
            .any(|entry| entry.status == JsonTranslationStatus::Removed)
    );
}

#[test]
fn selected_workspace_prefills_existing_target_language_values() {
    let fixture = TestDir::create("json_translation_selected_target_prefill");
    fixture.write_file(
        "work/source/AkiSister/localization/zhs/cards.json",
        r#"{"name":"探索之旅","desc":"造成伤害。"}"#,
    );
    fixture.write_file(
        "work/translated/AkiSister/localization/kor/cards.json",
        r#"{"name":"탐색의 여정","desc":"피해를 줍니다."}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");

    create_or_update_sheet(
        &fixture.path().join("work/source"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create selected sheet");
    let sheet = read_sheet(&sheet_path).expect("read sheet");

    assert!(sheet.entries.iter().any(|entry| {
        entry.key == "file://AkiSister/localization/zhs/cards.json#/name"
            && entry.translated_value == "탐색의 여정"
            && entry.status == JsonTranslationStatus::Ready
    }));
    assert!(sheet.entries.iter().any(|entry| {
        entry.key == "file://AkiSister/localization/zhs/cards.json#/desc"
            && entry.translated_value == "피해를 줍니다."
            && entry.status == JsonTranslationStatus::Ready
    }));
}

#[test]
fn target_language_source_prefills_current_values() {
    let fixture = TestDir::create("json_translation_target_source_prefill");
    fixture.write_file(
        "work/source/AkiSister/localization/kor/cards.json",
        r#"{"name":"탐색의 여정","desc":"피해를 줍니다."}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");

    create_or_update_sheet(
        &fixture.path().join("work/source"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create target source sheet");
    let sheet = read_sheet(&sheet_path).expect("read sheet");

    assert!(sheet.entries.iter().any(|entry| {
        entry.key == "file://AkiSister/localization/kor/cards.json#/name"
            && entry.source_value == "탐색의 여정"
            && entry.translated_value == "탐색의 여정"
            && entry.status == JsonTranslationStatus::Ready
    }));
    assert!(sheet.entries.iter().any(|entry| {
        entry.key == "file://AkiSister/localization/kor/cards.json#/desc"
            && entry.source_value == "피해를 줍니다."
            && entry.translated_value == "피해를 줍니다."
            && entry.status == JsonTranslationStatus::Ready
    }));
}

#[test]
fn accepts_utf8_bom_in_source_sheet_and_import_json() {
    let fixture = TestDir::create("json_translation_bom");
    fixture.write_file("source.json", "\u{feff}{\"name\":\"Strike\"}");
    let sheet_path = fixture.path().join("sheet.json");

    create_or_update_sheet(
        &fixture.path().join("source.json"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet from bom source");
    let mut sheet = read_sheet(&sheet_path).expect("read generated sheet");
    sheet.entries[0].translated_value = "타격".to_string();
    sheet.entries[0].status = JsonTranslationStatus::Ready;
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let sheet_with_bom = format!(
        "\u{feff}{}",
        fs::read_to_string(&sheet_path).expect("sheet text")
    );
    fixture.write_file("sheet-with-bom.json", &sheet_with_bom);
    let loaded =
        read_sheet(&fixture.path().join("sheet-with-bom.json")).expect("read sheet with bom");
    assert_eq!(loaded.entries[0].translated_value, "타격");

    let short_json = format!(
        "\u{feff}{{\"format\":\"sts2-short-translation-v1\",\"entries\":[{{\"id\":\"{}\",\"translated_value\":\"강타\"}}]}}",
        translation_slot_entries(&loaded)
            .into_iter()
            .next()
            .expect("slot")
            .id
    );
    fixture.write_file("import.json", &short_json);
    let (imported, report) = import_translations(&loaded, &fixture.path().join("import.json"))
        .expect("import bom short json");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(imported.entries[0].translated_value, "강타");
}

#[test]
fn infers_target_language_folder_next_to_source_language_folder() {
    let sheet = JsonTranslationSheet {
        source_path: r"Z:\mods\AkiSister\localization\zhs".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 0,
        entries: Vec::new(),
    };

    assert_eq!(
        target_language_output_path(&sheet),
        Some(PathBuf::from(r"Z:\mods\AkiSister\localization\kor"))
    );
}

#[test]
fn selected_source_roots_apply_into_translated_target_language_folder() {
    let fixture = TestDir::create("json_translation_selected_source_root");
    fixture.write_file(
        "work/source/AkiSister/localization/zhs/cards.json",
        r#"{"name":"探索之旅","desc":"造成伤害。"}"#,
    );
    let sheet_path = fixture.path().join("sheet.json");
    create_or_update_sheet(
        &fixture.path().join("work/source"),
        "kor",
        None,
        &sheet_path,
    )
    .expect("create sheet");
    let mut sheet = read_sheet(&sheet_path).expect("read sheet");
    for entry in &mut sheet.entries {
        if entry.key.ends_with("#/name") {
            entry.translated_value = "탐색의 여정".to_string();
        }
        if entry.key.ends_with("#/desc") {
            entry.translated_value = "피해를 줍니다.".to_string();
        }
        entry.status = JsonTranslationStatus::Ready;
    }
    write_sheet(&sheet_path, &sheet).expect("write sheet");

    let report = apply_sheet_to_target_language(&sheet_path).expect("apply target language");
    let translated = fixture
        .path()
        .join("work/translated/AkiSister/localization/kor/cards.json");

    assert_eq!(
        report.output_path,
        fixture
            .path()
            .join("work/translated/AkiSister/localization/kor")
    );
    assert!(translated.is_file());
    assert!(
        fs::read_to_string(translated)
            .expect("translated")
            .contains("탐색의 여정")
    );
    assert!(
        !fixture
            .path()
            .join("work/kor/AkiSister/localization/zhs")
            .exists()
    );
}

#[test]
fn infers_target_language_file_next_to_source_language_file() {
    let sheet = JsonTranslationSheet {
        source_path: r"Z:\mods\AkiSister\localization\zhs\cards.json".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 0,
        entries: Vec::new(),
    };

    assert_eq!(
        target_language_output_path(&sheet),
        Some(PathBuf::from(
            r"Z:\mods\AkiSister\localization\kor\cards.json"
        ))
    );
}

#[test]
fn imports_exported_csv_values_into_sheet() {
    let fixture = TestDir::create("json_translation_import_csv");
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: "file://cards.json#/name".to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "Strike".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    let id = translation_short_id("file://cards.json#/name");
    fixture.write_file(
        "translated.csv",
        &format!("id,source_value,translated_value\n{id},Strike,타격\n"),
    );

    let (updated, report) =
        import_translations(&sheet, &fixture.path().join("translated.csv")).expect("import");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(updated.entries[0].translated_value, "타격");
    assert_eq!(updated.entries[0].status, JsonTranslationStatus::Ready);
}

#[test]
fn imports_csv_values_by_translation_slot_id() {
    let fixture = TestDir::create("json_translation_import_csv_slot_id");
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: "file://cards.json#/name".to_string(),
            slot_id: Some("k001-aa".to_string()),
            previous_source_value: None,
            source_value: "Strike".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    fixture.write_file(
        "translated.csv",
        "id,file,source_value,translated_value\nk001-aa,cards.json,Strike,타격\n",
    );

    let (updated, report) =
        import_translations(&sheet, &fixture.path().join("translated.csv")).expect("import");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(updated.entries[0].translated_value, "타격");
    assert_eq!(updated.entries[0].status, JsonTranslationStatus::Ready);
}

#[test]
fn imports_short_translation_json_values_into_sheet() {
    let fixture = TestDir::create("json_translation_import_short_json");
    let key = "file://cards.json#/desc";
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "Deal damage.".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    let id = translation_slot_entries(&sheet)
        .into_iter()
        .next()
        .expect("slot")
        .id;
    fixture.write_file(
        "translated.json",
        &format!(
            r#"{{"format":"sts2-short-translation-v1","entries":[{{"id":"{}","source_value":"Deal damage.","translated_value":"피해를 줍니다."}}]}}"#,
            id
        ),
    );

    let (updated, report) =
        import_translations(&sheet, &fixture.path().join("translated.json")).expect("import");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(updated.entries[0].translated_value, "피해를 줍니다.");
    assert_eq!(updated.entries[0].status, JsonTranslationStatus::Ready);
}

#[test]
fn exports_compact_translation_json_grouped_by_file() {
    let key = "file://BaseLib/localization/zhs/powers.json#/desc";
    let sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "造成伤害。".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };

    let output = compact_source_translation_map(&sheet, false);
    let file = output.get("powers.json").expect("file group");

    assert_eq!(output.len(), 1);
    assert_eq!(
        file.get(
            &translation_slot_entries(&sheet)
                .into_iter()
                .next()
                .expect("slot")
                .id
        )
        .map(String::as_str),
        Some("造成伤害。")
    );
}

#[test]
fn exports_compact_validation_issue_json_with_context() {
    let sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: "file://BaseLib/localization/zhs/cards.json#/desc".to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "[orange]标记[/orange]\n获得 {Block:diff()} 格挡。".to_string(),
            translated_value: "표식을 얻습니다. {Block:diff()} 방어도.".to_string(),
            status: JsonTranslationStatus::Ready,
        }],
    };
    let validation = validate_translation_sheet(&sheet);
    let output = compact_validation_issue_translation_map(&sheet, &validation);
    let id = translation_slot_entries(&sheet)
        .into_iter()
        .next()
        .expect("slot")
        .id;
    let item = output
        .get("cards.json")
        .and_then(|file| file.get(&id))
        .expect("validation context item");

    assert_eq!(
        item.source,
        "[orange]标记[/orange]\n获得 {Block:diff()} 格挡。"
    );
    assert_eq!(item.translation, "표식을 얻습니다. {Block:diff()} 방어도.");
    assert!(
        item.issues
            .iter()
            .any(|issue| issue.starts_with("줄바꿈 수가 다릅니다"))
    );
    assert!(
        item.issues
            .iter()
            .any(|issue| issue == "대괄호 태그 구성이 원본과 다릅니다.")
    );
}

#[test]
fn imports_grouped_compact_translation_json_values_into_sheet() {
    let fixture = TestDir::create("json_translation_import_grouped_compact_json");
    let key = "file://BaseLib/localization/zhs/cards.json#/desc";
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "造成伤害。".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    let id = translation_slot_entries(&sheet)
        .into_iter()
        .next()
        .expect("slot")
        .id;
    fixture.write_file(
        "translated.json",
        &format!(r#"{{"BaseLib/localization/zhs/cards.json":{{"{id}":"피해를 줍니다."}}}}"#),
    );

    let (updated, report) =
        import_translations(&sheet, &fixture.path().join("translated.json")).expect("import");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(updated.entries[0].translated_value, "피해를 줍니다.");
    assert_eq!(updated.entries[0].status, JsonTranslationStatus::Ready);
}

#[test]
fn imports_source_identical_grouped_compact_values() {
    let fixture = TestDir::create("json_translation_import_source_identical_compact_json");
    let key = "file://BaseLib/localization/zhs/cards.json#/desc";
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "造成伤害。".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    let id = translation_slot_entries(&sheet)
        .into_iter()
        .next()
        .expect("slot")
        .id;
    fixture.write_file(
        "translated.json",
        &format!(r#"{{"BaseLib/localization/zhs/cards.json":{{"{id}":"造成伤害。"}}}}"#),
    );

    let (updated, report) =
        import_translations(&sheet, &fixture.path().join("translated.json")).expect("import");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(updated.entries[0].translated_value, "造成伤害。");
    assert_eq!(updated.entries[0].status, JsonTranslationStatus::Ready);
}

#[test]
fn compact_translation_json_uses_document_slot_ids() {
    let sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![
            JsonTranslationEntry {
                key: "file://cards.json#/a".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "A".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/b".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "B".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
        ],
    };

    let output = compact_source_translation_map(&sheet, false);
    let file = output.get("cards.json").expect("file");

    assert_eq!(file.len(), 2);
    assert!(file.keys().any(|id| id.starts_with("k001-")));
    assert!(file.keys().any(|id| id.starts_with("k002-")));
    assert!(file.keys().all(|id| id.len() == "k001-a7".len()));
}

#[test]
fn compact_translation_json_keeps_full_document_slots_when_exporting_empty_only() {
    let mut sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![
            JsonTranslationEntry {
                key: "file://cards.json#/a".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "A".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/b".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "B".to_string(),
                translated_value: "번역됨".to_string(),
                status: JsonTranslationStatus::Ready,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/c".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "C".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
        ],
    };
    sheet
        .entries
        .sort_by(|left, right| left.key.cmp(&right.key));

    let output = compact_source_translation_map(&sheet, true);
    let file = output.get("cards.json").expect("file");

    assert_eq!(file.len(), 2);
    assert!(file.keys().any(|id| id.starts_with("k001-")));
    assert!(file.keys().any(|id| id.starts_with("k003-")));
    assert!(!file.keys().any(|id| id.starts_with("k002-")));
}

#[test]
fn compact_translation_json_keeps_full_document_slots_when_filtering_keys() {
    let sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![
            JsonTranslationEntry {
                key: "file://cards.json#/a".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "A".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/b".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "B".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/c".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "C".to_string(),
                translated_value: String::new(),
                status: JsonTranslationStatus::New,
            },
        ],
    };
    let include_keys = ["file://cards.json#/c".to_string()].into_iter().collect();

    let output = compact_source_translation_map_with_keys(&sheet, false, Some(&include_keys));
    let file = output.get("cards.json").expect("file");

    assert_eq!(file.len(), 1);
    assert!(file.keys().any(|id| id.starts_with("k003-")));
}

#[test]
fn compact_validation_json_keeps_full_document_slots_when_filtering_keys() {
    let sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![
            JsonTranslationEntry {
                key: "file://cards.json#/a".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "[orange]A[/orange]".to_string(),
                translated_value: "[orange]A[/orange]".to_string(),
                status: JsonTranslationStatus::Ready,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/b".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "[orange]B[/orange]".to_string(),
                translated_value: "[orange]B[/orange]".to_string(),
                status: JsonTranslationStatus::Ready,
            },
            JsonTranslationEntry {
                key: "file://cards.json#/c".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "[orange]C[/orange]".to_string(),
                translated_value: "C".to_string(),
                status: JsonTranslationStatus::Ready,
            },
        ],
    };
    let include_keys = ["file://cards.json#/c".to_string()].into_iter().collect();
    let validation = validate_translation_sheet(&sheet);

    let output = compact_validation_issue_translation_map_with_keys(
        &sheet,
        &validation,
        Some(&include_keys),
    );
    let file = output.get("cards.json").expect("file");

    assert_eq!(file.len(), 1);
    assert!(file.keys().any(|id| id.starts_with("k003-")));
}

#[test]
fn compact_translation_json_expands_padding_after_999_entries() {
    let entries = (1..=1000)
        .map(|index| JsonTranslationEntry {
            key: format!("file://cards.json#/{index:04}"),
            slot_id: None,
            previous_source_value: None,
            source_value: format!("Source {index}"),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        })
        .collect();
    let sheet = JsonTranslationSheet {
        source_path: "source".to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries,
    };

    let output = compact_source_translation_map(&sheet, false);
    let file = output.get("cards.json").expect("file");

    assert_eq!(file.len(), 1000);
    assert!(file.keys().any(|id| id.starts_with("k0001-")));
    assert!(file.keys().any(|id| id.starts_with("k1000-")));
}

#[test]
fn rejects_legacy_compact_short_id_json() {
    let fixture = TestDir::create("json_translation_reject_legacy_compact_json");
    let key = "file://BaseLib/localization/zhs/cards.json#/desc";
    let id = translation_short_id(key);
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "造成伤害。".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    fixture.write_file(
        "translated.json",
        &format!(r#"{{"BaseLib/localization/zhs/cards.json":{{"{id}":"피해를 줍니다."}}}}"#),
    );

    let error = import_translations(&sheet, &fixture.path().join("translated.json"))
        .expect_err("legacy short id should be rejected");

    assert!(error.to_string().contains("이전 short-id JSON"));
}

#[test]
fn rejects_unknown_or_empty_slot_ids() {
    let fixture = TestDir::create("json_translation_reject_bad_slot_json");
    let key = "file://BaseLib/localization/zhs/cards.json#/desc";
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "造成伤害。".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    let bad_id = {
        let actual = translation_slot_entries(&sheet)
            .into_iter()
            .next()
            .expect("slot")
            .id;
        if actual == "k001-zz" {
            "k001-aa"
        } else {
            "k001-zz"
        }
    };
    fixture.write_file(
        "unknown.json",
        &format!(r#"{{"BaseLib/localization/zhs/cards.json":{{"{bad_id}":"피해를 줍니다."}}}}"#),
    );
    let error = import_translations(&sheet, &fixture.path().join("unknown.json"))
        .expect_err("bad checksum should be rejected");
    assert!(error.to_string().contains("unknown translation slot id"));

    let id = translation_slot_entries(&sheet)
        .into_iter()
        .next()
        .expect("slot")
        .id;
    fixture.write_file(
        "empty.json",
        &format!(r#"{{"BaseLib/localization/zhs/cards.json":{{"{id}":""}}}}"#),
    );
    let error = import_translations(&sheet, &fixture.path().join("empty.json"))
        .expect_err("empty translation should be rejected");
    assert!(error.to_string().contains("empty translated value"));
}

#[test]
fn imports_tree_translation_json_without_source_values() {
    let fixture = TestDir::create("json_translation_import_tree_json_without_source");
    let key = "file://BaseLib/localization/zhs/cards.json#/desc";
    let sheet = JsonTranslationSheet {
        source_path: fixture.path().join("source").display().to_string(),
        target_language: "kor".to_string(),
        updated_epoch: 1,
        entries: vec![JsonTranslationEntry {
            key: key.to_string(),
            slot_id: None,
            previous_source_value: None,
            source_value: "造成伤害。".to_string(),
            translated_value: String::new(),
            status: JsonTranslationStatus::New,
        }],
    };
    let id = translation_slot_entries(&sheet)
        .into_iter()
        .next()
        .expect("slot")
        .id;
    fixture.write_file(
        "translated.json",
        &format!(
            r#"{{"format":"sts2-tree-translation-v1","scope":"BaseLib/localization/zhs","target_language":"kor","entries":[{{"id":"{}","translated_value":"피해를 줍니다."}}]}}"#,
            id
        ),
    );

    let (updated, report) =
        import_translations(&sheet, &fixture.path().join("translated.json")).expect("import");

    assert_eq!(report.matched_entries, 1);
    assert_eq!(updated.entries[0].translated_value, "피해를 줍니다.");
    assert_eq!(updated.entries[0].status, JsonTranslationStatus::Ready);
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn create(name: &str) -> Self {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("test-work");
        path.push(format!("{name}-{}", epoch_now()));
        fs::create_dir_all(&path).expect("create test dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write_file(&self, child: &str, content: &str) {
        let path = self.path.join(child);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content.as_bytes()).expect("write file");
    }

    fn write_bytes(&self, child: &str, content: &[u8]) {
        let path = self.path.join(child);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content).expect("write file");
    }
}

fn utf16le_test_bytes(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}
