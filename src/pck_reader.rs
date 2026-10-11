//! Minimal reader for Godot `.pck` archives.
//!
//! Opening a mod for translation only needs its localization files, but the
//! bundled GodotPCKExplorer can only unpack whole archives, which takes about a
//! minute for large art-heavy mods. Godot stores files uncompressed behind a
//! plain index, so the few files we need can be copied straight out.
//!
//! Anything this reader does not fully understand (encrypted index or files,
//! sparse bundles, unknown versions) is reported as unsupported so callers can
//! fall back to the external tool.

use crate::error::{AppError, AppResult};
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

const MAGIC: &[u8; 4] = b"GDPC";
const PACK_DIR_ENCRYPTED: u32 = 1;
const PACK_SPARSE_BUNDLE: u32 = 1 << 2;
const PACK_FILE_ENCRYPTED: u32 = 1;
const PACK_FILE_REMOVAL: u32 = 1 << 1;
const MAX_FILE_COUNT: u32 = 2_000_000;
const MAX_PATH_BYTES: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PckEntry {
    /// Resource path without the `res://` prefix, using `/` separators.
    pub path: String,
    /// Absolute byte offset of the file data inside the `.pck`.
    pub offset: u64,
    pub size: u64,
}

/// Reads the file table, or `None` when the archive uses features this reader
/// does not handle.
pub fn read_pck_index(pck_path: &Path) -> AppResult<Option<Vec<PckEntry>>> {
    let file = File::open(pck_path).map_err(|source| AppError::io(pck_path, source))?;
    let length = file
        .metadata()
        .map_err(|source| AppError::io(pck_path, source))?
        .len();
    let mut reader = BufReader::new(file);
    read_index(&mut reader, length).map_err(|source| AppError::io(pck_path, source))
}

/// Copies every entry accepted by `wanted` into `destination`, keeping the
/// resource folder layout (`res://a/b.json` becomes `destination/a/b.json`).
/// Returns the number of files written, or `None` when the archive is
/// unsupported and nothing was written.
pub fn extract_pck_files(
    pck_path: &Path,
    destination: &Path,
    wanted: impl Fn(&str) -> bool,
) -> AppResult<Option<usize>> {
    let Some(entries) = read_pck_index(pck_path)? else {
        return Ok(None);
    };
    let mut source = File::open(pck_path).map_err(|source| AppError::io(pck_path, source))?;
    fs::create_dir_all(destination).map_err(|source| AppError::io(destination, source))?;

    let mut written = 0;
    for entry in entries.iter().filter(|entry| wanted(&entry.path)) {
        let Some(relative) = safe_relative_path(&entry.path) else {
            continue;
        };
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
        }
        source
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|error| AppError::io(pck_path, error))?;
        let mut output = File::create(&target).map_err(|error| AppError::io(&target, error))?;
        let copied = io::copy(&mut (&mut source).take(entry.size), &mut output)
            .map_err(|error| AppError::io(&target, error))?;
        output
            .flush()
            .map_err(|error| AppError::io(&target, error))?;
        if copied != entry.size {
            return Err(AppError::InvalidCommand(format!(
                "PCK 항목이 잘렸습니다: {} ({})",
                entry.path,
                pck_path.display()
            )));
        }
        written += 1;
    }
    Ok(Some(written))
}

/// Reads every entry accepted by `wanted` into memory, or `None` when the
/// archive is unsupported.
pub fn read_pck_files(
    pck_path: &Path,
    wanted: impl Fn(&str) -> bool,
) -> AppResult<Option<Vec<(String, Vec<u8>)>>> {
    let Some(entries) = read_pck_index(pck_path)? else {
        return Ok(None);
    };
    let mut source = File::open(pck_path).map_err(|source| AppError::io(pck_path, source))?;
    let mut files = Vec::new();
    for entry in entries.into_iter().filter(|entry| wanted(&entry.path)) {
        source
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|error| AppError::io(pck_path, error))?;
        let mut data = vec![0u8; entry.size as usize];
        source
            .read_exact(&mut data)
            .map_err(|error| AppError::io(pck_path, error))?;
        files.push((entry.path, data));
    }
    Ok(Some(files))
}

