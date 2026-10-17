//! Predicts the order Slay the Spire 2 loads mods in.
//!
//! The game (`ModManager.SortModList`) sorts the mods it will load with Kahn's
//! topological sort over manifest `dependencies`, and breaks ties with each
//! mod's position in `settings.save`'s `mod_list` (matched by id only; mods
//! missing from the list go last). Later mods win when two mods ship the same
//! loc key or PCK resource, so the order is worth controlling.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::fs;
use std::path::Path;

/// One mod the game will try to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOrderMod {
    /// Manifest `id`, which is what the game's settings and dependencies use.
    pub id: String,
    /// `game_settings::SOURCE_MODS_DIRECTORY` or `SOURCE_WORKSHOP`.
    pub source: String,
    /// Manifest `dependencies` ids.
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMod {
    /// Index into the `mods` passed to [`resolve`].
    pub index: usize,
    /// Whether the saved order names this mod; unlisted mods go last in an
    /// order the game picks itself.
    pub listed: bool,
    /// Dependencies placed after this mod in the saved order, which the game
    /// loads first anyway.
    pub waits_for: Vec<String>,
    /// Dependencies that are not going to be loaded, so this mod fails.
    pub missing_dependencies: Vec<String>,
    /// Part of a dependency cycle, so the game refuses to load it.
    pub circular: bool,
}

/// Returns `mods` in the order the game loads them, given the saved order of
/// mod ids. Ties the game leaves open (unlisted mods) keep the order of `mods`.
pub fn resolve(saved_order: &[String], mods: &[LoadOrderMod]) -> Vec<ResolvedMod> {
    // The game fills a dictionary in list order, so a repeated id keeps its last slot.
    let mut priority_by_id = BTreeMap::<&str, usize>::new();
    for (position, id) in saved_order.iter().enumerate() {
        priority_by_id.insert(id.as_str(), position);
    }
    let priority = |index: usize| {
        let listed = priority_by_id.get(mods[index].id.as_str()).copied();
        (listed.unwrap_or(usize::MAX), index)
    };
    let find = |id: &str| mods.iter().position(|candidate| candidate.id == id);

    let mut in_degree = vec![0usize; mods.len()];
    let mut dependents = vec![Vec::<usize>::new(); mods.len()];
    for (index, entry) in mods.iter().enumerate() {
        for dependency in &entry.dependencies {
            if let Some(found) = find(dependency) {
                in_degree[index] += 1;
                dependents[found].push(index);
            }
        }
    }

    let mut queue = BinaryHeap::new();
    for (index, &degree) in in_degree.iter().enumerate() {
        if degree == 0 {
            queue.push(Reverse((priority(index), index)));
        }
    }
    let mut sorted = Vec::with_capacity(mods.len());
    while let Some(Reverse((_, index))) = queue.pop() {
        sorted.push(index);
        for &dependent in &dependents[index] {
            in_degree[dependent] -= 1;
            if in_degree[dependent] == 0 {
                queue.push(Reverse((priority(dependent), dependent)));
            }
        }
    }
    let mut circular = vec![false; mods.len()];
    for (index, in_cycle) in circular.iter_mut().enumerate() {
        if !sorted.contains(&index) {
            *in_cycle = true;
            sorted.push(index);
        }
    }

    sorted
        .into_iter()
        .map(|index| {
            let entry = &mods[index];
            let own = priority(index);
            let mut waits_for = Vec::new();
            let mut missing_dependencies = Vec::new();
            for dependency in &entry.dependencies {
                match find(dependency) {
                    Some(found) if priority(found).0 > own.0 => waits_for.push(dependency.clone()),
                    Some(_) => {}
                    None => missing_dependencies.push(dependency.clone()),
                }
            }
            ResolvedMod {
                index,
                listed: own.0 != usize::MAX,
                waits_for,
                missing_dependencies,
                circular: circular[index],
            }
        })
        .collect()
}

