//! Unsubscribing from Steam Workshop items.
//!
//! Steam puts deleted Workshop files back, so the only way to remove a
//! Workshop mod is to unsubscribe. There is no URL or public web endpoint for
//! that, so this calls `ISteamUGC::UnsubscribeItem` through the game's own
//! `steam_api64.dll`, acting as the game (app id 2868840) for the running
//! Steam client.
//!
//! The call runs in a short-lived child process (the app's own exe started
//! with [`HELPER_FLAG`]): Steam shows the user as playing the game while the
//! API is up, and a fault inside the DLL must not take the app down with it.

use crate::config::STEAM_APP_ID;
use crate::process::hidden_command;
use std::path::{Path, PathBuf};

/// First argument that turns the app's exe into the unsubscribe helper.
pub const HELPER_FLAG: &str = "--steam-unsubscribe";

/// Where Godot .NET builds of the game keep the Steamworks DLL.
const STEAM_API_DLL: &str = "steam_api64.dll";

/// Finds the game's `steam_api64.dll` (`data_<project>_windows_x86_64/`).
pub fn find_steam_api_dll(game_dir: &Path) -> Option<PathBuf> {
    let direct = game_dir.join(STEAM_API_DLL);
    if direct.is_file() {
        return Some(direct);
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(game_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("data_"))
        })
        .map(|dir| dir.join(STEAM_API_DLL))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Unsubscribes from a Workshop item by running the helper in a child process.
pub fn unsubscribe(helper_exe: &Path, dll_path: &Path, item_id: u64) -> Result<(), String> {
    let output = hidden_command(helper_exe)
        .arg(HELPER_FLAG)
        .arg(dll_path)
        .arg(item_id.to_string())
        .env("SteamAppId", STEAM_APP_ID)
        .env("SteamGameId", STEAM_APP_ID)
        .output()
        .map_err(|error| format!("구독 해제 도우미 실행 실패: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if message.is_empty() {
        format!("구독 해제 실패 (종료 코드 {:?})", output.status.code())
    } else {
        message
    })
}

/// Runs the helper when the process was started with [`HELPER_FLAG`];
/// returns the exit code to end the process with, or `None` otherwise.
pub fn run_helper_from_args(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some(HELPER_FLAG) {
        return None;
    }
    let result = match (
        args.get(2),
        args.get(3).and_then(|id| id.parse::<u64>().ok()),
    ) {
        (Some(dll), Some(item_id)) => ffi::unsubscribe_in_process(Path::new(dll), item_id),
        _ => Err(format!(
            "사용법: {HELPER_FLAG} <steam_api64.dll> <항목 번호>"
        )),
    };
    Some(match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    })
}

#[cfg(windows)]
mod ffi {
    use std::ffi::{c_char, c_void};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    type InitFlat = unsafe extern "C" fn(*mut c_char) -> i32;
    type InitLegacy = unsafe extern "C" fn() -> bool;
    type NoArgs = unsafe extern "C" fn();
    type Accessor = unsafe extern "C" fn() -> *mut c_void;
    type UnsubscribeItem = unsafe extern "C" fn(*mut c_void, u64) -> u64;
    type IsApiCallCompleted = unsafe extern "C" fn(*mut c_void, u64, *mut bool) -> bool;
    type GetApiCallResult =
        unsafe extern "C" fn(*mut c_void, u64, *mut c_void, i32, i32, *mut bool) -> bool;

    /// `RemoteStorageUnsubscribePublishedFileResult_t` (Steam's 8-byte packing on Windows).
    #[repr(C)]
    #[derive(Default)]
    struct UnsubscribeResult {
        result: i32,
        published_file_id: u64,
    }
    const UNSUBSCRIBE_RESULT_CALLBACK: i32 = 1300 + 15;
    const RESULT_OK: i32 = 1;
    const INVALID_API_CALL: u64 = 0;
    const RESULT_TIMEOUT: Duration = Duration::from_secs(20);

    /// Accessor names differ by SDK; newest first.
    const UGC_ACCESSORS: &[&str] = &[
        "SteamAPI_SteamUGC_v021",
        "SteamAPI_SteamUGC_v020",
        "SteamAPI_SteamUGC_v019",
        "SteamAPI_SteamUGC_v018",
        "SteamAPI_SteamUGC_v017",
    ];
    const UTILS_ACCESSORS: &[&str] = &["SteamAPI_SteamUtils_v010", "SteamAPI_SteamUtils_v009"];

    struct Library(*mut c_void);

    impl Library {
        fn load(path: &Path) -> Result<Self, String> {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return Err(format!(
                    "Steam API DLL을 불러오지 못했습니다: {}",
                    path.display()
                ));
            }
            Ok(Self(handle))
        }

        /// # Safety
        /// `T` must be the function pointer type of the exported symbol.
        unsafe fn symbol<T: Copy>(&self, name: &str) -> Option<T> {
            let name = format!("{name}\0");
            let address = unsafe { GetProcAddress(self.0, name.as_ptr()) }?;
            Some(unsafe { std::mem::transmute_copy(&address) })
        }