fn read_index<R: Read + Seek>(reader: &mut R, length: u64) -> io::Result<Option<Vec<PckEntry>>> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Ok(None);
    }
    let version = read_u32(reader)?;
    if !(1..=3).contains(&version) {
        return Ok(None);
    }
    let _godot_major = read_u32(reader)?;
    let _godot_minor = read_u32(reader)?;
    let _godot_patch = read_u32(reader)?;

    // Mod packs are standalone files starting at byte 0, so a relative file
    // base (PACK_REL_FILEBASE) needs no shift.
    let (flags, file_base) = if version >= 2 {
        (read_u32(reader)?, read_u64(reader)?)
    } else {
        (0, 0)
    };
    if flags & (PACK_DIR_ENCRYPTED | PACK_SPARSE_BUNDLE) != 0 {
        return Ok(None);
    }
    if version >= 3 {
        let dir_offset = read_u64(reader)?;
        if dir_offset >= length {
            return Ok(None);
        }
        reader.seek(SeekFrom::Start(dir_offset))?;
    } else {
        reader.seek(SeekFrom::Current(16 * 4))?;
    }

    let file_count = read_u32(reader)?;
    if file_count > MAX_FILE_COUNT {
        return Ok(None);
    }
    let mut entries = Vec::with_capacity(file_count as usize);
    for _ in 0..file_count {
        let path_length = read_u32(reader)?;
        if path_length > MAX_PATH_BYTES {
            return Ok(None);
        }
        let mut path_bytes = vec![0u8; path_length as usize];
        reader.read_exact(&mut path_bytes)?;
        let offset = read_u64(reader)?;
        let size = read_u64(reader)?;
        let mut _md5 = [0u8; 16];
        reader.read_exact(&mut _md5)?;
        let file_flags = if version >= 2 { read_u32(reader)? } else { 0 };

        if file_flags & PACK_FILE_REMOVAL != 0 {
            continue;
        }
        if file_flags & PACK_FILE_ENCRYPTED != 0 {
            return Ok(None);
        }
        let absolute_offset = file_base.saturating_add(offset);
        if absolute_offset.saturating_add(size) > length {
            return Ok(None);
        }
        let end = path_bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(path_bytes.len());
        let raw_path = String::from_utf8_lossy(&path_bytes[..end]);
        let path = raw_path
            .strip_prefix("res://")
            .unwrap_or(&raw_path)
            .trim_start_matches('/')
            .to_string();
        entries.push(PckEntry {
            path,
            offset: absolute_offset,
            size,
        });
    }
    Ok(Some(entries))
}

