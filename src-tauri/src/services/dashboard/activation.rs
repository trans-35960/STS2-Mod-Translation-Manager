fn find_mod_record(app: &App, key: &str) -> Result<ModRecord, String> {
    let summary = app
        .scan_preview_report()
        .map_err(|error| error.to_string())?
        .summary;
    summary.into_records()
        .find(|record| record.stable_key() == key)
        .ok_or_else(|| format!("{key} 모드를 찾을 수 없습니다."))
}

