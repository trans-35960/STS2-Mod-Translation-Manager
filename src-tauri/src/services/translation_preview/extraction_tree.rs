fn extraction_tree(
    source: &Path,
    cache_key: &str,
    vendor_dir: &Path,
) -> Vec<ExtractionTreeNodeDto> {
    if source.is_file() && !is_supported_extractable_path(source) {
        let name = source
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| display_path(source));
        return vec![ExtractionTreeNodeDto {
            name,
            path: display_path(source),
            kind: "file".to_string(),
            source_path: display_path(source),
            children: Vec::new(),
        }];
    }

    let Some(scan_root) = full_extraction_scan_root(source, cache_key, vendor_dir) else {
        return Vec::new();
    };
    let language_paths = scan_translation_candidates(&scan_root)
        .map(|candidates| {
            candidates
                .into_iter()
                .filter(|candidate| !is_hardcoded_source_file(&candidate.path))
                .map(|candidate| candidate.path)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let hardcoded_paths = hardcoded_candidate_paths(&scan_root);

    let resource_roots = pck_resource_roots(&scan_root);
    if !resource_roots.is_empty() {
        let mut root = TreeNodeBuilder::named("res://", "res://");
        let mut files = Vec::new();
        for resource_root in resource_roots {
            collect_preview_files(&resource_root, &mut files, 3000);
        }
        files.sort();
        files.dedup();
        for file in files {
            if let Some(relative) = pck_resource_relative_path(&file) {
                root.insert_display(
                    &relative,
                    &format!("res://{}", slash_path(&relative)),
                    &file,
                    if language_paths.contains(&file) {
                        "language"
                    } else if hardcoded_paths.contains(&file) {
                        "hardcoded"
                    } else {
                        "file"
                    },
                );
            }
        }
        return vec![root.into_dto()];
    }

    let mut files = Vec::new();
    collect_preview_files(&scan_root, &mut files, 1200);
    files.sort();

    let mut root = TreeNodeBuilder::root();
    for file in files {
        let relative = file.strip_prefix(&scan_root).unwrap_or(&file);
        root.insert(
            relative,
            &file,
            if language_paths.contains(&file) {
                "language"
            } else if hardcoded_paths.contains(&file) {
                "hardcoded"
            } else {
                "file"
            },
        );
    }
    root.into_children()
}


fn collect_preview_files(root: &Path, files: &mut Vec<PathBuf>, limit: usize) {
    if files.len() >= limit {
        return;
    }

    let Ok(metadata) = fs::metadata(root) else {
        return;
    };
    if metadata.is_file() {
        files.push(root.to_path_buf());
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        if files.len() >= limit {
            return;
        }
        collect_preview_files(&entry, files, limit);
    }
}

struct TreeNodeBuilder {
    name: String,
    path: String,
    source_path: String,
    kind: String,
    children: BTreeMap<String, TreeNodeBuilder>,
}

impl TreeNodeBuilder {
    fn root() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            source_path: String::new(),
            kind: "dir".to_string(),
            children: BTreeMap::new(),
        }
    }

    fn named(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            source_path: String::new(),
            kind: "dir".to_string(),
            children: BTreeMap::new(),
        }
    }

    fn insert(&mut self, relative: &Path, absolute: &Path, leaf_kind: &str) {
        self.insert_display(relative, &display_path(absolute), absolute, leaf_kind);
    }

    fn insert_display(
        &mut self,
        relative: &Path,
        display: &str,
        source_path: &Path,
        leaf_kind: &str,
    ) {
        let components = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let display_paths = tree_display_paths(&components, display, source_path);
        let source_paths = tree_source_paths(&components, source_path);
        self.insert_components(&components, &display_paths, &source_paths, leaf_kind);
    }

    fn insert_components(
        &mut self,
        components: &[String],
        display_paths: &[String],
        source_paths: &[String],
        leaf_kind: &str,
    ) {
        let Some((name, rest)) = components.split_first() else {
            return;
        };
        let display = display_paths.first().cloned().unwrap_or_default();
        let source_path = source_paths.first().cloned().unwrap_or_default();
        let child = self
            .children
            .entry(name.clone())
            .or_insert_with(|| TreeNodeBuilder {
                name: name.clone(),
                path: display.clone(),
                source_path: source_path.clone(),
                kind: if rest.is_empty() {
                    leaf_kind.to_string()
                } else {
                    "dir".to_string()
                },
                children: BTreeMap::new(),
            });
        if rest.is_empty() {
            child.path = display;
            child.source_path = source_path;
            child.kind = leaf_kind.to_string();
            return;
        }
        child.path = display;
        child.source_path = source_path;
        child.insert_components(
            rest,
            display_paths.get(1..).unwrap_or(&[]),
            source_paths.get(1..).unwrap_or(&[]),
            leaf_kind,
        );
    }

    fn into_children(self) -> Vec<ExtractionTreeNodeDto> {
        self.children
            .into_values()
            .map(TreeNodeBuilder::into_dto)
            .collect()
    }

    fn into_dto(self) -> ExtractionTreeNodeDto {
        let TreeNodeBuilder {
            name,
            path,
            source_path,
            kind,
            children,
        } = self;
        ExtractionTreeNodeDto {
            name,
            path,
            source_path,
            kind,
            children: children
                .into_values()
                .map(TreeNodeBuilder::into_dto)
                .collect(),
        }
    }
}

fn hardcoded_candidate_paths(root: &Path) -> BTreeSet<PathBuf> {
    let mut files = Vec::new();
    collect_hardcoded_files(root, &mut files);
    files
        .into_iter()
        .filter(|path| hardcoded_file_has_strings(path))
        .collect()
}

fn tree_display_paths(components: &[String], leaf_display: &str, leaf_source: &Path) -> Vec<String> {
    if leaf_display.starts_with("res://") {
        return (0..components.len())
            .map(|index| format!("res://{}", components[..=index].join("/")))
            .collect();
    }
    tree_source_paths(components, leaf_source)
}

fn tree_source_paths(components: &[String], leaf_source: &Path) -> Vec<String> {
    let mut root = leaf_source.to_path_buf();
    for _ in components {
        root.pop();
    }
    let mut current = root;
    components
        .iter()
        .map(|component| {
            current.push(component);
            display_path(&current)
        })
        .collect()
}

