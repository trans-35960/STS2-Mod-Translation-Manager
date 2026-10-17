/// The game's load order for the mods set active here. With `draft`, shows
/// how that order would load instead of the saved one.
pub(crate) fn load_mod_load_order(
    draft: Option<Vec<ModOrderKeyDto>>,
) -> Result<ModLoadOrderDto, String> {
    let app = app();
    let settings_path = app.config().game_settings_path();
    let settings_found = settings_path.as_deref().is_some_and(Path::is_file);
    let saved = match draft {
        Some(draft) => draft,
        None => settings_path
            .as_deref()
            .map(game_settings::read_mod_entries)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| ModOrderKeyDto {
                id: entry.id,
                source: entry.source,
            })
            .collect(),
    };
    let mods = app.load_order_mods().map_err(|error| error.to_string())?;
    Ok(ModLoadOrderDto {
        settings_found,
        game_running: app.game_running(),
        entries: load_order_entries(&saved, mods),
    })
}

/// Saves the order to `settings.save`. The bridge stays first.
pub(crate) fn save_mod_load_order(order: Vec<ModOrderKeyDto>) -> Result<ModLoadOrderDto, String> {
    let app = app();
    if app.game_running() {
        return Err("게임이 실행 중이면 로드 순서를 바꿀 수 없습니다. 게임을 종료한 뒤 다시 시도하세요.".to_string());
    }
    let Some(settings_path) = app
        .config()
        .game_settings_path()
        .filter(|path| path.is_file())
    else {
        return Err("게임 설정 파일(settings.save)이 아직 없습니다. 게임을 한 번 실행한 뒤 다시 시도하세요.".to_string());
    };
    let mut order = order
        .into_iter()
        .filter(|entry| !is_bridge(&entry.id, &entry.source))
        .map(|entry| (entry.id, entry.source))
        .collect::<Vec<_>>();
    order.insert(
        0,
        (
            bridge::BRIDGE_MOD_ID.to_string(),
            game_settings::SOURCE_MODS_DIRECTORY.to_string(),
        ),
    );
    game_settings::write_mod_order(&settings_path, &app.config().save_backup_dir, &order)
        .map_err(|error| error.to_string())?;
    load_mod_load_order(None)
}

/// Lists `mods` in the edited order (`saved` first, the rest after in scan
/// order, the bridge always on top), each with where the game really loads it.
fn load_order_entries(
    saved: &[ModOrderKeyDto],
    mods: Vec<(String, LoadOrderMod)>,
) -> Vec<ModLoadOrderEntryDto> {
    let position = |entry: &LoadOrderMod| {
        if is_bridge(&entry.id, &entry.source) {
            return 0;
        }
        saved
            .iter()
            .position(|key| key.id == entry.id && key.source == entry.source)
            .map_or(usize::MAX, |index| index + 1)
    };
    let mut mods = mods;
    mods.sort_by_key(|(_, entry)| position(entry));
    let listed = mods
        .iter()
        .filter(|(_, entry)| position(entry) != usize::MAX)
        .map(|(_, entry)| entry.id.clone())
        .collect::<Vec<_>>();
    let candidates = mods.iter().map(|(_, entry)| entry.clone()).collect::<Vec<_>>();
    let resolved = sts2_mod_manager::load_order::resolve(&listed, &candidates);

    let mut entries = mods
        .into_iter()
        .map(|(key, entry)| ModLoadOrderEntryDto {
            pinned: is_bridge(&entry.id, &entry.source),
            id: entry.id,
            source: entry.source,
            key,
            load_index: 0,
            listed: false,
            waits_for: Vec::new(),
            missing_dependencies: Vec::new(),
            circular: false,
        })
        .collect::<Vec<_>>();
    for (load_index, resolved) in resolved.into_iter().enumerate() {
        let entry = &mut entries[resolved.index];
        entry.load_index = load_index;
        entry.listed = resolved.listed;
        entry.waits_for = resolved.waits_for;
        entry.missing_dependencies = resolved.missing_dependencies;
        entry.circular = resolved.circular;
    }
    entries
}

fn is_bridge(id: &str, source: &str) -> bool {
    id == bridge::BRIDGE_MOD_ID && source == game_settings::SOURCE_MODS_DIRECTORY
}
