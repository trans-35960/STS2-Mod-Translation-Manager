use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn open_path_in_system(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Go through the shell instead of launching explorer.exe directly so a
        // file manager the user set as the folder handler (Directory Opus,
        // Files, ...) is the one that opens.
        if path.is_file() {
            if windows_shell::reveal_item(path) {
                return Ok(());
            }
            Command::new("explorer")
                .arg("/select,")
                .arg(path)
                .spawn()
                .map_err(|error| format!("경로 열기 실패: {error}"))?;
            return Ok(());
        }

        if windows_shell::open_with_default_handler(path) {
            return Ok(());
        }
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|error| format!("경로 열기 실패: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|error| format!("경로 열기 실패: {error}"))?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|error| format!("경로 열기 실패: {error}"))?;
        Ok(())
    }
}

/// Opens a `steam://` link in the Steam client. `false` when no Steam client
/// is registered for the scheme, so the caller can fall back to the web.
pub(crate) fn open_steam_url(url: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        windows_shell::has_url_handler("steam")
            && windows_shell::open_with_default_handler(Path::new(url))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = url;
        false
    }
}

/// Opens a web page in the default browser.
pub(crate) fn open_url_in_system(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if windows_shell::open_with_default_handler(Path::new(url)) {
            return Ok(());
        }
        Command::new("explorer")
            .arg(url)
            .spawn()
            .map_err(|error| format!("웹페이지 열기 실패: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("웹페이지 열기 실패: {error}"))?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("웹페이지 열기 실패: {error}"))?;
        Ok(())
    }
}

pub(crate) fn nearest_existing_parent(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|candidate| candidate.exists())
        .map(Path::to_path_buf)
}

pub(crate) fn move_path_or_copy(source: &Path, target: &Path) -> std::io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(17) => {
            if let Err(copy_error) = copy_path_for_move(source, target) {
                let _ = remove_path_if_exists(target);
                return Err(copy_error);
            }
            remove_path_if_exists(source)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn remove_path_if_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn replace_dir_or_file(source: &Path, target: &Path) -> std::io::Result<()> {
    if target.exists() {
        if target.is_dir() {
            fs::remove_dir_all(target)?;
        } else {
            fs::remove_file(target)?;
        }
    }
    if source.is_dir() {
        copy_dir_all(source, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
        Ok(())
    }
}

pub(crate) fn copy_dir_all(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn copy_path_for_move(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        if target.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("target already exists: {}", target.display()),
            ));
        }
        copy_dir_all(source, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows_shell {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows_sys::Win32::UI::Shell::{
        ASSOCF_NONE, ASSOCSTR_COMMAND, AssocQueryStringW, ILFree, SHOpenFolderAndSelectItems,
        SHParseDisplayName, ShellExecuteW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Initializes COM for this thread for the lifetime of the guard; shell
    /// calls need it and the command runs on a blocking worker thread.
    struct ComGuard(bool);

    impl ComGuard {
        fn new() -> Self {
            let result = unsafe {
                CoInitializeEx(
                    null(),
                    (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as _,
                )
            };
            Self(result >= 0)
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    pub(super) fn open_with_default_handler(path: &Path) -> bool {
        let _com = ComGuard::new();
        let operation = wide("open".as_ref());
        let file = wide(path.as_os_str());
        let instance = unsafe {
            ShellExecuteW(
                null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW reports success with a value greater than 32.
        instance as isize > 32
    }

    /// Whether some program is registered to open `scheme:` links; without
    /// this check Windows offers to find an app in the Store instead.
    pub(super) fn has_url_handler(scheme: &str) -> bool {
        let scheme = wide(scheme.as_ref());
        let verb = wide("open".as_ref());
        let mut length = 0u32;
        // With no output buffer it only reports the command's length.
        let result = unsafe {
            AssocQueryStringW(
                ASSOCF_NONE,
                ASSOCSTR_COMMAND,
                scheme.as_ptr(),
                verb.as_ptr(),
                null_mut(),
                &mut length,
            )
        };
        result >= 0 && length > 1
    }

    pub(super) fn reveal_item(path: &Path) -> bool {
        let _com = ComGuard::new();
        let name = wide(path.as_os_str());
        let mut pidl: *mut ITEMIDLIST = null_mut();
        let parsed =
            unsafe { SHParseDisplayName(name.as_ptr(), null_mut(), &mut pidl, 0, null_mut()) };
        if parsed < 0 || pidl.is_null() {
            return false;
        }
        let opened = unsafe { SHOpenFolderAndSelectItems(pidl, 0, null(), 0) };
        unsafe { ILFree(pidl) };
        opened >= 0
    }
}