/// Reads a manifest's `id` and dependency ids. Accepts both the current
/// `[{ "id": .., "min_version": .. }]` form and the old list of plain ids.
pub fn read_manifest_load_info(manifest_path: &Path) -> Option<(String, Vec<String>)> {
    let text = fs::read_to_string(manifest_path).ok()?;
    let value =
        serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok()?;
    let id = value.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let dependencies = value
        .get("dependencies")
        .and_then(|dependencies| dependencies.as_array())
        .map(|dependencies| {
            dependencies
                .iter()
                .filter_map(|dependency| {
                    dependency
                        .as_str()
                        .or_else(|| dependency.get("id")?.as_str())
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    Some((id.to_string(), dependencies))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, dependencies: &[&str]) -> LoadOrderMod {
        LoadOrderMod {
            id: id.to_string(),
            source: "steam_workshop".to_string(),
            dependencies: dependencies.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn order(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    fn ids(mods: &[LoadOrderMod], resolved: &[ResolvedMod]) -> Vec<String> {
        resolved
            .iter()
            .map(|entry| mods[entry.index].id.clone())
            .collect()
    }

    #[test]
    fn follows_saved_order_without_dependencies() {
        let mods = vec![entry("A", &[]), entry("B", &[]), entry("C", &[])];
        let resolved = resolve(&order(&["C", "A", "B"]), &mods);
        assert_eq!(ids(&mods, &resolved), ["C", "A", "B"]);
        assert!(
            resolved
                .iter()
                .all(|entry| entry.listed && entry.waits_for.is_empty())
        );
    }

    #[test]
    fn dependencies_load_first_even_when_saved_later() {
        let mods = vec![
            entry("Lib", &[]),
            entry("Char", &["Lib"]),
            entry("Skin", &[]),
        ];
        let resolved = resolve(&order(&["Char", "Skin", "Lib"]), &mods);
        // Char waits for Lib; Skin has nothing to wait for and keeps its slot.
        assert_eq!(ids(&mods, &resolved), ["Skin", "Lib", "Char"]);
        let char_entry = resolved
            .iter()
            .find(|entry| entry.index == 1)
            .expect("char");
        assert_eq!(char_entry.waits_for, ["Lib"]);
    }

    #[test]
    fn unlisted_mods_go_last_in_given_order() {
        let mods = vec![entry("New2", &[]), entry("Old", &[]), entry("New1", &[])];
        let resolved = resolve(&order(&["Old"]), &mods);
        assert_eq!(ids(&mods, &resolved), ["Old", "New2", "New1"]);
        assert!(!resolved[1].listed);
    }

    #[test]
    fn flags_missing_and_circular_dependencies() {
        let mods = vec![
            entry("A", &["B"]),
            entry("B", &["A"]),
            entry("C", &["Gone"]),
        ];
        let resolved = resolve(&order(&["A", "B", "C"]), &mods);
        assert_eq!(ids(&mods, &resolved), ["C", "A", "B"]);
        assert_eq!(resolved[0].missing_dependencies, ["Gone"]);
        assert!(!resolved[0].circular);
        assert!(resolved[1].circular && resolved[2].circular);
    }

    #[test]
    fn reads_both_dependency_forms() {
        let dir = std::env::temp_dir().join(format!("sts2mm-load-order-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("dir");
        let new_style = dir.join("new.json");
        fs::write(
            &new_style,
            r#"{"id":"Char","dependencies":[{"id":"BaseLib","min_version":"1.0.0"}]}"#,
        )
        .expect("write");
        let old_style = dir.join("old.json");
        fs::write(
            &old_style,
            "\u{feff}{\"id\":\"Old\",\"dependencies\":[\"BaseLib\"]}",
        )
        .expect("write");

        assert_eq!(
            read_manifest_load_info(&new_style),
            Some(("Char".to_string(), vec!["BaseLib".to_string()]))
        );
        assert_eq!(
            read_manifest_load_info(&old_style),
            Some(("Old".to_string(), vec!["BaseLib".to_string()]))
        );
        let _ = fs::remove_dir_all(dir);
    }
}
