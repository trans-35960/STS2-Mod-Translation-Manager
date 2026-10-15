//! Which game members a mod DLL uses that the installed game does not have.
//!
//! A mod built against an older game keeps calling methods by their exact
//! signature. When the game drops or changes one, the call throws
//! `MissingMethodException` only once it runs, often deep into a run. This
//! reads the ECMA-335 metadata of a mod and of the assemblies it builds on
//! (`sts2.dll`, library mods such as BaseLib) and lists every type, method
//! and field reference that has no matching definition.
//!
//! Only references compiled into the metadata are seen: names a mod hands to
//! reflection or Harmony as strings (`AccessTools.Method("Foo")`) are not, so
//! an empty result is no guarantee that the mod works.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

type Result<T> = std::result::Result<T, String>;

const MODULE: usize = 0x00;
const TYPE_REF: usize = 0x01;
const TYPE_DEF: usize = 0x02;
const FIELD: usize = 0x04;
const METHOD_DEF: usize = 0x06;
const PARAM: usize = 0x08;
const MEMBER_REF: usize = 0x0A;
const EVENT: usize = 0x14;
const PROPERTY: usize = 0x17;
const MODULE_REF: usize = 0x1A;
const TYPE_SPEC: usize = 0x1B;
const ASSEMBLY: usize = 0x20;
const ASSEMBLY_REF: usize = 0x23;
const FILE: usize = 0x26;
const EXPORTED_TYPE: usize = 0x27;
const NESTED_CLASS: usize = 0x29;
const GENERIC_PARAM: usize = 0x2A;
/// Tag slot of a coded index that names no table.
const UNUSED: usize = usize::MAX;

#[derive(Clone, Copy)]
enum Col {
    U16,
    U32,
    Str,
    Guid,
    Blob,
    Table(usize),
    /// Coded index: the tables its tag picks from, and the tag width.
    Coded(&'static [usize], u32),
}

const TYPE_DEF_OR_REF: Col = Col::Coded(&[TYPE_DEF, TYPE_REF, TYPE_SPEC], 2);
const HAS_CONSTANT: Col = Col::Coded(&[FIELD, PARAM, PROPERTY], 2);
const HAS_CUSTOM_ATTRIBUTE: Col = Col::Coded(
    &[
        METHOD_DEF,
        FIELD,
        TYPE_REF,
        TYPE_DEF,
        PARAM,
        0x09,
        MEMBER_REF,
        MODULE,
        0x0E,
        PROPERTY,
        EVENT,
        0x11,
        MODULE_REF,
        TYPE_SPEC,
        ASSEMBLY,
        ASSEMBLY_REF,
        FILE,
        EXPORTED_TYPE,
        0x28,
        GENERIC_PARAM,
        0x2C,
        0x2B,
    ],
    5,
);
const HAS_FIELD_MARSHAL: Col = Col::Coded(&[FIELD, PARAM], 1);
const HAS_DECL_SECURITY: Col = Col::Coded(&[TYPE_DEF, METHOD_DEF, ASSEMBLY], 2);
const MEMBER_REF_PARENT: Col =
    Col::Coded(&[TYPE_DEF, TYPE_REF, MODULE_REF, METHOD_DEF, TYPE_SPEC], 3);
const HAS_SEMANTICS: Col = Col::Coded(&[EVENT, PROPERTY], 1);
const METHOD_DEF_OR_REF: Col = Col::Coded(&[METHOD_DEF, MEMBER_REF], 1);
const MEMBER_FORWARDED: Col = Col::Coded(&[FIELD, METHOD_DEF], 1);
const IMPLEMENTATION: Col = Col::Coded(&[FILE, ASSEMBLY_REF, EXPORTED_TYPE], 2);
const CUSTOM_ATTRIBUTE_TYPE: Col = Col::Coded(&[UNUSED, UNUSED, METHOD_DEF, MEMBER_REF, UNUSED], 3);
const RESOLUTION_SCOPE: Col = Col::Coded(&[MODULE, MODULE_REF, ASSEMBLY_REF, TYPE_REF], 2);
const TYPE_OR_METHOD_DEF: Col = Col::Coded(&[TYPE_DEF, METHOD_DEF], 1);

/// Column layout of every table that can appear in an assembly (II.22).
fn schema(table: usize) -> Option<&'static [Col]> {
    use Col::*;
    Some(match table {
        0x00 => &[U16, Str, Guid, Guid, Guid],
        0x01 => &[RESOLUTION_SCOPE, Str, Str],
        0x02 => &[
            U32,
            Str,
            Str,
            TYPE_DEF_OR_REF,
            Table(FIELD),
            Table(METHOD_DEF),
        ],
        0x03 => &[Table(FIELD)],
        0x04 => &[U16, Str, Blob],
        0x05 => &[Table(METHOD_DEF)],
        0x06 => &[U32, U16, U16, Str, Blob, Table(PARAM)],
        0x07 => &[Table(PARAM)],
        0x08 => &[U16, U16, Str],
        0x09 => &[Table(TYPE_DEF), TYPE_DEF_OR_REF],
        0x0A => &[MEMBER_REF_PARENT, Str, Blob],
        // The type byte is followed by a padding byte.
        0x0B => &[U16, HAS_CONSTANT, Blob],
        0x0C => &[HAS_CUSTOM_ATTRIBUTE, CUSTOM_ATTRIBUTE_TYPE, Blob],
        0x0D => &[HAS_FIELD_MARSHAL, Blob],
        0x0E => &[U16, HAS_DECL_SECURITY, Blob],
        0x0F => &[U16, U32, Table(TYPE_DEF)],
        0x10 => &[U32, Table(FIELD)],
        0x11 => &[Blob],
        0x12 => &[Table(TYPE_DEF), Table(EVENT)],
        0x13 => &[Table(EVENT)],
        0x14 => &[U16, Str, TYPE_DEF_OR_REF],
        0x15 => &[Table(TYPE_DEF), Table(PROPERTY)],
        0x16 => &[Table(PROPERTY)],
        0x17 => &[U16, Str, Blob],
        0x18 => &[U16, Table(METHOD_DEF), HAS_SEMANTICS],
        0x19 => &[Table(TYPE_DEF), METHOD_DEF_OR_REF, METHOD_DEF_OR_REF],
        0x1A => &[Str],
        0x1B => &[Blob],
        0x1C => &[U16, MEMBER_FORWARDED, Str, Table(MODULE_REF)],
        0x1D => &[U32, Table(FIELD)],
        0x1E => &[U32, U32],
        0x1F => &[U32],
        0x20 => &[U32, U16, U16, U16, U16, U32, Blob, Str, Str],
        0x21 => &[U32],
        0x22 => &[U32, U32, U32],
        0x23 => &[U16, U16, U16, U16, U32, Blob, Str, Str, Blob],
        0x24 => &[U32, Table(ASSEMBLY_REF)],
        0x25 => &[U32, U32, U32, Table(ASSEMBLY_REF)],
        0x26 => &[U32, Str, Blob],
        0x27 => &[U32, U32, Str, Str, IMPLEMENTATION],
        0x28 => &[U32, U32, Str, IMPLEMENTATION],
        0x29 => &[Table(TYPE_DEF), Table(TYPE_DEF)],
        0x2A => &[U16, U16, TYPE_OR_METHOD_DEF, Str],
        0x2B => &[METHOD_DEF_OR_REF, Blob],
        0x2C => &[Table(GENERIC_PARAM), TYPE_DEF_OR_REF],
        _ => return None,
    })
}

