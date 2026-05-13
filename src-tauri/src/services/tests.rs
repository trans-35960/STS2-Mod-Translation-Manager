#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_language_output_maps_to_pck_resource_path() {
        let source_path =
            Path::new(r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\source");
        let language_output = Path::new(
            r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\translated\AkiSister\localization\kor",
        );

        assert_eq!(
            language_output_relative_to_pck(source_path, language_output).expect("relative"),
            PathBuf::from(r"AkiSister\localization\kor")
        );
    }

    #[test]
    fn pck_folder_output_drops_accidental_json_target_file() {
        let language_output = Path::new(
            r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\translated\AkiSister\localization\kor",
        );

        assert_eq!(
            adjust_pck_target_for_language_output(
                language_output,
                PathBuf::from(r"AkiSister\localization\kor\ancients.json"),
            ),
            PathBuf::from(r"AkiSister\localization\kor")
        );
    }

    #[test]
    fn pck_file_output_appends_file_name_to_folder_target() {
        let language_output = Path::new(
            r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\translated\AkiSister\localization\kor\ancients.json",
        );

        assert_eq!(
            adjust_pck_target_for_language_output(
                language_output,
                PathBuf::from(r"AkiSister\localization\kor"),
            ),
            PathBuf::from(r"AkiSister\localization\kor\ancients.json")
        );
    }

    #[test]
    fn compare_language_uses_scan_root_before_localization_folder() {
        let root = std::env::temp_dir().join(format!("sts2-compare-lang-{}", timestamp_string()));
        let source_root = root.join("source");
        let eng_dir = source_root.join("localization").join("eng");
        let zhs_dir = source_root.join("localization").join("zhs");
        fs::create_dir_all(&eng_dir).expect("create eng");
        fs::create_dir_all(&zhs_dir).expect("create zhs");
        fs::write(eng_dir.join("all.loc"), "{\n  \"title\": \"Name\"\n}").expect("write eng");
        fs::write(zhs_dir.join("all.loc"), "{\n  \"title\": \"名称\"\n}").expect("write zhs");

        let sheet_path = root.join("sheet.translation.json");
        write_sheet(
            &sheet_path,
            &JsonTranslationSheet {
                source_path: zhs_dir.to_string_lossy().to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 1,
                entries: vec![JsonTranslationEntry {
                    key: "file://all.loc#/title".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "名称".to_string(),
                    translated_value: "이름".to_string(),
                    status: JsonTranslationStatus::Ready,
                }],
            },
        )
        .expect("write sheet");

        let values = compare_translation_language(
            sheet_path.to_string_lossy().to_string(),
            "res://localization/eng/all.loc".to_string(),
        )
        .expect("compare language");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].key, "file://all.loc#/title");
        assert_eq!(values[0].value, "Name");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compare_language_fallback_uses_sample_language_root_for_compact_keys() {
        let root = std::env::temp_dir().join(format!("sts2-compare-compact-{}", timestamp_string()));
        let source_root = root.join("source");
        let eng_dir = source_root.join("localization").join("eng");
        let zhs_dir = source_root.join("localization").join("zhs");
        fs::create_dir_all(&eng_dir).expect("create eng");
        fs::create_dir_all(&zhs_dir).expect("create zhs");
        fs::write(eng_dir.join("cards.json"), "{\n  \"name\": \"Strike\"\n}")
            .expect("write eng");
        fs::write(zhs_dir.join("cards.json"), "{\n  \"name\": \"打击\"\n}")
            .expect("write zhs");

        let sheet = JsonTranslationSheet {
            source_path: zhs_dir.to_string_lossy().to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 1,
            entries: vec![JsonTranslationEntry {
                key: "file://cards.json#/name".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "打击".to_string(),
                translated_value: "타격".to_string(),
                status: JsonTranslationStatus::Ready,
            }],
        };

        let values = compare_translation_language_by_files(
            &sheet,
            &source_root,
            "eng",
            Path::new("localization/eng/cards.json"),
        )
        .expect("compare language");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].key, "file://cards.json#/name");
        assert_eq!(values[0].value, "Strike");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compare_language_matches_language_prefixed_entry_keys() {
        let root = std::env::temp_dir().join(format!("sts2-compare-prefixed-{}", timestamp_string()));
        let source_root = root.join("source");
        let eng_dir = source_root.join("NamieFamily").join("localization").join("eng");
        let zhs_dir = source_root.join("NamieFamily").join("localization").join("zhs");
        fs::create_dir_all(&eng_dir).expect("create eng");
        fs::create_dir_all(&zhs_dir).expect("create zhs");
        fs::write(eng_dir.join("ancients.json"), "{\n  \"next\": \"Continue\"\n}")
            .expect("write eng");
        fs::write(zhs_dir.join("ancients.json"), "{\n  \"next\": \"继续\"\n}")
            .expect("write zhs");

        let sheet = JsonTranslationSheet {
            source_path: source_root.to_string_lossy().to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 1,
            entries: vec![JsonTranslationEntry {
                key: "file://NamieFamily/localization/eng/ancients.json#/next".to_string(),
                slot_id: None,
                previous_source_value: None,
                source_value: "Continue".to_string(),
                translated_value: "계속".to_string(),
                status: JsonTranslationStatus::Ready,
            }],
        };

        let values = compare_translation_language_by_files(
            &sheet,
            &source_root,
            "zhs",
            Path::new("NamieFamily/localization/zhs/ancients.json"),
        )
        .expect("compare language");
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].key,
            "file://NamieFamily/localization/eng/ancients.json#/next"
        );
        assert_eq!(values[0].value, "继续");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compare_language_uses_context_pck_contents_root() {
        let root = std::env::temp_dir().join(format!("sts2-compare-context-{}", timestamp_string()));
        let work_root = root.join("work");
        let source_dir = work_root.join("source");
        let pck_root = root.join("cache").join("NamieFamily.pck.contents");
        let zhs_dir = pck_root.join("NamieFamily").join("localization").join("zhs");
        fs::create_dir_all(&source_dir).expect("create source");
        fs::create_dir_all(&zhs_dir).expect("create zhs");
        fs::write(zhs_dir.join("ancients.json"), "{\n  \"next\": \"继续\"\n}")
            .expect("write zhs");
        fs::write(
            work_root.join("translation_context.tsv"),
            format!(
                "mod_key\tnamiefamily\nresource_path\tres://NamieFamily/localization/eng\npck_contents_root\t{}\n",
                pck_root.display()
            ),
        )
        .expect("write context");

        let sheet_path = root.join("sheet.translation.json");
        write_sheet(
            &sheet_path,
            &JsonTranslationSheet {
                source_path: source_dir.to_string_lossy().to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 1,
                entries: vec![JsonTranslationEntry {
                    key: "file://NamieFamily/localization/eng/ancients.json#/next".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "Continue".to_string(),
                    translated_value: "계속".to_string(),
                    status: JsonTranslationStatus::Ready,
                }],
            },
        )
        .expect("write sheet");

        let values = compare_translation_language(
            sheet_path.to_string_lossy().to_string(),
            "res://NamieFamily/localization/zhs".to_string(),
        )
        .expect("compare language");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].value, "继续");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn patch_mod_export_uses_translated_workspace_root() {
        let source_path = Path::new(
            r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\source\AkiSister\localization\eng",
        );
        let language_output = Path::new(
            r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\translated\AkiSister\localization\kor",
        );

        assert_eq!(
            translation_patch_payload_root(source_path, language_output).expect("payload root"),
            PathBuf::from(
                r"Z:\game\sts2\modmanager\translation_work\selected\mod\workspace\translated"
            )
        );
    }

    #[test]
    fn patch_mod_package_id_keeps_mod_id_shape() {
        assert_eq!(sanitize_package_id("Aki Sister"), "Aki_Sister");
        assert_eq!(sanitize_package_id("___"), "translation_patch");
    }

    #[test]
    fn kor_language_label_is_korean() {
        assert_eq!(language_label("kor"), "한국어");
    }

    #[test]
    fn language_preview_does_not_inherit_parent_kor_path() {
        let root = std::env::temp_dir()
            .join(format!("sts2-preview-parent-kor-{}", timestamp_string()))
            .join("kor")
            .join("blight");
        for language in ["eng", "rus", "zhs"] {
            let dir = root.join("localization").join(language);
            fs::create_dir_all(&dir).expect("create localization");
            fs::write(dir.join("all.loc"), r#"{"title":"Name"}"#).expect("write loc");
        }

        let previews = language_preview_from_scan_root(&root);
        let codes = previews
            .iter()
            .map(|language| language.code.as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&"en"));
        assert!(codes.contains(&"ru"));
        assert!(codes.contains(&"zh-cn"));
        assert!(!codes.contains(&"kor"));
        let _ = fs::remove_dir_all(root.parent().and_then(Path::parent).unwrap_or(&root));
    }

    #[test]
    fn translation_patch_targets_base_manifest_id_when_folder_name_differs() {
        let base_record = ModRecord {
            name: "Miyu STS2-622-v1-0-2-7-12-9-1777882282".to_string(),
            path: PathBuf::from(r"Z:\game\Slay the Spire 2\mods.disabled\Miyu"),
            source: ModSource::Disabled,
            kind: ModKind::Directory,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 1,
                modified: None,
            },
        };
        let patch_record = ModRecord {
            name: "Miyu_character_tr".to_string(),
            path: PathBuf::from(r"Z:\game\Slay the Spire 2\mods.disabled\Miyu_character_tr"),
            source: ModSource::Disabled,
            kind: ModKind::Directory,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 1,
                modified: None,
            },
        };
        let base_manifest = ModManifestInfo {
            id: Some("Miyu_character".to_string()),
            name: Some("霞沢美游[Kasumizawa Miyu]".to_string()),
            version: Some("1.0.2.7.12.9".to_string()),
            ..ModManifestInfo::default()
        };
        let patch_manifest = ModManifestInfo {
            is_translation_patch: true,
            target_mod_id: Some("Miyu_character".to_string()),
            target_languages: vec!["kor".to_string()],
            ..ModManifestInfo::default()
        };

        assert!(translation_patch_matches_language(&patch_manifest, "kor"));
        assert!(translation_patch_targets_base(
            &patch_record,
            &patch_manifest,
            &base_record,
            &base_manifest
        ));
    }

    #[test]
    fn record_manifest_reads_translation_patch_json_next_to_payload() {
        let root = std::env::temp_dir().join(format!("sts2-record-manifest-{}", timestamp_string()));
        let mod_root = root.join("Miyu_character_tr");
        let scan_root = mod_root.join("Miyu.pck.contents");
        fs::create_dir_all(&scan_root).expect("create scan root");
        fs::write(
            mod_root.join("Miyu_character_tr.json"),
            r#"{
                "id": "Miyu_character_tr",
                "is_translation_patch": true,
                "target_mod_id": "Miyu_character",
                "target_languages": ["kor"],
                "dependencies": ["Miyu_character"]
            }"#,
        )
        .expect("write patch manifest");

        let manifest = read_mod_manifest_for_record(&mod_root, &scan_root);

        assert!(manifest.is_translation_patch);
        assert_eq!(manifest.target_mod_id.as_deref(), Some("Miyu_character"));
        assert_eq!(manifest.target_languages, vec!["kor".to_string()]);
        assert_eq!(
            manifest
                .dependencies
                .first()
                .map(|dependency| dependency.id.as_str()),
            Some("Miyu_character")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn connected_patch_copy_finds_target_language_inside_pck_contents() {
        let root = std::env::temp_dir().join(format!("sts2-patch-copy-{}", timestamp_string()));
        let scan_root = root.join("scan");
        let patch_file = scan_root
            .join("Miyu.pck.contents")
            .join("Miyu_character")
            .join("localization")
            .join("kor")
            .join("cards.json");
        fs::create_dir_all(patch_file.parent().expect("patch parent")).expect("create patch dir");
        fs::write(&patch_file, r#"{"ID":{"NAMES":["번역"]}}"#).expect("write patch json");
        let translated_root = root.join("translated");
        let relative = PathBuf::from(r"Miyu_character\localization\kor\cards.json");

        assert!(
            copy_resource_relative_if_exists(&scan_root, &relative, &translated_root)
                .expect("copy patch")
        );
        assert_eq!(
            fs::read_to_string(translated_root.join(relative)).expect("read copied"),
            r#"{"ID":{"NAMES":["번역"]}}"#
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folder_mods_prefer_nested_pck_for_preview() {
        let root = std::env::temp_dir().join(format!("sts2-pck-preview-{}", timestamp_string()));
        let mod_dir = root.join("AkiSister-654");
        let inner = mod_dir.join("AkiSister");
        fs::create_dir_all(&inner).expect("create mod");
        fs::write(inner.join("AkiSister.json"), "{}").expect("write json");
        fs::write(inner.join("AkiSister.pck"), "pck").expect("write pck");
        let record = ModRecord {
            name: "AkiSister-654".to_string(),
            path: mod_dir,
            source: sts2_mod_manager::domain::ModSource::GameMods,
            kind: ModKind::Directory,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 3,
                modified: None,
            },
        };

        assert_eq!(
            extraction_source_for_record(&record)
                .file_name()
                .map(|value| value.to_string_lossy().to_string()),
            Some("AkiSister.pck".to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folder_mods_with_loc_files_keep_directory_for_preview() {
        let root = std::env::temp_dir().join(format!("sts2-loc-preview-{}", timestamp_string()));
        let mod_dir = root.join("Blight");
        fs::create_dir_all(mod_dir.join("localization").join("eng")).expect("create mod");
        fs::write(mod_dir.join("blight.dll"), "dll").expect("write dll");
        fs::write(
            mod_dir.join("localization").join("eng").join("all.loc"),
            r#"{"BLIGHT_BUTTON.title":"Blight Mode"}"#,
        )
        .expect("write loc");
        let record = ModRecord {
            name: "Blight".to_string(),
            path: mod_dir.clone(),
            source: sts2_mod_manager::domain::ModSource::ExternalManager,
            kind: ModKind::Directory,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 3,
                modified: None,
            },
        };

        assert_eq!(extraction_source_for_record(&record), mod_dir);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_install_flattens_single_payload_directory() {
        let root = std::env::temp_dir().join(format!("sts2-archive-payload-{}", timestamp_string()));
        let extracted = root.join("extracted");
        let payload = extracted.join("blight");
        fs::create_dir_all(payload.join("localization").join("zhs")).expect("create loc dir");
        fs::write(payload.join("blight.dll"), "dll").expect("write dll");
        fs::write(payload.join("blight.json"), "{}").expect("write json");
        fs::write(payload.join("localization").join("zhs").join("all.loc"), "loc")
            .expect("write loc");

        assert_eq!(archive_install_payload_root(&extracted), payload);
        assert!(folder_install_root_has_runtime_payload(
            &extracted.join("blight")
        ));
        assert!(!folder_install_root_has_runtime_payload(
            &extracted.join("blight").join("localization")
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dropped_archive_preview_keeps_localization_folders_inside_mod_root() {
        let root = std::env::temp_dir().join(format!("sts2-drop-localization-{}", timestamp_string()));
        let mod_root = root.join("blight");
        fs::create_dir_all(mod_root.join("localization").join("eng")).expect("create eng");
        fs::create_dir_all(mod_root.join("localization").join("rus")).expect("create rus");
        fs::create_dir_all(mod_root.join("localization").join("zhs")).expect("create zhs");
        fs::write(mod_root.join("blight.dll"), "dll").expect("write dll");
        fs::write(mod_root.join("blight.json"), "{}").expect("write json");
        fs::write(mod_root.join("localization").join("eng").join("all.loc"), "eng")
            .expect("write eng loc");
        fs::write(mod_root.join("localization").join("rus").join("all.loc"), "rus")
            .expect("write rus loc");
        fs::write(mod_root.join("localization").join("zhs").join("all.loc"), "zhs")
            .expect("write zhs loc");

        let records = split_dropped_directory(&root).expect("split dropped directory");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "blight");
        assert_eq!(records[0].path, mod_root);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folder_mods_with_hardcoded_dll_keep_directory_for_preview() {
        let root = std::env::temp_dir().join(format!("sts2-dll-preview-{}", timestamp_string()));
        let mod_dir = root.join("AscensionUnlockMod");
        let payload_dir = mod_dir.join("AscensionUnlockMod");
        fs::create_dir_all(&payload_dir).expect("create mod");
        fs::write(payload_dir.join("AscensionUnlockMod.pck"), "pck").expect("write pck");
        fs::write(
            payload_dir.join("AscensionUnlockMod.dll"),
            utf16le_test_bytes("Current ascension state unavailable."),
        )
        .expect("write dll");
        let record = ModRecord {
            name: "AscensionUnlockMod".to_string(),
            path: mod_dir.clone(),
            source: sts2_mod_manager::domain::ModSource::GameMods,
            kind: ModKind::Directory,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 3,
                modified: None,
            },
        };

        assert_eq!(extraction_source_for_record(&record), mod_dir);
        let scan_root = root.join("preview");
        assert!(expand_directory_preview(&record.path, &scan_root, Path::new("missing-vendor")));
        let dll_path = scan_root
            .join("AscensionUnlockMod")
            .join("AscensionUnlockMod.dll");
        assert!(dll_path.exists());
        assert_eq!(
            selected_translation_files(&scan_root, "AscensionUnlockMod/AscensionUnlockMod.dll"),
            vec![dll_path]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn directory_preview_cache_expands_pcks_without_copying_whole_mod() {
        let root = std::env::temp_dir().join(format!("sts2-dir-preview-{}", timestamp_string()));
        let source = root
            .join("LibraryOfRuina")
            .join("disabled")
            .join("LibraryOfRuina");
        fs::create_dir_all(&source).expect("create source");
        fs::write(source.join("LibraryOfRuina.pck"), "pck").expect("write pck");
        let destination = root.join("preview");

        assert!(!expand_directory_preview(
            root.join("LibraryOfRuina").as_path(),
            &destination,
            Path::new("missing-vendor")
        ));
        assert!(
            !destination
                .join("disabled")
                .join("LibraryOfRuina")
                .join("LibraryOfRuina.pck")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn directory_preview_detects_existing_pck_contents() {
        let root =
            std::env::temp_dir().join(format!("sts2-dir-preview-ready-{}", timestamp_string()));
        let contents = root.join("Mod").join("Mod.pck.contents");
        fs::create_dir_all(&contents).expect("create contents");

        assert!(directory_preview_has_pck_contents(&root));
        assert!(!directory_preview_has_pck_contents(&root.join("missing")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backup_paths_do_not_overwrite_existing_files() {
        let root = std::env::temp_dir().join(format!("sts2-backup-path-{}", timestamp_string()));
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("AkiSister.rar"), "old").expect("write existing backup");

        assert_eq!(
            unique_backup_path(&root, std::ffi::OsStr::new("AkiSister.rar"))
                .file_name()
                .map(|value| value.to_string_lossy().to_string()),
            Some("AkiSister-1.rar".to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn move_path_or_copy_moves_regular_file() {
        let root = std::env::temp_dir().join(format!("sts2-move-path-{}", timestamp_string()));
        fs::create_dir_all(&root).expect("create root");
        let source = root.join("source.rar");
        let target = root.join("target.rar");
        fs::write(&source, "mod").expect("write source");

        move_path_or_copy(&source, &target).expect("move path");

        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&target).expect("read target"), "mod");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn translation_memory_cleanup_removes_pck_payloads_but_keeps_sheets() {
        let root = std::env::temp_dir().join(format!("sts2-memory-cleanup-{}", timestamp_string()));
        let memory_root = root.join("translation_memory");
        let mod_memory = memory_root.join("akisister-654");
        fs::create_dir_all(mod_memory.join("pck_build").join("full_extract"))
            .expect("create build");
        fs::write(
            mod_memory
                .join("pck_build")
                .join("full_extract")
                .join("mod.pck"),
            "large",
        )
        .expect("write build pck");
        fs::write(mod_memory.join("patched.pck"), "packed").expect("write pck");
        fs::write(mod_memory.join("translated_mod.zip"), "zip").expect("write zip");
        let sheet = mod_memory.join("cards.kor.translation.json");
        fs::write(&sheet, "{}").expect("write sheet");
        let mut removed_dirs = 0;
        let mut removed_files = 0;

        cleanup_translation_memory_payloads(
            &memory_root,
            &mut removed_dirs,
            &mut removed_files,
            std::slice::from_ref(&root),
        )
        .expect("cleanup payloads");

        assert!(removed_dirs > 0);
        assert!(removed_files > 0);
        assert!(!mod_memory.join("pck_build").exists());
        assert!(!mod_memory.join("patched.pck").exists());
        assert!(!mod_memory.join("translated_mod.zip").exists());
        assert!(sheet.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn work_cache_cleanup_removes_all_reported_cache_usage() {
        let root = std::env::temp_dir().join(format!("sts2-work-cache-cleanup-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(config.state_dir.join("language_preview_extract").join("scan-1"))
            .expect("create preview cache");
        fs::write(
            config
                .state_dir
                .join("language_preview_extract")
                .join("scan-1")
                .join("payload.bin"),
            "cache",
        )
        .expect("write preview cache");
        fs::create_dir_all(config.state_dir.join("drop_imports").join("drop-1"))
            .expect("create drop cache");
        fs::write(
            config
                .state_dir
                .join("drop_imports")
                .join("drop-1")
                .join("mod.zip"),
            "zip",
        )
        .expect("write drop cache");
        fs::write(config.state_dir.join("language_preview_cache.tsv"), "cache")
            .expect("write language cache");

        let memory_root = config
            .translation_work_dir
            .join("translation_memory")
            .join("blight");
        fs::create_dir_all(&memory_root).expect("create memory");
        let sheet = memory_root.join("all.kor.translation.json");
        fs::write(&sheet, "{}").expect("write sheet");
        fs::write(memory_root.join("patched.pck"), "pck").expect("write pck");
        let selected_root = config
            .translation_work_dir
            .join("selected")
            .join("blight")
            .join("source");
        fs::create_dir_all(&selected_root).expect("create selected");
        fs::write(selected_root.join("source.pck"), "pck").expect("write selected pck");

        assert!(work_cache_usage(&config).bytes > 0);
        let mut removed_dirs = 0;
        let mut removed_files = 0;
        cleanup_work_caches(&config, &mut removed_dirs, &mut removed_files).expect("cleanup");

        let usage = work_cache_usage(&config);
        assert_eq!(usage.bytes, 0);
        assert_eq!(usage.files, 0);
        assert_eq!(usage.dirs, 0);
        assert!(removed_dirs > 0);
        assert!(removed_files > 0);
        assert!(sheet.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn connected_mod_keys_skip_disabled_game_versions() {
        let active = ModRecord {
            name: "The Reaper For Main Branch-412-0-43-1777465658".to_string(),
            path: PathBuf::from(r"Z:\game\Slay the Spire 2\mods\The Reaper.zip"),
            source: sts2_mod_manager::domain::ModSource::GameMods,
            kind: ModKind::Archive,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 1,
                modified: None,
            },
        };
        let disabled = ModRecord {
            name: "The Reaper For Main Branch-412-0-40-1777040399".to_string(),
            path: PathBuf::from(r"Z:\game\Slay the Spire 2\mods.disabled\The Reaper Old.zip"),
            source: sts2_mod_manager::domain::ModSource::Disabled,
            kind: ModKind::Archive,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 1,
                modified: None,
            },
        };
        let keys = connected_mod_keys(&ScanSummary {
            game_mods: vec![active],
            disabled_mods: vec![disabled],
            external_manager_mods: Vec::new(),
        });

        assert!(keys.contains("the-reaper-for-main-branch-412-0-43-1777465658"));
        assert!(!keys.contains("the-reaper-for-main-branch-412-0-40-1777040399"));
    }

    #[test]
    fn manifest_reader_accepts_json_with_comments() {
        let root = std::env::temp_dir().join(format!("sts2-jsonc-manifest-{}", timestamp_string()));
        fs::create_dir_all(&root).expect("create root");
        fs::write(
            root.join("ZSproject.json"),
            r#"{
  "id": "zsproject", // unique id
  "name": "ZS Test Mod",
  "version": "0.4",
  "dependencies": ["BaseLib"] // required mods
}"#,
        )
        .expect("write manifest");

        let manifest = read_mod_manifest_info(&root);

        assert_eq!(manifest.id.as_deref(), Some("zsproject"));
        assert_eq!(manifest.name.as_deref(), Some("ZS Test Mod"));
        assert_eq!(manifest.version.as_deref(), Some("0.4"));
        assert_eq!(
            manifest
                .dependencies
                .iter()
                .map(|dependency| dependency.id.as_str())
                .collect::<Vec<_>>(),
            vec!["BaseLib"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deleted_mod_entries_hide_keys_until_restored() {
        let root = std::env::temp_dir().join(format!("sts2-deleted-index-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(&config.state_dir).expect("create state");
        let backup = config
            .state_dir
            .join("deleted_mods")
            .join("entry")
            .join("mod.zip");
        fs::create_dir_all(backup.parent().expect("backup parent")).expect("create backup parent");
        fs::write(&backup, "mod").expect("write backup");
        upsert_deleted_mod_entry(
            &config,
            DeletedModEntry {
                id: "entry".to_string(),
                key: "deleted-mod".to_string(),
                name: "Deleted Mod".to_string(),
                original_path: config.game_mods_dir.join("Deleted Mod.zip"),
                backup_path: backup,
                deleted_epoch: 100,
                bytes: 3,
            },
        )
        .expect("write deleted entry");

        assert!(deleted_mod_keys(&config).contains("deleted-mod"));
        remove_deleted_mod_entry(&config, "entry").expect("remove entry");
        assert!(!deleted_mod_keys(&config).contains("deleted-mod"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn restored_game_archive_is_marked_for_extraction() {
        let root =
            std::env::temp_dir().join(format!("sts2-deleted-archive-{}", timestamp_string()));
        let config = test_config(&root);
        let backup = root
            .join("state")
            .join("deleted_mods")
            .join("entry")
            .join("Mod.rar");
        fs::create_dir_all(backup.parent().expect("backup parent"))
            .expect("create backup parent");
        fs::write(&backup, "archive").expect("write backup");
        let entry = DeletedModEntry {
            id: "entry".to_string(),
            key: "mod".to_string(),
            name: "Mod".to_string(),
            original_path: config.game_mods_dir.join("Mod.rar"),
            backup_path: backup,
            deleted_epoch: 100,
            bytes: 7,
        };

        assert!(should_expand_restored_archive(&entry, &config));
        assert_eq!(
            restored_archive_install_dir(&entry.original_path, &config),
            Some(config.game_mods_dir.join("Mod"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reappeared_deleted_game_mods_are_quarantined() {
        let root =
            std::env::temp_dir().join(format!("sts2-deleted-quarantine-{}", timestamp_string()));
        let config = test_config(&root);
        let active = config.game_mods_dir.join("DeletedMod");
        let backup = config
            .state_dir
            .join("deleted_mods")
            .join("entry")
            .join("DeletedMod");
        fs::create_dir_all(&active).expect("create active");
        fs::create_dir_all(&backup).expect("create backup");
        fs::create_dir_all(&config.state_dir).expect("create state");
        fs::write(active.join("mod.json"), "{}").expect("write active");
        fs::write(backup.join("mod.json"), "{}").expect("write backup");
        upsert_deleted_mod_entry(
            &config,
            DeletedModEntry {
                id: "entry".to_string(),
                key: "deleted-mod".to_string(),
                name: "Deleted Mod".to_string(),
                original_path: active.clone(),
                backup_path: backup.clone(),
                deleted_epoch: 100,
                bytes: 2,
            },
        )
        .expect("write deleted entry");

        let moved = quarantine_reappeared_deleted_mods(&config).expect("quarantine");

        assert_eq!(moved, 1);
        assert!(!active.exists());
        assert!(backup.exists());
        assert!(backup.with_file_name("DeletedMod-1").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deleted_mods_are_removed_from_desired_active_keys() {
        let root =
            std::env::temp_dir().join(format!("sts2-deleted-desired-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(&config.state_dir).expect("create state");
        let mut desired = BTreeSet::new();
        desired.insert("deleted-mod".to_string());
        desired.insert("kept-mod".to_string());
        write_desired_active_mod_keys(&desired, &config.state_dir).expect("write desired");
        let backup = config
            .state_dir
            .join("deleted_mods")
            .join("entry")
            .join("Deleted Mod");
        fs::create_dir_all(&backup).expect("create backup");
        upsert_deleted_mod_entry(
            &config,
            DeletedModEntry {
                id: "entry".to_string(),
                key: "deleted-mod".to_string(),
                name: "Deleted Mod".to_string(),
                original_path: config.game_mods_dir.join("Deleted Mod"),
                backup_path: backup,
                deleted_epoch: 100,
                bytes: 2,
            },
        )
        .expect("write deleted entry");
        let summary = ScanSummary {
            game_mods: Vec::new(),
            disabled_mods: Vec::new(),
            external_manager_mods: Vec::new(),
        };

        let removed = prune_deleted_desired_mod_keys(&config, &summary).expect("prune desired");
        let desired = desired_active_mod_keys(&summary, &config.state_dir).expect("read desired");

        assert_eq!(removed, 1);
        assert!(!desired.contains("deleted-mod"));
        assert!(desired.contains("kept-mod"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expired_deleted_mods_become_tombstones() {
        let root = std::env::temp_dir().join(format!("sts2-deleted-prune-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(&config.state_dir).expect("create state");
        let backup = config
            .state_dir
            .join("deleted_mods")
            .join("entry")
            .join("old.zip");
        fs::create_dir_all(backup.parent().expect("backup parent")).expect("create backup parent");
        fs::write(&backup, "mod").expect("write backup");
        upsert_deleted_mod_entry(
            &config,
            DeletedModEntry {
                id: "entry".to_string(),
                key: "old-mod".to_string(),
                name: "Old Mod".to_string(),
                original_path: config.game_mods_dir.join("Old Mod.zip"),
                backup_path: backup.clone(),
                deleted_epoch: 1,
                bytes: 3,
            },
        )
        .expect("write deleted entry");

        prune_expired_deleted_mods(&config, 1).expect("prune deleted");

        assert!(!backup.exists());
        assert!(
            read_deleted_mod_entries(&config)
                .expect("read entries")
                .is_empty()
        );
        assert!(deleted_mod_keys(&config).contains("old-mod"));
        let _ = fs::remove_dir_all(root);
    }

    fn utf16le_test_bytes(value: &str) -> Vec<u8> {
        let mut bytes = vec![0, 0xff];
        bytes.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
        bytes.extend([0, 0xff]);
        bytes
    }

    #[test]
    fn reappeared_mods_clear_deleted_tombstones() {
        let root = std::env::temp_dir().join(format!("sts2-deleted-reappeared-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(&config.state_dir).expect("create state");
        remember_deleted_mod_tombstone(&config, "new-download").expect("write tombstone");
        let summary = ScanSummary {
            game_mods: Vec::new(),
            disabled_mods: Vec::new(),
            external_manager_mods: vec![ModRecord {
                name: "New Download".to_string(),
                path: root.join("Vortex").join("downloads").join("New Download.zip"),
                source: ModSource::ExternalManager,
                kind: ModKind::Archive,
                version_hint: None,
                fingerprint: sts2_mod_manager::domain::ModFingerprint {
                    bytes: 3,
                    modified: None,
                },
            }],
        };

        assert!(!deleted_mod_keys_for_summary(&config, &summary).contains("new-download"));
        forget_revived_deleted_mod_tombstones(&config, &summary).expect("clear tombstone");
        assert!(!deleted_mod_keys(&config).contains("new-download"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn translation_apply_history_keeps_latest_record_by_mod() {
        let root = std::env::temp_dir().join(format!("sts2-apply-history-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(&config.state_dir).expect("create state");
        let older = TranslationApplyRecord {
            mod_key: "akisister".to_string(),
            target_language: "kor".to_string(),
            applied_epoch: 100,
            applied_entries: 1,
            output_path: root.join("old"),
            installed_mod_path: None,
            packed_pck_path: None,
        };
        let newer = TranslationApplyRecord {
            mod_key: "akisister".to_string(),
            target_language: "kor".to_string(),
            applied_epoch: 200,
            applied_entries: 3,
            output_path: root.join("new"),
            installed_mod_path: Some(root.join("installed.pck")),
            packed_pck_path: Some(root.join("packed.pck")),
        };

        write_translation_apply_record(&config, &older).expect("write older");
        write_translation_apply_record(&config, &newer).expect("write newer");
        let index = read_translation_apply_index(&config).expect("read index");

        assert_eq!(index["akisister"].applied_epoch, 200);
        assert_eq!(index["akisister"].applied_entries, 3);
        assert_eq!(
            index["akisister"].installed_mod_path,
            Some(root.join("installed.pck"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn csv_export_uses_translation_slot_ids() {
        let root = std::env::temp_dir().join(format!("sts2-csv-slot-export-{}", timestamp_string()));
        fs::create_dir_all(&root).expect("create root");
        let sheet = JsonTranslationSheet {
            source_path: root.join("source").display().to_string(),
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
        let output = root.join("sheet.csv");

        let report = export_json_translation_csv(
            output.display().to_string(),
            json_sheet_dto(sheet),
        )
        .expect("export csv");
        let csv = fs::read_to_string(&output).expect("read csv");

        assert_eq!(report.rows, 1);
        assert!(csv.contains("k001-aa,Strike,"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn change_json_export_includes_new_and_updated_sources() {
        let root = std::env::temp_dir().join(format!("sts2-change-export-{}", timestamp_string()));
        fs::create_dir_all(&root).expect("create root");
        let sheet = JsonTranslationSheet {
            source_path: root.join("source").display().to_string(),
            target_language: "kor".to_string(),
            updated_epoch: 1,
            entries: vec![
                JsonTranslationEntry {
                    key: "file://cards.json#/updated".to_string(),
                    slot_id: Some("k001-aa".to_string()),
                    previous_source_value: Some("Old source".to_string()),
                    source_value: "New source".to_string(),
                    translated_value: "기존 번역".to_string(),
                    status: JsonTranslationStatus::Updated,
                },
                JsonTranslationEntry {
                    key: "file://cards.json#/new".to_string(),
                    slot_id: Some("k002-bb".to_string()),
                    previous_source_value: None,
                    source_value: "Brand new".to_string(),
                    translated_value: String::new(),
                    status: JsonTranslationStatus::New,
                },
                JsonTranslationEntry {
                    key: "file://cards.json#/ready".to_string(),
                    slot_id: Some("k003-cc".to_string()),
                    previous_source_value: None,
                    source_value: "Ready".to_string(),
                    translated_value: "완료".to_string(),
                    status: JsonTranslationStatus::Ready,
                },
            ],
        };
        let output = root.join("changes.json");

        let report = export_json_translation_change_json(
            output.display().to_string(),
            json_sheet_dto(sheet),
            None,
        )
        .expect("export change json");
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&output).expect("read output"))
                .expect("parse output");

        assert_eq!(report.rows, 2);
        assert_eq!(
            json["cards.json"]["k001-aa"]["original_source"],
            "Old source"
        );
        assert_eq!(json["cards.json"]["k001-aa"]["source"], "New source");
        assert_eq!(json["cards.json"]["k001-aa"]["translation"], "기존 번역");
        assert_eq!(json["cards.json"]["k002-bb"]["original_source"], "");
        assert!(json["cards.json"].get("k003-cc").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pck_source_install_replaces_active_pck() {
        let root = std::env::temp_dir().join(format!("sts2-pck-install-{}", timestamp_string()));
        let config = test_config(&root);
        let active_pck = config.game_mods_dir.join("Mod").join("Mod.pck");
        let patched_pck = root.join("patched.pck");
        fs::create_dir_all(active_pck.parent().expect("active parent"))
            .expect("create active parent");
        fs::create_dir_all(&config.state_dir).expect("create state");
        fs::write(&active_pck, "old pck").expect("write active");
        fs::write(&patched_pck, "patched pck").expect("write patched");
        let context = TranslationContext {
            mod_key: Some("mod".to_string()),
            extraction_source_path: Some(active_pck.clone()),
            input_pck_path: Some(active_pck.clone()),
            pck_contents_root: None,
            pck_stem: Some("Mod".to_string()),
            translation_patch_source_path: None,
            translation_patch_pck_stem: None,
        };

        let installed = install_patched_archive_mod(
            &context,
            &root.join("missing-archive"),
            &active_pck,
            &patched_pck,
            &config,
        )
        .expect("install pck");

        assert_eq!(installed, Some(active_pck.clone()));
        assert_eq!(
            fs::read_to_string(&active_pck).expect("read active"),
            "patched pck"
        );
        assert!(config.state_dir.join("applied_backups").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn directory_source_resolves_nested_pck_without_archive_extract() {
        let root = std::env::temp_dir().join(format!("sts2-dir-pck-resolve-{}", timestamp_string()));
        let source_dir = root.join("mods.disabled").join("AkiSister");
        let nested = source_dir.join("payload").join("AkiSister.pck");
        fs::create_dir_all(nested.parent().expect("nested parent")).expect("create nested");
        fs::write(&nested, "pck").expect("write pck");

        let resolved = pck_from_extractable_source(
            &source_dir,
            Some("AkiSister"),
            &root.join("build"),
            &root.join("missing-vendor"),
        )
        .expect("resolve nested pck");

        assert_eq!(resolved, nested);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn directory_source_install_replaces_nested_pck() {
        let root = std::env::temp_dir().join(format!("sts2-dir-pck-install-{}", timestamp_string()));
        let config = test_config(&root);
        let source_dir = root.join("mods.disabled").join("AkiSister");
        let source_pck = source_dir.join("AkiSister.pck");
        let patched_pck = root.join("patched.pck");
        fs::create_dir_all(&source_dir).expect("create source");
        fs::create_dir_all(&config.state_dir).expect("create state");
        fs::write(&source_pck, "old pck").expect("write source pck");
        fs::write(&patched_pck, "patched pck").expect("write patched");
        let context = TranslationContext {
            mod_key: Some("akisister".to_string()),
            extraction_source_path: Some(source_dir.clone()),
            input_pck_path: None,
            pck_contents_root: None,
            pck_stem: Some("AkiSister".to_string()),
            translation_patch_source_path: None,
            translation_patch_pck_stem: None,
        };

        let installed = install_patched_archive_mod(
            &context,
            &root.join("missing-archive"),
            &source_pck,
            &patched_pck,
            &config,
        )
        .expect("install nested pck");

        assert_eq!(installed, Some(source_dir));
        assert_eq!(
            fs::read_to_string(&source_pck).expect("read source pck"),
            "patched pck"
        );
        assert!(config.state_dir.join("applied_backups").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn translation_context_routes_direct_apply_to_connected_patch() {
        let root =
            std::env::temp_dir().join(format!("sts2-patch-context-{}", timestamp_string()));
        let work_root = root
            .join("translation_work")
            .join("selected")
            .join("BaseMod")
            .join("localization-eng");
        let source_file = work_root
            .join("source")
            .join("BaseMod.pck.contents")
            .join("BaseMod")
            .join("localization")
            .join("eng")
            .join("cards.json");
        let base_source = root.join("mods").join("BaseMod").join("BaseMod.pck");
        let patch_source = root.join("mods").join("BaseMod_tr").join("BaseMod_tr.pck");
        fs::create_dir_all(source_file.parent().expect("source parent")).expect("create source");
        write_translation_context(TranslationContextWriteRequest {
            work_dir: &work_root,
            mod_key: "BaseMod",
            resource_path: "res://BaseMod/localization/eng",
            extraction_source: &base_source,
            pck_contents_root: Some(&work_root.join("source").join("BaseMod.pck.contents")),
            pck_stem: "BaseMod",
            translation_patch_source: Some(&patch_source),
            translation_patch_pck_stem: Some("BaseMod_tr"),
        })
        .expect("write context");

        let context = read_translation_context(&source_file).expect("read context");
        let direct = context.direct_apply_context();

        assert_eq!(context.extraction_source_path, Some(base_source));
        assert_eq!(
            context.translation_patch_source_path,
            Some(patch_source.clone())
        );
        assert_eq!(direct.extraction_source_path, Some(patch_source));
        assert_eq!(direct.input_pck_path, None);
        assert_eq!(direct.pck_stem.as_deref(), Some("BaseMod_tr"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_install_replaces_active_mod_root_folder() {
        let root =
            std::env::temp_dir().join(format!("sts2-archive-install-root-{}", timestamp_string()));
        let config = test_config(&root);
        let active_root = config.game_mods_dir.join("Herta");
        let source_archive = active_root.join("payload").join("HertaV1.42.rar");
        let archive_dir = root.join("archive");
        let input_pck = archive_dir.join("inner").join("Herta.pck");
        let patched_pck = root.join("patched.pck");
        fs::create_dir_all(source_archive.parent().expect("source parent"))
            .expect("create source parent");
        fs::create_dir_all(input_pck.parent().expect("pck parent")).expect("create pck parent");
        fs::create_dir_all(&config.state_dir).expect("create state");
        fs::write(active_root.join("marker.txt"), "old").expect("write marker");
        fs::write(&source_archive, "archive").expect("write archive");
        fs::write(&input_pck, "old pck").expect("write pck");
        fs::write(&patched_pck, "patched").expect("write patched");
        let context = TranslationContext {
            mod_key: Some("herta".to_string()),
            extraction_source_path: Some(source_archive),
            input_pck_path: None,
            pck_contents_root: None,
            pck_stem: Some("Herta".to_string()),
            translation_patch_source_path: None,
            translation_patch_pck_stem: None,
        };

        let installed = install_patched_archive_mod(
            &context,
            &archive_dir,
            &input_pck,
            &patched_pck,
            &config,
        )
        .expect("install archive");

        assert_eq!(installed, Some(active_root.clone()));
        assert!(!active_root.join("marker.txt").exists());
        assert!(active_root.join("Herta.pck").exists());
        assert!(!active_root.join("inner").exists());
        assert!(!config.game_mods_dir.join("HertaV1.42").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folder_loc_translation_applies_without_pck_repack() {
        let root = std::env::temp_dir().join(format!("sts2-folder-loc-apply-{}", timestamp_string()));
        let config = test_config(&root);
        let install_root = config.game_mods_dir.join("Blight");
        let active_source = install_root.join("blight").join("localization").join("zhs");
        fs::create_dir_all(&active_source).expect("create active loc");
        fs::create_dir_all(&config.translation_work_dir).expect("create translation work");
        fs::write(active_source.join("all.loc"), r#"{"title":"荒疫"}"#).expect("write active loc");

        let work_root = config
            .translation_work_dir
            .join("selected")
            .join("blight")
            .join("zhs");
        let source_root = work_root.join("source");
        let source_file = source_root
            .join("blight")
            .join("localization")
            .join("zhs")
            .join("all.loc");
        fs::create_dir_all(source_file.parent().expect("source parent")).expect("create source");
        fs::write(&source_file, r#"{"title":"荒疫"}"#).expect("write source");
        write_translation_context(TranslationContextWriteRequest {
            work_dir: &work_root,
            mod_key: "blight",
            resource_path: &display_path(active_source.as_path()),
            extraction_source: &install_root,
            pck_contents_root: None,
            pck_stem: "",
            translation_patch_source: None,
            translation_patch_pck_stem: None,
        })
        .expect("write context");

        let sheet_path = config
            .translation_work_dir
            .join("translation_memory")
            .join("blight")
            .join("all.kor.translation.json");
        fs::create_dir_all(sheet_path.parent().expect("sheet parent")).expect("create sheet dir");
        write_sheet(
            &sheet_path,
            &JsonTranslationSheet {
                source_path: source_file.to_string_lossy().to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 1,
                entries: vec![JsonTranslationEntry {
                    key: "/title".to_string(),
                    slot_id: Some("k001-aa".to_string()),
                    previous_source_value: None,
                    source_value: "荒疫".to_string(),
                    translated_value: "황역".to_string(),
                    status: JsonTranslationStatus::New,
                }],
            },
        )
        .expect("write sheet");

        let report =
            apply_sheet_and_pack_pck(&sheet_path, None, None, &config).expect("apply folder loc");
        let active_target = install_root
            .join("blight")
            .join("localization")
            .join("kor")
            .join("all.loc");

        assert_eq!(report.applied_entries, 1);
        assert_eq!(report.packed_pck_path, None);
        assert_eq!(report.installed_mod_path, Some(install_root.clone()));
        assert_eq!(
            fs::read_to_string(active_target).expect("read target loc"),
            "{\n  \"title\": \"황역\"\n}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn translation_memory_fallback_requires_same_source_resource() {
        let root =
            std::env::temp_dir().join(format!("sts2-memory-resource-{}", timestamp_string()));
        let memory = root.join("translation_memory").join("mod");
        fs::create_dir_all(&memory).expect("create memory");
        let eng_sheet = memory.join("localization-eng.kor.translation.json");
        let zhs_sheet = memory.join("localization-zhs.kor.translation.json");
        write_sheet(
            &eng_sheet,
            &JsonTranslationSheet {
                source_path: root
                    .join("selected")
                    .join("mod")
                    .join("eng")
                    .join("source")
                    .join("localization")
                    .join("eng")
                    .to_string_lossy()
                    .to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 1,
                entries: vec![JsonTranslationEntry {
                    key: "file://cards.json#/name".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "Name".to_string(),
                    translated_value: "이름".to_string(),
                    status: JsonTranslationStatus::Ready,
                }],
            },
        )
        .expect("write eng");
        write_sheet(
            &zhs_sheet,
            &JsonTranslationSheet {
                source_path: root
                    .join("selected")
                    .join("mod")
                    .join("zhs")
                    .join("source")
                    .join("localization")
                    .join("zhs")
                    .to_string_lossy()
                    .to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 1,
                entries: vec![JsonTranslationEntry {
                    key: "file://cards.json#/name".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "名称".to_string(),
                    translated_value: "이름".to_string(),
                    status: JsonTranslationStatus::Ready,
                }],
            },
        )
        .expect("write zhs");

        assert_eq!(
            fallback_translation_memory_sheet(&root, "mod", "kor", "res://localization/zhs"),
            Some(zhs_sheet.clone())
        );
        assert!(!translation_sheet_matches_resource(
            &eng_sheet,
            "res://localization/zhs"
        ));
        write_sheet(
            &zhs_sheet,
            &JsonTranslationSheet {
                source_path: root
                    .join("selected")
                    .join("mod")
                    .join("zhs")
                    .join("source")
                    .join("blight")
                    .join("localization")
                    .join("zhs")
                    .join("all.loc")
                    .to_string_lossy()
                    .to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 3,
                entries: vec![JsonTranslationEntry {
                    key: "/title".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "Name".to_string(),
                    translated_value: "이름".to_string(),
                    status: JsonTranslationStatus::Ready,
                }],
            },
        )
        .expect("write nested zhs loc");
        assert!(translation_sheet_matches_resource(
            &zhs_sheet,
            "res://localization/zhs"
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn translation_memory_fallback_prefers_translated_sibling_version_over_empty_current_sheet() {
        let root =
            std::env::temp_dir().join(format!("sts2-memory-sibling-{}", timestamp_string()));
        let memory_root = root.join("translation_memory");
        let current_memory = memory_root.join("blight-0-3-8");
        let sibling_memory = memory_root.join("blight-1-0");
        fs::create_dir_all(&current_memory).expect("create current memory");
        fs::create_dir_all(&sibling_memory).expect("create sibling memory");
        let current_sheet = current_memory.join("localization-zhs.kor.translation.json");
        let sibling_sheet = sibling_memory.join("localization-zhs.kor.translation.json");
        write_sheet(
            &current_sheet,
            &JsonTranslationSheet {
                source_path: root
                    .join("selected")
                    .join("blight-0-3-8")
                    .join("zhs")
                    .join("source")
                    .join("localization")
                    .join("zhs")
                    .to_string_lossy()
                    .to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 1,
                entries: vec![JsonTranslationEntry {
                    key: "file://all.loc#/title".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "荒疫".to_string(),
                    translated_value: String::new(),
                    status: JsonTranslationStatus::Missing,
                }],
            },
        )
        .expect("write empty current sheet");
        write_sheet(
            &sibling_sheet,
            &JsonTranslationSheet {
                source_path: root
                    .join("selected")
                    .join("blight-1-0")
                    .join("zhs")
                    .join("source")
                    .join("localization")
                    .join("zhs")
                    .to_string_lossy()
                    .to_string(),
                target_language: "kor".to_string(),
                updated_epoch: 2,
                entries: vec![JsonTranslationEntry {
                    key: "file://all.loc#/title".to_string(),
                    slot_id: None,
                    previous_source_value: None,
                    source_value: "荒疫".to_string(),
                    translated_value: "황역".to_string(),
                    status: JsonTranslationStatus::Ready,
                }],
            },
        )
        .expect("write sibling sheet");

        assert_eq!(
            select_translation_memory_sheet(
                &root,
                Some(current_sheet),
                &["blight-0-3-8".to_string(), "blight-1-0".to_string()],
                "kor",
                "res://localization/zhs",
            ),
            Some(sibling_sheet)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn translation_memory_family_tokens_use_manifest_name_for_version_only_rows() {
        let record = ModRecord {
            name: "0.3.8".to_string(),
            path: PathBuf::from(r"Z:\game\Slay the Spire 2\mods\Blight.zip"),
            source: ModSource::ExternalManager,
            kind: ModKind::Archive,
            version_hint: Some("0.3.8".to_string()),
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 1,
                modified: None,
            },
        };
        let manifest = ModManifestInfo {
            id: Some("Blight".to_string()),
            name: Some("荒疫".to_string()),
            version: Some("0.3.8".to_string()),
            ..ModManifestInfo::default()
        };

        let tokens = translation_memory_family_tokens(&record, &manifest);

        assert!(tokens.contains("blight"));
        assert!(tokens.contains("荒疫"));
        assert!(!tokens.contains("038"));
    }

    #[test]
    fn nested_mod_payload_dir_detects_single_inner_manifest_folder() {
        let root = std::env::temp_dir().join(format!("sts2-nested-mod-{}", timestamp_string()));
        let outer = root.join("CoolMod");
        let inner = outer.join("CoolMod");
        fs::create_dir_all(&inner).expect("create inner");
        fs::write(inner.join("CoolMod.json"), r#"{"name":"CoolMod","version":"1.0"}"#)
            .expect("write manifest");

        assert_eq!(nested_mod_payload_dir(&outer), Some(inner));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vortex_nested_mod_layout_is_not_an_install_warning() {
        let root = std::env::temp_dir().join(format!("sts2-vortex-nested-mod-{}", timestamp_string()));
        let mods = root.join("mods");
        let same_name_outer = mods.join("AscensionUnlockMod");
        let same_name_inner = same_name_outer.join("AscensionUnlockMod");
        let download_name_outer = mods.join("Miyu STS2-622-v1-0-2-7-12-9-1777882282");
        let download_name_inner = download_name_outer.join("Miyu_character");
        fs::create_dir_all(&same_name_inner).expect("create same-name inner");
        fs::create_dir_all(&download_name_inner).expect("create download-name inner");
        fs::write(
            mods.join("vortex.deployment.slaythespire2-mod.json"),
            r#"{"deploymentMethod":"symlink_activator_elevated"}"#,
        )
        .expect("write vortex marker");
        fs::write(
            same_name_inner.join("AscensionUnlockMod.json"),
            r#"{"name":"AscensionUnlockMod","version":"1.0"}"#,
        )
        .expect("write same-name manifest");
        fs::write(
            download_name_inner.join("Miyu_character.json"),
            r#"{"name":"Miyu","version":"1.0"}"#,
        )
        .expect("write download-name manifest");

        assert_eq!(nested_mod_payload_dir(&same_name_outer), Some(same_name_inner));
        assert!(is_vortex_nested_mod_layout(&same_name_outer));
        assert!(is_vortex_nested_mod_layout(&download_name_outer));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recent_vortex_download_archive_is_marked_downloading() {
        let record = ModRecord {
            name: "AkiSister".to_string(),
            path: PathBuf::from(
                r"C:\Users\angel\AppData\Roaming\Vortex\downloads\slaythespire2\AkiSister.rar",
            ),
            source: ModSource::ExternalManager,
            kind: ModKind::Archive,
            version_hint: None,
            fingerprint: ModFingerprint {
                bytes: 1024,
                modified: Some(SystemTime::now()),
            },
        };

        assert_eq!(
            download_state_for_record(&record).as_deref(),
            Some("downloading")
        );
    }

    #[test]
    fn log_classifier_categorizes_pck_and_lock_failures() {
        let pck = classify_game_log_line("[Error] failed to load mod payload.pck").expect("pck");
        let locked = classify_game_log_line("Access is denied while moving mod file").expect("lock");

        assert_eq!(pck.0, "pck");
        assert_eq!(locked.0, "locked");
    }

    #[test]
    fn log_classifier_marks_model_not_found_as_current_run_conflict() {
        let current_run = classify_game_log_line(
            "ERROR: MegaCrit.Sts2.Core.Models.Exceptions.ModelNotFoundException: Model id=CHARACTER.MIYU_CHARACTER not found",
        )
        .expect("current-run conflict");

        assert_eq!(current_run.0, "current-run");
        assert!(current_run.1.contains("진행 중 런"));
    }

    #[test]
    fn mod_safety_warnings_ignore_manifest_keywords() {
        let manifest = ModManifestInfo {
            id: None,
            name: Some("Co-op Sync".to_string()),
            version: None,
            author: None,
            description: Some("Adds online multiplayer save sync".to_string()),
            dependencies: Vec::new(),
            target_mod_id: None,
            target_mod_name: None,
            target_mod_version: None,
            target_languages: Vec::new(),
            is_translation_patch: false,
        };

        assert!(mod_safety_warnings(&manifest).is_empty());
    }

    #[test]
    fn extraction_tree_directory_nodes_keep_resource_paths() {
        let root = std::env::temp_dir().join(format!("sts2-tree-resource-path-{}", timestamp_string()));
        let contents = root.join("AkiSister.pck.contents");
        fs::create_dir_all(contents.join("AkiSister").join("localization").join("zhs"))
            .expect("create localization");
        fs::write(
            contents
                .join("AkiSister")
                .join("localization")
                .join("zhs")
                .join("cards.json"),
            r#"{"name":"旅程"}"#,
        )
        .expect("write cards");

        let tree = extraction_tree(&root, "test-tree-resource-path", Path::new("missing-vendor"));
        let resource_root = tree.first().expect("resource root");
        let mod_node = resource_root
            .children
            .iter()
            .find(|node| node.name == "AkiSister")
            .expect("mod node");
        let localization_node = mod_node
            .children
            .iter()
            .find(|node| node.name == "localization")
            .expect("localization node");
        let zhs_node = localization_node
            .children
            .iter()
            .find(|node| node.name == "zhs")
            .expect("zhs node");

        assert_eq!(resource_root.path, "res://");
        assert_eq!(mod_node.path, "res://AkiSister");
        assert_eq!(localization_node.path, "res://AkiSister/localization");
        assert_eq!(zhs_node.path, "res://AkiSister/localization/zhs");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn selected_translation_files_accepts_directory_names_from_old_tree_nodes() {
        let root =
            std::env::temp_dir().join(format!("sts2-selected-resource-fallback-{}", timestamp_string()));
        let localization = root
            .join("AkiSister.pck.contents")
            .join("AkiSister")
            .join("localization")
            .join("zhs");
        fs::create_dir_all(&localization).expect("create localization");
        fs::write(localization.join("cards.json"), r#"{"name":"旅程"}"#).expect("write cards");
        fs::write(localization.join("relics.json"), r#"{"name":"錨"}"#).expect("write relics");

        assert_eq!(
            selected_translation_files(&root, "res://AkiSister/localization").len(),
            2
        );
        assert_eq!(selected_translation_files(&root, "localization").len(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn selected_translation_files_accepts_loc_language_dirs() {
        let root = std::env::temp_dir().join(format!("sts2-selected-loc-resource-{}", timestamp_string()));
        let localization = root.join("Blight").join("localization").join("zhs");
        fs::create_dir_all(&localization).expect("create localization");
        fs::write(
            localization.join("all.loc"),
            r#"{"BLIGHT_BUTTON.title":"Blight Mode"}"#,
        )
        .expect("write loc");
        let localization_display = display_path(&localization);

        assert_eq!(
            default_translation_resource_path(&root, "kor").as_deref(),
            Some(localization_display.as_str())
        );
        assert_eq!(
            selected_translation_files(&root, &localization_display).len(),
            1
        );
        assert_eq!(
            selected_translation_files(&root, &display_path(&localization.join("all.loc"))).len(),
            1
        );
        assert_eq!(
            selected_translation_files(&root, "Blight/localization/zhs").len(),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ui_settings_prefills_detected_game_exe() {
        let root = std::env::temp_dir().join(format!("sts2-settings-game-exe-{}", timestamp_string()));
        let config = test_config(&root);
        let exe = config.game_dir.join("SlayTheSpire2.exe");
        fs::create_dir_all(&config.game_dir).expect("create game dir");
        fs::write(&exe, "fake exe").expect("write exe");

        let settings = read_ui_settings(&config).expect("read settings");

        assert_eq!(settings.game_exe_path, display_path(&exe));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ui_settings_empty_game_exe_keeps_detected_default() {
        let root = std::env::temp_dir().join(format!("sts2-settings-empty-game-exe-{}", timestamp_string()));
        let config = test_config(&root);
        let exe = config.game_dir.join("SlayTheSpire2.exe");
        fs::create_dir_all(&config.game_dir).expect("create game dir");
        fs::create_dir_all(&config.state_dir).expect("create state dir");
        fs::write(&exe, "fake exe").expect("write exe");
        fs::write(
            config.state_dir.join("tauri_settings.tsv"),
            "target_language\tkor\ngame_exe_path\t\n",
        )
        .expect("write settings");

        let settings = read_ui_settings(&config).expect("read settings");

        assert_eq!(settings.game_exe_path, display_path(&exe));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_settings_request_accepts_tauri_camel_case_args() {
        let request: SaveSettingsRequest = serde_json::from_value(serde_json::json!({
            "translationWorkDir": "translation_work",
            "targetLanguage": "kor",
            "gameExePath": "",
            "gameLogPath": "",
            "saveDir": "saves",
            "saveBackupDir": "backups",
            "saveBackupRetentionDays": 7,
            "saveBackupMaxEntries": 14,
            "deletedRetentionDays": 30,
            "modViewMode": "detail"
        }))
        .expect("deserialize save settings request");

        assert_eq!(request.translation_work_dir, "translation_work");
        assert_eq!(request.save_backup_max_entries, 14);
        assert_eq!(request.mod_view_mode, "detail");
    }

    #[test]
    fn setup_issues_warn_when_work_paths_overlap() {
        let root = std::env::temp_dir().join(format!("sts2-settings-overlap-{}", timestamp_string()));
        let config = test_config(&root);
        let settings = UiSettingsDto {
            translation_work_dir: root.join("shared").join("translation").display().to_string(),
            target_language: "kor".to_string(),
            game_exe_path: String::new(),
            game_log_path: String::new(),
            save_dir: root.join("saves").display().to_string(),
            save_backup_dir: root.join("shared").display().to_string(),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            deleted_retention_days: 30,
            mod_view_mode: "detail".to_string(),
        };
        let launch = LaunchStatus {
            ready: true,
            game_exe: Some(config.game_dir.join("SlayTheSpire2.exe")),
            steam_exe: None,
            running: false,
        };

        let issues = setup_issues(&config, &settings, &launch);

        assert!(issues.iter().any(|issue| {
            !issue.blocking
                && issue.message.contains("세이브 백업 경로")
                && issue.message.contains("번역/추출 작업 경로")
                && issue.message.contains("포함 관계")
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn current_run_log_warning_requires_existing_modded_current_run() {
        let root = std::env::temp_dir().join(format!("sts2-current-run-log-{}", timestamp_string()));
        let config = test_config(&root);
        fs::create_dir_all(&config.logs_dir).expect("logs");
        let log = config.logs_dir.join("godot.log");
        fs::write(
            &log,
            "ERROR: MegaCrit.Sts2.Core.Models.Exceptions.ModelNotFoundException: Model id=CHARACTER.MIYU_CHARACTER not found\n",
        )
        .expect("write log");
        let settings = UiSettingsDto {
            translation_work_dir: config.translation_work_dir.display().to_string(),
            target_language: "kor".to_string(),
            game_exe_path: String::new(),
            game_log_path: log.display().to_string(),
            save_dir: config.save_dir.as_ref().unwrap().display().to_string(),
            save_backup_dir: config.save_backup_dir.display().to_string(),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            deleted_retention_days: 30,
            mod_view_mode: "detail".to_string(),
        };

        assert!(log_diagnostics(&config, &settings, &[]).is_empty());

        let current_run = config
            .save_dir
            .as_ref()
            .unwrap()
            .join("modded")
            .join("profile1")
            .join("saves")
            .join("current_run.save");
        fs::create_dir_all(current_run.parent().unwrap()).expect("current run parent");
        fs::write(
            &current_run,
            r#"{ "players": [{ "character_id": "CHARACTER.MIYU_CHARACTER" }] }"#,
        )
        .expect("write current run");

        let diagnostics = log_diagnostics(&config, &settings, &[]);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].detail.contains("modded"));
        let _ = fs::remove_dir_all(root);
    }

    fn test_config(root: &Path) -> AppConfig {
        AppConfig {
            workspace_dir: root.to_path_buf(),
            game_dir: root.join("game"),
            game_mods_dir: root.join("game").join("mods"),
            game_exe_path: None,
            save_dir: Some(root.join("saves")),
            save_backup_dir: root.join("backups"),
            save_backup_retention_days: 7,
            save_backup_max_entries: 14,
            presets_dir: root.join("presets"),
            translation_work_dir: root.join("translation_work"),
            logs_dir: root.join("logs"),
            state_dir: root.join("state"),
            mod_index_path: root.join("state").join("mod_index.tsv"),
            vendor_dir: root.join("vendor"),
            external_manager_dirs: Vec::new(),
        }
    }
}

