fn json_report_dto(report: JsonSheetReport) -> JsonSheetReportDto {
    JsonSheetReportDto {
        sheet_path: display_path(&report.sheet_path),
        entries: report.entries,
        new_entries: report.new_entries,
        updated_entries: report.updated_entries,
        missing_entries: report.missing_entries,
        removed_entries: report.removed_entries,
    }
}

fn json_sheet_from_dto(sheet: JsonSheetDto) -> Result<JsonTranslationSheet, String> {
    Ok(JsonTranslationSheet {
        source_path: sheet.source_path,
        target_language: sheet.target_language,
        updated_epoch: epoch_seconds(Some(SystemTime::now())).unwrap_or(sheet.updated_epoch),
        entries: sheet
            .entries
            .into_iter()
            .map(json_entry_from_dto)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn json_entry_from_dto(entry: JsonEntryDto) -> Result<JsonTranslationEntry, String> {
    Ok(JsonTranslationEntry {
        key: entry.key,
        slot_id: entry.slot_id,
        previous_source_value: entry.previous_source_value,
        source_value: entry.source_value,
        translated_value: entry.translated_value,
        status: match entry.status.as_str() {
            "new" => JsonTranslationStatus::New,
            "ready" => JsonTranslationStatus::Ready,
            "updated" => JsonTranslationStatus::Updated,
            "missing" => JsonTranslationStatus::Missing,
            "removed" => JsonTranslationStatus::Removed,
            other => return Err(format!("알 수 없는 번역 상태: {other}")),
        },
    })
}

fn json_sheet_dto(sheet: sts2_mod_manager::json_translation::JsonTranslationSheet) -> JsonSheetDto {
    JsonSheetDto {
        source_path: sheet.source_path,
        target_language: sheet.target_language,
        updated_epoch: sheet.updated_epoch,
        entries: sheet.entries.into_iter().map(json_entry_dto).collect(),
    }
}

fn json_entry_dto(entry: JsonTranslationEntry) -> JsonEntryDto {
    JsonEntryDto {
        key: entry.key,
        slot_id: entry.slot_id,
        previous_source_value: entry.previous_source_value,
        source_value: entry.source_value,
        translated_value: entry.translated_value,
        status: status_label(entry.status).to_string(),
    }
}

fn status_label(status: JsonTranslationStatus) -> &'static str {
    match status {
        JsonTranslationStatus::New => "new",
        JsonTranslationStatus::Ready => "ready",
        JsonTranslationStatus::Updated => "updated",
        JsonTranslationStatus::Missing => "missing",
        JsonTranslationStatus::Removed => "removed",
    }
}

fn json_validation_dto(report: JsonValidationReport) -> JsonValidationDto {
    JsonValidationDto {
        valid: report.valid,
        total_entries: report.total_entries,
        missing_entries: report.missing_entries,
        updated_entries: report.updated_entries,
        removed_entries: report.removed_entries,
        format_issues: report
            .format_issues
            .into_iter()
            .map(|issue| JsonValidationIssueDto {
                key: issue.key,
                kind: issue.kind,
                message: issue.message,
            })
            .collect(),
    }
}

fn json_apply_dto(report: PckPatchReport) -> JsonApplyDto {
    JsonApplyDto {
        output_path: report
            .packed_pck_path
            .as_deref()
            .map(display_path)
            .unwrap_or_else(|| display_path(&report.language_output_path)),
        applied_entries: report.applied_entries,
        language_output_path: display_path(&report.language_output_path),
        packed_pck_path: report
            .packed_pck_path
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
        installed_mod_path: report
            .installed_mod_path
            .as_deref()
            .map(display_path)
            .unwrap_or_default(),
    }
}

fn json_import_dto(report: JsonImportReport) -> JsonImportDto {
    JsonImportDto {
        input_path: display_path(&report.input_path),
        matched_entries: report.matched_entries,
        unmatched_entries: report.unmatched_entries,
    }
}