fn truncated() -> String {
    "metadata is truncated".to_string()
}

fn slice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    offset
        .checked_add(len)
        .and_then(|end| bytes.get(offset..end))
        .ok_or_else(truncated)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let raw = slice(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let raw = slice(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from(read_u32(bytes, offset)?) | (u64::from(read_u32(bytes, offset + 4)?) << 32))
}

/// ECMA-335 compressed unsigned integer (II.23.2).
fn read_compressed(bytes: &[u8], pos: &mut usize) -> Result<u32> {
    let first = *bytes.get(*pos).ok_or_else(truncated)?;
    let (value, len) = if first & 0x80 == 0 {
        (u32::from(first), 1)
    } else if first & 0xC0 == 0x80 {
        let raw = slice(bytes, *pos, 2)?;
        ((u32::from(first & 0x3F) << 8) | u32::from(raw[1]), 2)
    } else if first & 0xE0 == 0xC0 {
        let raw = slice(bytes, *pos, 4)?;
        (
            (u32::from(first & 0x1F) << 24)
                | (u32::from(raw[1]) << 16)
                | (u32::from(raw[2]) << 8)
                | u32::from(raw[3]),
            4,
        )
    } else {
        return Err("bad compressed integer".to_string());
    };
    *pos += len;
    Ok(value)
}

/// A type as a signature spells it. Types from other assemblies compare by
/// full name, so a reference matches a definition however each side reaches
/// the type (`System.Runtime` versus `System.Private.CoreLib`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Primitive(&'static str),
    /// `Namespace.Name`, nested types as `Outer/Inner`.
    Named(String),
    Generic(Box<Ty>, Vec<Ty>),
    SzArray(Box<Ty>),
    Array(Box<Ty>, u32),
    ByRef(Box<Ty>),
    Pointer(Box<Ty>),
    TypeParameter(u32),
    MethodParameter(u32),
    FunctionPointer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSig {
    pub generic_params: u32,
    pub return_type: Ty,
    pub params: Vec<Ty>,
}

/// What a mod uses from another assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub assembly: String,
    /// Full name of the type, or of the type the member is on.
    pub owner: String,
    pub member: Member,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Member {
    Type,
    Method { name: String, signature: MethodSig },
    Field { name: String },
}

/// The types an assembly defines, as references to it are checked against.
#[derive(Debug, Clone, Default)]
pub struct TargetAssembly {
    pub name: String,
    types: HashMap<String, TargetType>,
}

#[derive(Debug, Clone, Default)]
struct TargetType {
    base: Option<TypeLink>,
    /// Name → signatures; `None` for a signature this reader cannot decode,
    /// which then matches any reference by that name.
    methods: HashMap<String, Vec<Option<MethodSig>>>,
    fields: HashSet<String>,
}

#[derive(Debug, Clone)]
struct TypeLink {
    /// `None` for a type of the same assembly.
    assembly: Option<String>,
    name: String,
}

impl TargetAssembly {
    pub fn has_type(&self, full_name: &str) -> bool {
        self.types.contains_key(full_name)
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingKind {
    Type,
    Method,
    Field,
}

/// A reference with no matching definition, ready to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingReference {
    /// Assembly that should define it (`sts2`, `BaseLib`).
    pub assembly: String,
    pub kind: MissingKind,
    /// `CreatureCmd.Damage(PlayerChoiceContext, Creature, decimal)`.
    pub display: String,
    /// Same-named methods the assembly does have, shown the same way.
    #[serde(default)]
    pub available: Vec<String>,
}

impl MissingReference {
    /// `CreatureCmd.Damage(…) — 게임에서 시그니처가 바뀜`.
    pub fn describe(&self) -> String {
        let target = if self.assembly.eq_ignore_ascii_case(GAME_ASSEMBLY) {
            "게임".to_string()
        } else {
            self.assembly.clone()
        };
        match self.kind {
            MissingKind::Type => format!("{} 타입 — {target}에 없음", self.display),
            _ if !self.available.is_empty() => {
                format!("{} — {target}에서 시그니처가 바뀜", self.display)
            }
            _ => format!("{} — {target}에서 없어짐", self.display),
        }
    }
}

/// Assembly name of the game (`data_sts2_windows_x86_64/sts2.dll`).
pub const GAME_ASSEMBLY: &str = "sts2";

/// Parsed metadata of one assembly.
pub struct Metadata {
    bytes: Vec<u8>,
    strings: usize,
    strings_len: usize,
    blobs: usize,
    blobs_len: usize,
    rows: [u32; 64],
    table_offsets: [usize; 64],
    row_sizes: [usize; 64],
    columns: Vec<Vec<(usize, usize)>>,
    /// Nested TypeDef row → enclosing TypeDef row.
    enclosing: HashMap<u32, u32>,
    assembly_name: String,
}

const MAX_DEPTH: u32 = 32;
/// Same-named overloads listed next to a missing method.
const MAX_AVAILABLE: usize = 3;

impl Metadata {
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(bytes)
    }

    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        let root = metadata_root(&bytes)?;
        if read_u32(&bytes, root)? != 0x424A_5342 {
            return Err("no CLI metadata signature".to_string());
        }
        let version_len = read_u32(&bytes, root + 12)? as usize;
        let mut pos = root + 16 + version_len + 2;
        let stream_count = read_u16(&bytes, pos)?;
        pos += 2;
        let mut tables_stream = None;
        let (mut strings, mut strings_len) = (0, 0);
        let (mut blobs, mut blobs_len) = (0, 0);
        for _ in 0..stream_count {
            let offset = root + read_u32(&bytes, pos)? as usize;
            let size = read_u32(&bytes, pos + 4)? as usize;
            let name_start = pos + 8;
            let name_len = bytes
                .get(name_start..)
                .and_then(|rest| rest.iter().take(32).position(|&byte| byte == 0))
                .ok_or_else(truncated)?;
            let name = &bytes[name_start..name_start + name_len];
            pos = name_start + (name_len + 4) / 4 * 4;
            slice(&bytes, offset, size)?;
            match name {
                b"#~" | b"#-" => tables_stream = Some(offset),
                b"#Strings" => (strings, strings_len) = (offset, size),
                b"#Blob" => (blobs, blobs_len) = (offset, size),
                _ => {}
            }
        }
        let tables_stream = tables_stream.ok_or("no metadata tables stream")?;
        let heap_sizes = *bytes.get(tables_stream + 6).ok_or_else(truncated)?;
        let valid = read_u64(&bytes, tables_stream + 8)?;
        let mut rows = [0u32; 64];
        let mut pos = tables_stream + 24;
        for (table, count) in rows.iter_mut().enumerate() {
            if valid & (1 << table) != 0 {
                if schema(table).is_none() {
                    return Err(format!("unknown metadata table 0x{table:02X}"));
                }
                *count = read_u32(&bytes, pos)?;
                pos += 4;
            }
        }
        if heap_sizes & 0x40 != 0 {
            pos += 4;
        }

