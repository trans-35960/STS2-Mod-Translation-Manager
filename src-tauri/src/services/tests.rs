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
    fn translation_patch_targets_base_manifest_id_when_folder_name_differs() {
        let base_record = ModRecord {
            name: "Miyu STS2-622-v1-0-2-7-12-9-1777882282".to_string(),
            path: PathBuf::from(r"Z:\game\Slay the Spire 2\mods.disabled\Miyu"),
            source: ModSource::Vault,
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
            source: ModSource::Vault,
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

        cleanup_translation_memory_payloads(&memory_root, &mut removed_dirs, &mut removed_files)
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
            source: sts2_mod_manager::domain::ModSource::Vault,
            kind: ModKind::Archive,
            version_hint: None,
            fingerprint: sts2_mod_manager::domain::ModFingerprint {
                bytes: 1,
                modified: None,
            },
        };
        let keys = connected_mod_keys(&ScanSummary {
            game_mods: vec![active],
            vault_mods: vec![disabled],
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
            vault_mods: Vec::new(),
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
            pck_stem: Some("Mod".to_string()),
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
            pck_stem: Some("Herta".to_string()),
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
        assert!(active_root.join("inner").join("Herta.pck").exists());
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
        write_translation_context(
            &work_root,
            "blight",
            &display_path(active_source.as_path()),
            &install_root,
            None,
            "",
        )
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
                entries: Vec::new(),
            },
        )
        .expect("write zhs");

        assert_eq!(
            fallback_translation_memory_sheet(&root, "mod", "kor", "res://localization/zhs"),
            Some(zhs_sheet)
        );
        assert!(!translation_sheet_matches_resource(
            &eng_sheet,
            "res://localization/zhs"
        ));
        let _ = fs::remove_dir_all(root);
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
    fn log_classifier_categorizes_pck_and_lock_failures() {
        let pck = classify_game_log_line("[Error] failed to load mod payload.pck").expect("pck");
        let locked = classify_game_log_line("Access is denied while moving mod file").expect("lock");

        assert_eq!(pck.0, "pck");
        assert_eq!(locked.0, "locked");
    }

    #[test]
    fn mod_safety_warnings_detect_multiplayer_manifest_text() {
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

        assert!(!mod_safety_warnings(&manifest).is_empty());
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
            vault_dir: root.join("vault"),
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

