use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorTool {
    pub name: &'static str,
    pub purpose: &'static str,
    pub expected_path: PathBuf,
    pub available: bool,
}

pub fn inspect(vendor_dir: &Path) -> Vec<VendorTool> {
    let tools = [
        (
            "GodotPCKExplorer.Console",
            "extract and repack Godot .pck files",
            vendor_dir
                .join("godot-pck-explorer-dotnet-ui-console-win-linux-mac")
                .join("GodotPCKExplorer.Console.exe"),
        ),
        (
            "7-Zip",
            "extract .7z and .rar archives",
            vendor_dir.join("7zip").join("7z.exe"),
        ),
    ];

    tools
        .into_iter()
        .map(|(name, purpose, expected_path)| VendorTool {
            name,
            purpose,
            available: expected_path.exists(),
            expected_path,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_expected_vendor_paths() {
        let tools = inspect(Path::new("vendor"));

        assert!(
            tools
                .iter()
                .any(|tool| tool.expected_path.ends_with("GodotPCKExplorer.Console.exe"))
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.expected_path.ends_with("7z.exe"))
        );
    }
}