/// Rejects absolute paths and `..` so a crafted archive cannot write outside
/// the destination folder.
fn safe_relative_path(path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(candidate.to_path_buf())
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PACK_REL_FILEBASE: u32 = 1 << 1;

    struct TestFile<'a> {
        path: &'a str,
        data: &'a [u8],
        flags: u32,
    }

    /// Builds a pack the way Godot 4 writes one: header, data, then index
    /// (v3) or header, index, data (v2).
    fn build_pck(version: u32, files: &[TestFile<'_>]) -> Vec<u8> {
        let header_len: u64 = if version >= 3 { 40 } else { 32 + 64 };
        let index = || {
            let mut index = Vec::new();
            index.extend((files.len() as u32).to_le_bytes());
            let mut offset = 0u64;
            for file in files {
                let mut path = file.path.as_bytes().to_vec();
                while path.len() % 4 != 0 {
                    path.push(0);
                }
                index.extend((path.len() as u32).to_le_bytes());
                index.extend(&path);
                index.extend(offset.to_le_bytes());
                index.extend((file.data.len() as u64).to_le_bytes());
                index.extend([0u8; 16]);
                index.extend(file.flags.to_le_bytes());
                offset += file.data.len() as u64;
            }
            index
        };
        let data = files
            .iter()
            .flat_map(|file| file.data.iter().copied())
            .collect::<Vec<_>>();

        let mut out = Vec::new();
        out.extend(MAGIC);
        out.extend(version.to_le_bytes());
        out.extend([4u32, 5, 1].iter().flat_map(|value| value.to_le_bytes()));
        out.extend(PACK_REL_FILEBASE.to_le_bytes());
        if version >= 3 {
            let file_base = header_len;
            let dir_offset = file_base + data.len() as u64;
            out.extend(file_base.to_le_bytes());
            out.extend(dir_offset.to_le_bytes());
            out.extend(&data);
            out.extend(index());
        } else {
            let index_bytes = index();
            let file_base = header_len + index_bytes.len() as u64;
            out.extend(file_base.to_le_bytes());
            out.extend([0u8; 64]);
            out.extend(index_bytes);
            out.extend(&data);
        }
        out
    }

    fn sample_files() -> Vec<TestFile<'static>> {
        vec![
            TestFile {
                path: "res://Mod/localization/eng/cards.json",
                data: br#"{"A":"Strike"}"#,
                flags: 0,
            },
            TestFile {
                path: "res://Mod/images/big.png",
                data: &[7u8; 64],
                flags: 0,
            },
            TestFile {
                path: "Mod/localization/kor/cards.json",
                data: r#"{"A":"타격"}"#.as_bytes(),
                flags: 0,
            },
        ]
    }

    #[test]
    fn extracts_only_wanted_files_from_v2_and_v3_packs() {
        for version in [2, 3] {
            let dir = test_dir(&format!("pck_reader_v{version}"));
            let pck = dir.join("Mod.pck");
            fs::write(&pck, build_pck(version, &sample_files())).expect("write pck");
            let out = dir.join("Mod.pck.contents");

            let written = extract_pck_files(&pck, &out, |path| path.ends_with(".json"))
                .expect("extract")
                .expect("supported");

            assert_eq!(written, 2, "version {version}");
            assert_eq!(
                fs::read_to_string(out.join("Mod/localization/eng/cards.json")).expect("eng"),
                r#"{"A":"Strike"}"#
            );
            assert_eq!(
                fs::read_to_string(out.join("Mod/localization/kor/cards.json")).expect("kor"),
                r#"{"A":"타격"}"#
            );
            assert!(!out.join("Mod/images/big.png").exists());
        }
    }

    #[test]
    fn encrypted_or_foreign_archives_are_reported_unsupported() {
        let dir = test_dir("pck_reader_unsupported");
        let mut files = sample_files();
        files[1].flags = PACK_FILE_ENCRYPTED;
        let encrypted = dir.join("Encrypted.pck");
        fs::write(&encrypted, build_pck(3, &files)).expect("write");
        assert_eq!(read_pck_index(&encrypted).expect("read"), None);

        let foreign = dir.join("Foreign.pck");
        fs::write(&foreign, b"PK\x03\x04not a pck").expect("write");
        assert_eq!(read_pck_index(&foreign).expect("read"), None);
    }

    #[test]
    fn rejects_paths_escaping_the_destination() {
        assert_eq!(safe_relative_path("../evil.json"), None);
        assert_eq!(safe_relative_path("a/../../evil.json"), None);
        assert_eq!(safe_relative_path("C:/evil.json"), None);
        assert_eq!(
            safe_relative_path("Mod/loc/eng.json"),
            Some(PathBuf::from("Mod/loc/eng.json"))
        );
    }

    fn test_dir(name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("test-work");
        path.push(format!(
            "{}-{}",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create test dir");
        path
    }
}
