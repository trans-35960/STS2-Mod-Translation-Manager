/// Scans a mod and its translation patches for text still in a script the
/// target language does not use (see `sts2_mod_manager::leftover_text`).
pub(crate) fn scan_leftover_text(
    key: String,
    patch_keys: Vec<String>,
    target_language: String,
) -> Result<LeftoverReportDto, String> {
    let app = app();
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    let records = summary.into_records().collect::<Vec<_>>();
    let find = |key: &str| {
        records
            .iter()
            .find(|record| record.stable_key() == key)
            .ok_or_else(|| format!("{key} 모드를 찾을 수 없습니다."))
    };
    let vendor_dir = &app.config().vendor_dir;
    let mod_path = leftover_scan_path(find(&key)?, vendor_dir)?;
    let patch_paths = patch_keys
        .iter()
        .map(|patch_key| leftover_scan_path(find(patch_key)?, vendor_dir))
        .collect::<Result<Vec<_>, String>>()?;
    let report = leftover_text::scan_leftover_text(
        &mod_path,
        &patch_paths.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
        &target_language,
    )
    .map_err(|error| error.to_string())?;
    Ok(leftover_report_dto(report, target_language))
}

/// Folders, `.pck` and `.dll` files are scanned in place; archives are read
/// from the same expanded copy the language preview uses.
fn leftover_scan_path(record: &ModRecord, vendor_dir: &Path) -> Result<PathBuf, String> {
    let path = &record.path;
    let readable_in_place = path.is_dir()
        || path.extension().and_then(|value| value.to_str()).is_some_and(|extension| {
            matches!(extension.to_ascii_lowercase().as_str(), "pck" | "dll")
        });
    if readable_in_place {
        return Ok(path.clone());
    }
    let source = extraction_source_for_record(record);
    let cache_key = language_cache_key(record, &source, vendor_dir);
    extraction_scan_root(&source, &cache_key, vendor_dir).ok_or_else(|| {
        format!(
            "{} 압축을 풀지 못해 검사할 수 없습니다. 추출로 파일 트리를 먼저 만들어 주세요.",
            record.name
        )
    })
}

fn leftover_report_dto(
    report: leftover_text::LeftoverReport,
    target_language: String,
) -> LeftoverReportDto {
    let total_texts = report.total_texts();
    let code_like_texts = report
        .files
        .iter()
        .flat_map(|file| &file.texts)
        .filter(|text| text.code_like)
        .count();
    LeftoverReportDto {
        target_language,
        scanned_files: report.scanned_files,
        total_texts,
        code_like_texts,
        unreadable: report.unreadable,
        files: report
            .files
            .into_iter()
            .map(|file| LeftoverFileDto {
                location: match file.location {
                    leftover_text::LeftoverLocation::Dll => "dll",
                    leftover_text::LeftoverLocation::Resource => "resource",
                    leftover_text::LeftoverLocation::Patch => "patch",
                }
                .to_string(),
                file: file.file,
                texts: file
                    .texts
                    .into_iter()
                    .map(|text| LeftoverTextDto {
                        text: text.text,
                        code_like: text.code_like,
                    })
                    .collect(),
            })
            .collect(),
    }
}