        unsafe fn first_symbol<T: Copy>(&self, names: &[&str]) -> Option<T> {
            names.iter().find_map(|name| unsafe { self.symbol(name) })
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe { FreeLibrary(self.0) };
        }
    }

    pub(super) fn unsubscribe_in_process(dll_path: &Path, item_id: u64) -> Result<(), String> {
        let library = Library::load(dll_path)?;
        unsafe {
            init(&library)?;
            let result = unsubscribe_with_api(&library, item_id);
            if let Some(shutdown) = library.symbol::<NoArgs>("SteamAPI_Shutdown") {
                shutdown();
            }
            result
        }
    }

    unsafe fn init(library: &Library) -> Result<(), String> {
        const NOT_RUNNING: &str = "Steam에 연결하지 못했습니다. Steam이 실행 중이고 게임을 보유한 계정으로 로그인되어 있는지 확인하세요.";
        if let Some(init_flat) = unsafe { library.symbol::<InitFlat>("SteamAPI_InitFlat") } {
            let mut message = [0 as c_char; 1024];
            if unsafe { init_flat(message.as_mut_ptr()) } == 0 {
                return Ok(());
            }
            let detail = unsafe { std::ffi::CStr::from_ptr(message.as_ptr()) }.to_string_lossy();
            return Err(if detail.trim().is_empty() {
                NOT_RUNNING.to_string()
            } else {
                format!("{NOT_RUNNING} ({})", detail.trim())
            });
        }
        let init = unsafe { library.symbol::<InitLegacy>("SteamAPI_Init") }
            .ok_or("Steam API DLL에 초기화 함수가 없습니다.")?;
        if unsafe { init() } {
            Ok(())
        } else {
            Err(NOT_RUNNING.to_string())
        }
    }

    unsafe fn unsubscribe_with_api(library: &Library, item_id: u64) -> Result<(), String> {
        let missing = |what: &str| format!("Steam API DLL에서 {what}을(를) 찾지 못했습니다.");
        let ugc_accessor = unsafe { library.first_symbol::<Accessor>(UGC_ACCESSORS) }
            .ok_or_else(|| missing("ISteamUGC"))?;
        let utils_accessor = unsafe { library.first_symbol::<Accessor>(UTILS_ACCESSORS) }
            .ok_or_else(|| missing("ISteamUtils"))?;
        let unsubscribe_item =
            unsafe { library.symbol::<UnsubscribeItem>("SteamAPI_ISteamUGC_UnsubscribeItem") }
                .ok_or_else(|| missing("UnsubscribeItem"))?;
        let is_completed = unsafe {
            library.symbol::<IsApiCallCompleted>("SteamAPI_ISteamUtils_IsAPICallCompleted")
        }
        .ok_or_else(|| missing("IsAPICallCompleted"))?;
        let get_result =
            unsafe { library.symbol::<GetApiCallResult>("SteamAPI_ISteamUtils_GetAPICallResult") }
                .ok_or_else(|| missing("GetAPICallResult"))?;
        let run_callbacks = unsafe { library.symbol::<NoArgs>("SteamAPI_RunCallbacks") }
            .ok_or_else(|| missing("RunCallbacks"))?;

        let ugc = unsafe { ugc_accessor() };
        let utils = unsafe { utils_accessor() };
        if ugc.is_null() || utils.is_null() {
            return Err("Steam 창작마당 인터페이스를 열지 못했습니다.".to_string());
        }
        let call = unsafe { unsubscribe_item(ugc, item_id) };
        if call == INVALID_API_CALL {
            return Err("Steam이 구독 해제 요청을 받지 않았습니다.".to_string());
        }

        let started = Instant::now();
        loop {
            unsafe { run_callbacks() };
            let mut failed = false;
            if unsafe { is_completed(utils, call, &mut failed) } {
                let mut result = UnsubscribeResult::default();
                let fetched = unsafe {
                    get_result(
                        utils,
                        call,
                        (&mut result as *mut UnsubscribeResult).cast(),
                        std::mem::size_of::<UnsubscribeResult>() as i32,
                        UNSUBSCRIBE_RESULT_CALLBACK,
                        &mut failed,
                    )
                };
                if !fetched || failed {
                    return Err("Steam에서 구독 해제 결과를 받지 못했습니다.".to_string());
                }
                if result.result != RESULT_OK {
                    return Err(format!(
                        "Steam이 구독 해제를 거부했습니다 (EResult {}).",
                        result.result
                    ));
                }
                return Ok(());
            }
            if started.elapsed() > RESULT_TIMEOUT {
                return Err(
                    "Steam의 구독 해제 응답이 너무 늦습니다. 잠시 후 다시 확인하세요.".to_string(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(not(windows))]
mod ffi {
    use std::path::Path;

    pub(super) fn unsubscribe_in_process(_dll_path: &Path, _item_id: u64) -> Result<(), String> {
        Err("창작마당 구독 해제는 Windows에서만 지원합니다.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_ignores_normal_launches() {
        assert_eq!(run_helper_from_args(&["app.exe".to_string()]), None);
    }

    #[test]
    fn helper_rejects_bad_item_ids() {
        let args = [
            "app.exe".to_string(),
            HELPER_FLAG.to_string(),
            "steam_api64.dll".to_string(),
            "abc".to_string(),
        ];
        assert_eq!(run_helper_from_args(&args), Some(1));
    }

    #[test]
    fn finds_dll_in_godot_data_folder() {
        let root =
            std::env::temp_dir().join(format!("sts2_steam_workshop_test_{}", std::process::id()));
        let data = root.join("data_sts2_windows_x86_64");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join(STEAM_API_DLL), b"").unwrap();
        assert_eq!(find_steam_api_dll(&root), Some(data.join(STEAM_API_DLL)));
        let _ = std::fs::remove_dir_all(&root);
    }
}