        let width = |col: Col| -> usize {
            match col {
                Col::U16 => 2,
                Col::U32 => 4,
                Col::Str => wide(heap_sizes & 0x01 != 0),
                Col::Guid => wide(heap_sizes & 0x02 != 0),
                Col::Blob => wide(heap_sizes & 0x04 != 0),
                Col::Table(table) => wide(rows[table] > 0xFFFF),
                Col::Coded(tables, bits) => {
                    let largest = tables
                        .iter()
                        .map(|&table| if table == UNUSED { 0 } else { rows[table] })
                        .max()
                        .unwrap_or(0);
                    wide(largest >= 1 << (16 - bits))
                }
            }
        };
        let mut table_offsets = [0usize; 64];
        let mut row_sizes = [0usize; 64];
        let mut columns = vec![Vec::new(); 64];
        for table in 0..64 {
            let Some(cols) = schema(table) else {
                continue;
            };
            let mut offset = 0;
            for &col in cols {
                let size = width(col);
                columns[table].push((offset, size));
                offset += size;
            }
            row_sizes[table] = offset;
            table_offsets[table] = pos;
            pos += offset * rows[table] as usize;
        }
        slice(&bytes, 0, pos)?;

        let mut metadata = Self {
            bytes,
            strings,
            strings_len,
            blobs,
            blobs_len,
            rows,
            table_offsets,
            row_sizes,
            columns,
            enclosing: HashMap::new(),
            assembly_name: String::new(),
        };
        for row in 1..=metadata.rows[NESTED_CLASS] {
            let nested = metadata.cell(NESTED_CLASS, row, 0)?;
            let enclosing = metadata.cell(NESTED_CLASS, row, 1)?;
            metadata.enclosing.insert(nested, enclosing);
        }
        metadata.assembly_name = if metadata.rows[ASSEMBLY] > 0 {
            metadata.string(metadata.cell(ASSEMBLY, 1, 7)?)?
        } else if metadata.rows[MODULE] > 0 {
            metadata.string(metadata.cell(MODULE, 1, 1)?)?
        } else {
            String::new()
        };
        Ok(metadata)
    }

    pub fn assembly_name(&self) -> &str {
        &self.assembly_name
    }

    /// Names of the other assemblies this one references.
    pub fn assembly_references(&self) -> Result<Vec<String>> {
        (1..=self.rows[ASSEMBLY_REF])
            .map(|row| self.string(self.cell(ASSEMBLY_REF, row, 6)?))
            .collect()
    }

    fn cell(&self, table: usize, row: u32, col: usize) -> Result<u32> {
        if row == 0 || row > self.rows[table] {
            return Err(format!("row {row} out of table 0x{table:02X}"));
        }
        let (offset, size) = self.columns[table][col];
        let at = self.table_offsets[table] + (row as usize - 1) * self.row_sizes[table] + offset;
        if size == 2 {
            Ok(u32::from(read_u16(&self.bytes, at)?))
        } else {
            read_u32(&self.bytes, at)
        }
    }

    fn string(&self, index: u32) -> Result<String> {
        let index = index as usize;
        if index >= self.strings_len.max(1) {
            return Err("string index out of heap".to_string());
        }
        let heap = &self.bytes[self.strings..self.strings + self.strings_len];
        let rest = &heap[index..];
        let end = rest
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(rest.len());
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    fn blob(&self, index: u32) -> Result<&[u8]> {
        let heap = &self.bytes[self.blobs..self.blobs + self.blobs_len];
        let mut pos = index as usize;
        let len = read_compressed(heap, &mut pos)? as usize;
        slice(heap, pos, len)
    }

    fn type_def_name(&self, row: u32, depth: u32) -> Result<String> {
        let name = self.string(self.cell(TYPE_DEF, row, 1)?)?;
        if let Some(&outer) = self.enclosing.get(&row)
            && depth < MAX_DEPTH
        {
            return Ok(format!("{}/{name}", self.type_def_name(outer, depth + 1)?));
        }
        Ok(qualify(&self.string(self.cell(TYPE_DEF, row, 2)?)?, name))
    }

    fn type_ref_name(&self, row: u32, depth: u32) -> Result<String> {
        let name = self.string(self.cell(TYPE_REF, row, 1)?)?;
        let scope = self.cell(TYPE_REF, row, 0)?;
        if scope & 3 == 3 && scope >> 2 != 0 && depth < MAX_DEPTH {
            return Ok(format!(
                "{}/{name}",
                self.type_ref_name(scope >> 2, depth + 1)?
            ));
        }
        Ok(qualify(&self.string(self.cell(TYPE_REF, row, 2)?)?, name))
    }

    /// Assembly a TypeRef resolves to; `None` for module references and
    /// exported types.
    fn type_ref_assembly(&self, mut row: u32) -> Result<Option<String>> {
        for _ in 0..MAX_DEPTH {
            let scope = self.cell(TYPE_REF, row, 0)?;
            let scope_row = scope >> 2;
            match scope & 3 {
                0 => return Ok(Some(self.assembly_name.clone())),
                2 if scope_row != 0 => {
                    return Ok(Some(self.string(self.cell(
                        ASSEMBLY_REF,
                        scope_row,
                        6,
                    )?)?));
                }
                3 if scope_row != 0 => row = scope_row,
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    /// Assembly (`None` = this one) and full name of a `TypeDefOrRef` coded
    /// index; a generic instantiation stands for its generic type.
    fn type_link(&self, coded: u32, depth: u32) -> Result<Option<TypeLink>> {
        let row = coded >> 2;
        if row == 0 {
            return Ok(None);
        }
        match coded & 3 {
            0 => Ok(Some(TypeLink {
                assembly: None,
                name: self.type_def_name(row, 0)?,
            })),
            1 => {
                let Some(assembly) = self.type_ref_assembly(row)? else {
                    return Ok(None);
                };
                let assembly =
                    (!assembly.eq_ignore_ascii_case(&self.assembly_name)).then_some(assembly);
                Ok(Some(TypeLink {
                    assembly,
                    name: self.type_ref_name(row, 0)?,
                }))
            }
            2 if depth < MAX_DEPTH => {
                let blob = self.blob(self.cell(TYPE_SPEC, row, 0)?)?;
                if blob.first() != Some(&0x15) || blob.len() < 3 {
                    return Ok(None);
                }
                let mut pos = 2;
                let inner = read_compressed(blob, &mut pos)?;
                self.type_link(inner, depth + 1)
            }
            _ => Ok(None),
        }
    }

    fn coded_type(&self, coded: u32, depth: u32) -> Result<Ty> {
        let row = coded >> 2;
        match coded & 3 {
            0 => Ok(Ty::Named(self.type_def_name(row, 0)?)),
            1 => Ok(Ty::Named(self.type_ref_name(row, 0)?)),
            2 => {
                let blob = self.blob(self.cell(TYPE_SPEC, row, 0)?)?;
                self.read_type(blob, &mut 0, depth + 1)
            }
            _ => Err("bad TypeDefOrRef index".to_string()),
        }
    }

    fn read_type(&self, sig: &[u8], pos: &mut usize, depth: u32) -> Result<Ty> {
        if depth > MAX_DEPTH {
            return Err("signature nests too deep".to_string());
        }
        let element = *sig.get(*pos).ok_or_else(truncated)?;
        *pos += 1;
        let primitive = match element {
            0x01 => "void",
            0x02 => "bool",
            0x03 => "char",
            0x04 => "sbyte",
            0x05 => "byte",
            0x06 => "short",
            0x07 => "ushort",
            0x08 => "int",
            0x09 => "uint",
            0x0A => "long",
            0x0B => "ulong",
            0x0C => "float",
            0x0D => "double",
            0x0E => "string",
            0x16 => "TypedReference",
            0x18 => "nint",
            0x19 => "nuint",
            0x1C => "object",
            _ => "",
        };
        if !primitive.is_empty() {
            return Ok(Ty::Primitive(primitive));
        }
        let next = depth + 1;
        Ok(match element {
            0x0F => Ty::Pointer(Box::new(self.read_type(sig, pos, next)?)),
            0x10 => Ty::ByRef(Box::new(self.read_type(sig, pos, next)?)),
            0x11 | 0x12 => self.coded_type(read_compressed(sig, pos)?, next)?,
            0x13 => Ty::TypeParameter(read_compressed(sig, pos)?),
            0x1E => Ty::MethodParameter(read_compressed(sig, pos)?),
            0x14 => {
                let element = self.read_type(sig, pos, next)?;
                let rank = read_compressed(sig, pos)?;
                for _ in 0..2 {
                    let count = read_compressed(sig, pos)?;
                    for _ in 0..count {
                        read_compressed(sig, pos)?;
                    }
                }
                Ty::Array(Box::new(element), rank)
            }
            0x15 => {
                *pos += 1;
                let generic = self.coded_type(read_compressed(sig, pos)?, next)?;
                let count = read_compressed(sig, pos)?;
                let mut args = Vec::new();
                for _ in 0..count {
                    args.push(self.read_type(sig, pos, next)?);
                }
                Ty::Generic(Box::new(generic), args)
            }
            0x1B => {
                self.read_method_sig(sig, pos, next)?;
                Ty::FunctionPointer
            }
            0x1D => Ty::SzArray(Box::new(self.read_type(sig, pos, next)?)),
            // Custom modifiers (`modreq`/`modopt`) do not change what matches.
            0x1F | 0x20 => {
                read_compressed(sig, pos)?;
                self.read_type(sig, pos, next)?
            }
            0x45 => self.read_type(sig, pos, next)?,
            other => return Err(format!("unknown element type 0x{other:02X}")),
        })
    }

    fn read_method_sig(&self, sig: &[u8], pos: &mut usize, depth: u32) -> Result<MethodSig> {
        let convention = *sig.get(*pos).ok_or_else(truncated)?;
        *pos += 1;
        let generic_params = if convention & 0x10 != 0 {
            read_compressed(sig, pos)?
        } else {
            0
        };
        let count = read_compressed(sig, pos)?;
        let return_type = self.read_type(sig, pos, depth)?;
        let mut params = Vec::new();
        for _ in 0..count {
            // Vararg call sites mark where the extra arguments start.
            if sig.get(*pos) == Some(&0x41) {
                *pos += 1;
            }
            params.push(self.read_type(sig, pos, depth)?);
        }
        Ok(MethodSig {
            generic_params,
            return_type,
            params,
        })
    }

    fn method_sig(&self, blob: u32) -> Result<MethodSig> {
        self.read_method_sig(self.blob(blob)?, &mut 0, 0)
    }

    /// Every type, method and field this assembly uses from other assemblies.
    pub fn references(&self) -> Result<Vec<Reference>> {
        let mut references = Vec::new();
        for row in 1..=self.rows[TYPE_REF] {
            let Some(assembly) = self.type_ref_assembly(row)? else {
                continue;
            };
            if assembly.eq_ignore_ascii_case(&self.assembly_name) {
                continue;
            }
            references.push(Reference {
                assembly,
                owner: self.type_ref_name(row, 0)?,
                member: Member::Type,
            });
        }
        for row in 1..=self.rows[MEMBER_REF] {
            let parent = self.cell(MEMBER_REF, row, 0)?;
            let parent_row = parent >> 3;
            let owner = match parent & 7 {
                1 => self.type_link((parent_row << 2) | 1, 0)?,
                4 => self.type_link((parent_row << 2) | 2, 0)?,
                _ => None,
            };
            let Some(TypeLink {
                assembly: Some(assembly),
                name: owner,
            }) = owner
            else {
                continue;
            };
            let name = self.string(self.cell(MEMBER_REF, row, 1)?)?;
            let blob = self.cell(MEMBER_REF, row, 2)?;
            let member = if self
                .blob(blob)?
                .first()
                .is_some_and(|&first| first & 0x0F == 0x06)
            {
                Member::Field { name }
            } else {
                // A signature this reader cannot follow is left unchecked
                // rather than reported.
                let Ok(signature) = self.method_sig(blob) else {
                    continue;
                };
                Member::Method { name, signature }
            };
            references.push(Reference {
                assembly,
                owner,
                member,
            });
        }
        Ok(references)
    }

    /// The types this assembly defines, with their methods and fields.
    pub fn target(&self) -> Result<TargetAssembly> {
        let mut types = HashMap::new();
        let type_rows = self.rows[TYPE_DEF];
        for row in 1..=type_rows {
            let range = |col: usize, table: usize| -> Result<std::ops::Range<u32>> {
                let end_limit = self.rows[table] + 1;
                let start = self.cell(TYPE_DEF, row, col)?.min(end_limit);
                let end = if row < type_rows {
                    self.cell(TYPE_DEF, row + 1, col)?.min(end_limit)
                } else {
                    end_limit
                };
                Ok(start.max(1)..end.max(start.max(1)))
            };
            let mut target = TargetType {
                base: self.type_link(self.cell(TYPE_DEF, row, 3)?, 0)?,
                ..TargetType::default()
            };
            for field in range(4, FIELD)? {
                target
                    .fields
                    .insert(self.string(self.cell(FIELD, field, 1)?)?);
            }
            for method in range(5, METHOD_DEF)? {
                let name = self.string(self.cell(METHOD_DEF, method, 3)?)?;
                let signature = self.method_sig(self.cell(METHOD_DEF, method, 4)?).ok();
                target.methods.entry(name).or_default().push(signature);
            }
            types.insert(self.type_def_name(row, 0)?, target);
        }
        Ok(TargetAssembly {
            name: self.assembly_name.clone(),
            types,
        })
    }
}

fn wide(is_wide: bool) -> usize {
    if is_wide { 4 } else { 2 }
}

fn qualify(namespace: &str, name: String) -> String {
    if namespace.is_empty() {
        name
    } else {
        format!("{namespace}.{name}")
    }
}

/// File offset of the CLI metadata root of a PE image.
fn metadata_root(bytes: &[u8]) -> Result<usize> {
    if bytes.get(..2) != Some(b"MZ") {
        return Err("not a PE file".to_string());
    }
    let pe = read_u32(bytes, 0x3C)? as usize;
    if slice(bytes, pe, 4)? != b"PE\0\0" {
        return Err("not a PE file".to_string());
    }
    let coff = pe + 4;
    let section_count = read_u16(bytes, coff + 2)? as usize;
    let optional_size = read_u16(bytes, coff + 16)? as usize;
    let optional = coff + 20;
    let directories = match read_u16(bytes, optional)? {
        0x10B => optional + 96,
        0x20B => optional + 112,
        _ => return Err("unknown PE optional header".to_string()),
    };
    let sections = optional + optional_size;
    let to_offset = |rva: u32| -> Result<usize> {
        for index in 0..section_count {
            let header = sections + index * 40;
            let virtual_size = read_u32(bytes, header + 8)?;
            let virtual_address = read_u32(bytes, header + 12)?;
            let raw_size = read_u32(bytes, header + 16)?;
            let raw_pointer = read_u32(bytes, header + 20)?;
            if rva >= virtual_address && rva - virtual_address < virtual_size.max(raw_size) {
                return Ok((rva - virtual_address + raw_pointer) as usize);
            }
        }
        Err("RVA outside every section".to_string())
    };
    let cli_rva = read_u32(bytes, directories + 14 * 8)?;
    if cli_rva == 0 {
        return Err("not a .NET assembly".to_string());
    }
    let cli = to_offset(cli_rva)?;
    to_offset(read_u32(bytes, cli + 8)?)
}

/// References from `references` that no target defines. Only references to
/// a target assembly are checked; a member is looked up on its type and the
/// type's base types, across targets where a base lives in another one.
pub fn find_missing(
    references: &[Reference],
    targets: &[&TargetAssembly],
) -> Vec<MissingReference> {
    let mut missing = Vec::<MissingReference>::new();
    for reference in references {
        let Some(target) = targets
            .iter()
            .position(|target| target.name.eq_ignore_ascii_case(&reference.assembly))
        else {
            continue;
        };
        let owner_known = targets[target].types.contains_key(&reference.owner);
        let found = match &reference.member {
            Member::Type => {
                if owner_known {
                    continue;
                }
                MissingReference {
                    assembly: targets[target].name.clone(),
                    kind: MissingKind::Type,
                    display: short_type_name(&reference.owner),
                    available: Vec::new(),
                }
            }
            // A member of a missing type: the type itself is reported.
            _ if !owner_known => continue,
            Member::Field { name } => {
                if type_chain(targets, target, &reference.owner)
                    .iter()
                    .any(|ty| ty.fields.contains(name))
                {
                    continue;
                }
                MissingReference {
                    assembly: targets[target].name.clone(),
                    kind: MissingKind::Field,
                    display: format!("{}.{name}", short_type_name(&reference.owner)),
                    available: Vec::new(),
                }
            }
            Member::Method { name, signature } => {
                let overloads = type_chain(targets, target, &reference.owner)
                    .iter()
                    .filter_map(|ty| ty.methods.get(name))
                    .flatten()
                    .collect::<Vec<_>>();
                if overloads
                    .iter()
                    .any(|overload| overload.as_ref().is_none_or(|known| known == signature))
                {
                    continue;
                }
                let mut known = overloads.into_iter().flatten().collect::<Vec<_>>();
                // The overloads closest to the call: most leading parameters
                // in common, then fewest extra ones.
                known.sort_by_key(|other| {
                    let shared = other
                        .params
                        .iter()
                        .zip(&signature.params)
                        .take_while(|(left, right)| left == right)
                        .count();
                    (
                        std::cmp::Reverse(shared),
                        other.params.len().abs_diff(signature.params.len()),
                    )
                });
                known.truncate(MAX_AVAILABLE);
                let with_return = known.iter().any(|other| {
                    other.params == signature.params
                        && other.generic_params == signature.generic_params
                });
                MissingReference {
                    assembly: targets[target].name.clone(),
                    kind: MissingKind::Method,
                    display: method_display(&reference.owner, name, signature, with_return),
                    available: known
                        .iter()
                        .map(|other| method_display(&reference.owner, name, other, with_return))
                        .collect(),
                }
            }
        };
        if !missing.contains(&found) {
            missing.push(found);
        }
    }
    missing.sort_by(|left, right| {
        (&left.assembly, &left.display).cmp(&(&right.assembly, &right.display))
    });
    missing
}

fn type_chain<'t>(
    targets: &[&'t TargetAssembly],
    target: usize,
    owner: &str,
) -> Vec<&'t TargetType> {
    let mut chain = Vec::new();
    let mut current = Some((target, owner.to_string()));
    while let Some((index, name)) = current.take() {
        let Some(ty) = targets[index].types.get(&name) else {
            break;
        };
        chain.push(ty);
        if chain.len() >= MAX_DEPTH as usize {
            break;
        }
        current = ty.base.as_ref().and_then(|link| {
            let index = match &link.assembly {
                None => Some(index),
                Some(assembly) => targets
                    .iter()
                    .position(|target| target.name.eq_ignore_ascii_case(assembly)),
            }?;
            Some((index, link.name.clone()))
        });
    }
    chain
}

/// `MegaCrit.Sts2.Core.Commands.CreatureCmd` → `CreatureCmd`;
/// `Outer/Inner` → `Outer.Inner`; generic arity marks dropped.
pub fn short_type_name(full_name: &str) -> String {
    let mut parts = full_name.split('/');
    let outer = parts.next().unwrap_or_default();
    let outer = outer.rsplit('.').next().unwrap_or(outer);
    std::iter::once(outer)
        .chain(parts)
        .map(|part| part.split('`').next().unwrap_or(part))
        .collect::<Vec<_>>()
        .join(".")
}

fn type_display(ty: &Ty) -> String {
    match ty {
        Ty::Primitive(name) => (*name).to_string(),
        Ty::Named(name) if name == "System.Decimal" => "decimal".to_string(),
        Ty::Named(name) => short_type_name(name),
        Ty::Generic(generic, args) => {
            if matches!(generic.as_ref(), Ty::Named(name) if name == "System.Nullable`1")
                && args.len() == 1
            {
                return format!("{}?", type_display(&args[0]));
            }
            let args = args.iter().map(type_display).collect::<Vec<_>>().join(", ");
            format!("{}<{args}>", type_display(generic))
        }
        Ty::SzArray(element) => format!("{}[]", type_display(element)),
        Ty::Array(element, rank) => {
            format!(
                "{}[{}]",
                type_display(element),
                ",".repeat(rank.saturating_sub(1) as usize)
            )
        }
        Ty::ByRef(element) => format!("ref {}", type_display(element)),
        Ty::Pointer(element) => format!("{}*", type_display(element)),
        Ty::TypeParameter(index) | Ty::MethodParameter(index) => format!("T{}", index + 1),
        Ty::FunctionPointer => "delegate*".to_string(),
    }
}

fn method_display(owner: &str, name: &str, signature: &MethodSig, with_return: bool) -> String {
    let owner = short_type_name(owner);
    let params = signature
        .params
        .iter()
        .map(type_display)
        .collect::<Vec<_>>()
        .join(", ");
    let generics = match signature.generic_params {
        0 => String::new(),
        count => format!(
            "<{}>",
            (1..=count)
                .map(|index| format!("T{index}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let mut display = match name {
        ".ctor" => format!("new {owner}({params})"),
        ".cctor" => format!("{owner} 정적 생성자"),
        _ => format!("{owner}.{name}{generics}({params})"),
    };
    if with_return {
        display.push_str(&format!(": {}", type_display(&signature.return_type)));
    }
    display
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    /// Builds a minimal PE32 image holding the given metadata tables. Every
    /// heap and table here stays small, so each index is two bytes wide.
    struct Builder {
        strings: Vec<u8>,
        blobs: Vec<u8>,
        tables: BTreeMap<usize, (u32, Vec<u8>)>,
    }

    impl Builder {
        fn new() -> Self {
            Self {
                strings: vec![0],
                blobs: vec![0],
                tables: BTreeMap::new(),
            }
        }

        fn string(&mut self, text: &str) -> u32 {
            let index = self.strings.len() as u32;
            self.strings.extend_from_slice(text.as_bytes());
            self.strings.push(0);
            index
        }

        fn blob(&mut self, bytes: &[u8]) -> u32 {
            let index = self.blobs.len() as u32;
            self.blobs.push(bytes.len() as u8);
            self.blobs.extend_from_slice(bytes);
            index
        }

        /// Appends a row; each cell is (value, width in bytes).
        fn row(&mut self, table: usize, cells: &[(u32, usize)]) -> u32 {
            let (count, bytes) = self.tables.entry(table).or_default();
            for &(value, width) in cells {
                bytes.extend_from_slice(&value.to_le_bytes()[..width]);
            }
            *count += 1;
            *count
        }

        fn assembly(&mut self, name: &str) {
            let name = self.string(name);
            self.row(MODULE, &[(0, 2), (name, 2), (0, 2), (0, 2), (0, 2)]);
            self.row(
                ASSEMBLY,
                &[
                    (0, 4),
                    (1, 2),
                    (0, 2),
                    (0, 2),
                    (0, 2),
                    (0, 4),
                    (0, 2),
                    (name, 2),
                    (0, 2),
                ],
            );
        }

        fn assembly_ref(&mut self, name: &str) -> u32 {
            let name = self.string(name);
            self.row(
                ASSEMBLY_REF,
                &[
                    (1, 2),
                    (0, 2),
                    (0, 2),
                    (0, 2),
                    (0, 4),
                    (0, 2),
                    (name, 2),
                    (0, 2),
                    (0, 2),
                ],
            )
        }

        fn type_ref(&mut self, assembly_ref: u32, namespace: &str, name: &str) -> u32 {
            let (name, namespace) = (self.string(name), self.string(namespace));
            self.row(
                TYPE_REF,
                &[((assembly_ref << 2) | 2, 2), (name, 2), (namespace, 2)],
            )
        }

        fn member_ref(&mut self, type_ref: u32, name: &str, signature: &[u8]) {
            let (name, signature) = (self.string(name), self.blob(signature));
            self.row(
                MEMBER_REF,
                &[((type_ref << 3) | 1, 2), (name, 2), (signature, 2)],
            );
        }

        fn type_def(
            &mut self,
            namespace: &str,
            name: &str,
            extends: u32,
            methods_from: u32,
        ) -> u32 {
            let (name, namespace) = (self.string(name), self.string(namespace));
            self.row(
                TYPE_DEF,
                &[
                    (0, 4),
                    (name, 2),
                    (namespace, 2),
                    (extends, 2),
                    (1, 2),
                    (methods_from, 2),
                ],
            )
        }

        fn method_def(&mut self, name: &str, signature: &[u8]) -> u32 {
            let (name, signature) = (self.string(name), self.blob(signature));
            self.row(
                METHOD_DEF,
                &[(0, 4), (0, 2), (0, 2), (name, 2), (signature, 2), (1, 2)],
            )
        }

        fn image(&self) -> Vec<u8> {
            let mut tables = vec![0, 0, 0, 0, 2, 0, 0, 1];
            let valid = self
                .tables
                .keys()
                .fold(0u64, |valid, &table| valid | (1 << table));
            tables.extend_from_slice(&valid.to_le_bytes());
            tables.extend_from_slice(&0u64.to_le_bytes());
            for (count, _) in self.tables.values() {
                tables.extend_from_slice(&count.to_le_bytes());
            }
            for (_, bytes) in self.tables.values() {
                tables.extend_from_slice(bytes);
            }
            let streams = [
                (&b"#~\0\0"[..], pad4(tables)),
                (&b"#Strings\0\0\0\0"[..], pad4(self.strings.clone())),
                (&b"#Blob\0\0\0"[..], pad4(self.blobs.clone())),
            ];
            let mut metadata = Vec::new();
            metadata.extend_from_slice(&0x424A_5342u32.to_le_bytes());
            metadata.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0, 0, 12, 0, 0, 0]);
            metadata.extend_from_slice(b"v4.0.30319\0\0");
            metadata.extend_from_slice(&[0, 0, streams.len() as u8, 0]);
            let header_len: usize = streams.iter().map(|(name, _)| 8 + name.len()).sum();
            let mut offset = metadata.len() + header_len;
            for (name, data) in &streams {
                metadata.extend_from_slice(&(offset as u32).to_le_bytes());
                metadata.extend_from_slice(&(data.len() as u32).to_le_bytes());
                metadata.extend_from_slice(name);
                offset += data.len();
            }
            for (_, data) in &streams {
                metadata.extend_from_slice(data);
            }

            const SECTION_RVA: u32 = 0x2000;
            const SECTION_FILE: usize = 0x200;
            const CLI_LEN: u32 = 72;
            let mut image = vec![0u8; SECTION_FILE];
            image[..2].copy_from_slice(b"MZ");
            image[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
            image[0x80..0x84].copy_from_slice(b"PE\0\0");
            let coff = 0x84;
            image[coff..coff + 2].copy_from_slice(&0x14Cu16.to_le_bytes());
            image[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
            image[coff + 16..coff + 18].copy_from_slice(&224u16.to_le_bytes());
            let optional = coff + 20;
            image[optional..optional + 2].copy_from_slice(&0x10Bu16.to_le_bytes());
            let cli_directory = optional + 96 + 14 * 8;
            image[cli_directory..cli_directory + 4].copy_from_slice(&SECTION_RVA.to_le_bytes());
            image[cli_directory + 4..cli_directory + 8].copy_from_slice(&CLI_LEN.to_le_bytes());
            let section = optional + 224;
            let section_len = (CLI_LEN as usize + metadata.len()) as u32;
            image[section..section + 5].copy_from_slice(b".text");
            image[section + 8..section + 12].copy_from_slice(&section_len.to_le_bytes());
            image[section + 12..section + 16].copy_from_slice(&SECTION_RVA.to_le_bytes());
            image[section + 16..section + 20].copy_from_slice(&section_len.to_le_bytes());
            image[section + 20..section + 24].copy_from_slice(&(SECTION_FILE as u32).to_le_bytes());

            let mut cli = vec![0u8; CLI_LEN as usize];
            cli[..4].copy_from_slice(&CLI_LEN.to_le_bytes());
            cli[8..12].copy_from_slice(&(SECTION_RVA + CLI_LEN).to_le_bytes());
            cli[12..16].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
            image.extend_from_slice(&cli);
            image.extend_from_slice(&metadata);
            image
        }
    }

    fn pad4(mut bytes: Vec<u8>) -> Vec<u8> {
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }

    /// static void (int, int)
    const DAMAGE_SIG: &[u8] = &[0x00, 2, 0x01, 0x08, 0x08];

    fn game() -> TargetAssembly {
        let mut builder = Builder::new();
        builder.assembly("sts2");
        builder.type_def("", "<Module>", 0, 1);
        let cmd = builder.type_def("Game", "Cmd", 0, 1);
        // Derived : Cmd, no methods of its own.
        builder.type_def("Game", "Derived", cmd << 2, 2);
        builder.method_def("Damage", DAMAGE_SIG);
        let metadata = Metadata::parse(builder.image()).expect("game metadata");
        assert_eq!(metadata.assembly_name(), "sts2");
        metadata.target().expect("game types")
    }

    fn mod_references() -> Vec<Reference> {
        let mut builder = Builder::new();
        builder.assembly("TestMod");
        let sts2 = builder.assembly_ref("sts2");
        let runtime = builder.assembly_ref("System.Runtime");
        let cmd = builder.type_ref(sts2, "Game", "Cmd");
        let derived = builder.type_ref(sts2, "Game", "Derived");
        builder.type_ref(sts2, "Game", "Gone");
        let decimal = builder.type_ref(runtime, "System", "Decimal");
        builder.member_ref(cmd, "Damage", DAMAGE_SIG);
        builder.member_ref(derived, "Damage", DAMAGE_SIG);
        // static void (int, decimal): the game takes (int, int).
        builder.member_ref(
            cmd,
            "Damage",
            &[0x00, 2, 0x01, 0x08, 0x11, ((decimal << 2) | 1) as u8],
        );
        builder.member_ref(cmd, "Heal", &[0x00, 0, 0x01]);
        builder.member_ref(cmd, "count", &[0x06, 0x08]);
        let metadata = Metadata::parse(builder.image()).expect("mod metadata");
        assert_eq!(
            metadata.assembly_references().expect("refs"),
            ["sts2", "System.Runtime"]
        );
        metadata.references().expect("mod references")
    }

    #[test]
    fn reads_references_and_definitions_from_metadata() {
        let game = game();
        assert!(game.has_type("Game.Cmd") && game.has_type("Game.Derived"));
        let references = mod_references();
        assert!(references.contains(&Reference {
            assembly: "sts2".to_string(),
            owner: "Game.Cmd".to_string(),
            member: Member::Method {
                name: "Damage".to_string(),
                signature: MethodSig {
                    generic_params: 0,
                    return_type: Ty::Primitive("void"),
                    params: vec![
                        Ty::Primitive("int"),
                        Ty::Named("System.Decimal".to_string())
                    ],
                },
            },
        }));
        assert!(
            references
                .iter()
                .any(|reference| reference.owner == "System.Decimal")
        );
    }

    #[test]
    fn reports_members_the_game_lacks() {
        let missing = find_missing(&mod_references(), &[&game()]);
        let described = missing
            .iter()
            .map(MissingReference::describe)
            .collect::<Vec<_>>();
        assert_eq!(
            described,
            [
                "Cmd.Damage(int, decimal) — 게임에서 시그니처가 바뀜",
                "Cmd.Heal() — 게임에서 없어짐",
                "Cmd.count — 게임에서 없어짐",
                "Gone 타입 — 게임에 없음",
            ]
        );
        assert_eq!(missing[0].available, ["Cmd.Damage(int, int)"]);
    }

    #[test]
    fn only_checks_assemblies_given_as_targets() {
        assert!(find_missing(&mod_references(), &[]).is_empty());
    }

    #[test]
    fn decodes_compressed_integers() {
        for (bytes, value) in [
            (&[0x03][..], 3),
            (&[0x7F][..], 0x7F),
            (&[0x80, 0x80][..], 0x80),
            (&[0xBF, 0xFF][..], 0x3FFF),
            (&[0xC0, 0x00, 0x40, 0x00][..], 0x4000),
        ] {
            let mut pos = 0;
            assert_eq!(read_compressed(bytes, &mut pos), Ok(value));
            assert_eq!(pos, bytes.len());
        }
    }

    #[test]
    fn shows_types_like_csharp() {
        let task = Ty::Generic(
            Box::new(Ty::Named("System.Threading.Tasks.Task`1".to_string())),
            vec![Ty::Generic(
                Box::new(Ty::Named("System.Nullable`1".to_string())),
                vec![Ty::Primitive("int")],
            )],
        );
        assert_eq!(type_display(&task), "Task<int?>");
        assert_eq!(short_type_name("A.B.Outer`1/Inner"), "Outer.Inner");
        assert_eq!(
            type_display(&Ty::SzArray(Box::new(Ty::Named(
                "System.Decimal".to_string()
            )))),
            "decimal[]"
        );
    }

    fn installed_game_dll() -> Option<PathBuf> {
        let game_dir = std::env::var_os("STS2_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::config::AppConfig::from_workspace(".").game_dir);
        Some(game_dir.join("data_sts2_windows_x86_64").join("sts2.dll"))
            .filter(|path| path.is_file())
    }

    /// Reads the real game and the bridge built against it; skipped when
    /// either is not on this machine.
    #[test]
    fn reads_installed_game_and_bridge() {
        let bridge = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("vendor")
            .join("bridge")
            .join("STS2ModManagerBridge.dll");
        let Some(game_dll) = installed_game_dll().filter(|_| bridge.is_file()) else {
            eprintln!(
                "skipped: game or bridge build not found ({})",
                bridge.display()
            );
            return;
        };
        let game = Metadata::read(&game_dll)
            .expect("sts2.dll")
            .target()
            .expect("game types");
        assert_eq!(game.name, GAME_ASSEMBLY);
        assert!(game.has_type("MegaCrit.Sts2.Core.Commands.CreatureCmd"));
        let references = Metadata::read(&bridge)
            .expect("bridge")
            .references()
            .expect("refs");
        assert!(
            references
                .iter()
                .any(|reference| reference.assembly == GAME_ASSEMBLY)
        );
        for missing in find_missing(&references, &[&game]) {
            eprintln!("bridge: {}", missing.describe());
        }
    }

    /// Prints what every installed Workshop mod lacks in the installed game:
    /// `cargo test workshop_report -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn workshop_report() {
        let game_dll = installed_game_dll().expect("installed game");
        let game = Metadata::read(&game_dll).unwrap().target().unwrap();
        let workshop = game_dll
            .ancestors()
            .nth(4)
            .unwrap()
            .join("workshop/content/2868840");
        let mut dlls = Vec::new();
        for item in fs::read_dir(workshop).unwrap().flatten() {
            for manifest in crate::discovery::find_mod_manifests(&item.path(), 3) {
                for entry in fs::read_dir(manifest.parent().unwrap()).unwrap().flatten() {
                    if entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
                    {
                        dlls.push(entry.path());
                    }
                }
            }
        }
        for dll in dlls {
            let references = Metadata::read(&dll).unwrap().references().unwrap();
            let missing = find_missing(&references, &[&game]);
            println!(
                "== {}: {} missing",
                dll.file_name().unwrap().to_string_lossy(),
                missing.len()
            );
            for item in missing {
                println!("   {}", item.describe());
                for available in item.available {
                    println!("      게임: {available}");
                }
            }
        }
    }
}
