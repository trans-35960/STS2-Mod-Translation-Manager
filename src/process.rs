use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn hidden_command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut command = Command::new(program);
    configure_hidden_command(&mut command);
    command
}

pub fn powershell_expand_archive(source: &Path, destination: &Path) -> std::io::Result<ExitStatus> {
    hidden_command("powershell")
        .env("STS2_ARCHIVE_SOURCE", source)
        .env("STS2_ARCHIVE_DESTINATION", destination)
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$ErrorActionPreference = 'Stop'; Expand-Archive -LiteralPath $env:STS2_ARCHIVE_SOURCE -DestinationPath $env:STS2_ARCHIVE_DESTINATION -Force",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
}

pub fn powershell_compress_directory_contents(
    source_dir: &Path,
    archive_path: &Path,
) -> std::io::Result<ExitStatus> {
    hidden_command("powershell")
        .env("STS2_ARCHIVE_SOURCE_DIR", source_dir)
        .env("STS2_ARCHIVE_DESTINATION", archive_path)
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$ErrorActionPreference = 'Stop'; $items = Get-ChildItem -LiteralPath $env:STS2_ARCHIVE_SOURCE_DIR -Force; if (-not $items) { throw 'Source directory is empty.' }; Compress-Archive -LiteralPath $items.FullName -DestinationPath $env:STS2_ARCHIVE_DESTINATION -Force",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
}

#[cfg(windows)]
fn configure_hidden_command(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_hidden_command(_command: &mut Command) {}
