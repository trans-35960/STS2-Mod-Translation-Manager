//! Renders prompt text as a compact PDF. Some AI chats (Gemini) bill a PDF
//! page as a fixed number of tokens, so small text packed onto a few pages
//! can be cheaper than pasting the same text. Fonts are taken from the
//! system and subset by krilla, so the files stay small.

use krilla::Document;
use krilla::geom::Point;
use krilla::page::PageSettings;
use krilla::text::{Font, TextDirection};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

const PAGE_WIDTH: f32 = 595.0;
const PAGE_HEIGHT: f32 = 842.0;
const MARGIN: f32 = 28.0;
/// AI chats read the PDF's text layer, not the rendered glyphs, so small
/// text only means fewer pages.
pub(crate) const DEFAULT_PDF_FONT_SIZE: f32 = 3.0;
const LINE_SPACING: f32 = 1.25;
const TAB_WIDTH: usize = 4;

/// Korean first, then Chinese and Japanese for glyphs Malgun lacks, then
/// Latin/symbol fonts.
const FONT_FILES: &[(&str, u32)] = &[
    ("malgun.ttf", 0),
    ("msyh.ttc", 0),
    ("YuGothR.ttc", 0),
    ("msgothic.ttc", 0),
    ("segoeui.ttf", 0),
    ("seguisym.ttf", 0),
];

struct LoadedFont {
    data: Arc<Vec<u8>>,
    index: u32,
    font: Font,
    units_per_em: f32,
    ascender: f32,
}

/// System fonts, loaded only when a character needs them.
struct FontSet {
    paths: Vec<(PathBuf, u32)>,
    loaded: Vec<Option<LoadedFont>>,
    tried: Vec<bool>,
    /// char -> (font slot, advance in em)
    glyphs: HashMap<char, (usize, f32)>,
}

impl FontSet {
    fn system() -> Result<Self, String> {
        let dirs = font_dirs();
        let paths = FONT_FILES
            .iter()
            .filter_map(|(name, index)| {
                dirs.iter()
                    .map(|dir| dir.join(name))
                    .find(|path| path.is_file())
                    .map(|path| (path, *index))
            })
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return Err("PDF에 쓸 시스템 글꼴(맑은 고딕 등)을 찾지 못했습니다.".to_string());
        }
        Ok(Self {
            loaded: paths.iter().map(|_| None).collect(),
            tried: vec![false; paths.len()],
            paths,
            glyphs: HashMap::new(),
        })
    }

    fn load(&mut self, slot: usize) -> Option<&LoadedFont> {
        if !self.tried[slot] {
            self.tried[slot] = true;
            let (path, index) = &self.paths[slot];
            self.loaded[slot] = std::fs::read(path).ok().and_then(|bytes| {
                let data = Arc::new(bytes);
                let face = ttf_parser::Face::parse(&data, *index).ok()?;
                let units_per_em = f32::from(face.units_per_em());
                let ascender = f32::from(face.ascender()) / units_per_em;
                let font = Font::new(data.clone().into(), *index)?;
                Some(LoadedFont {
                    data: data.clone(),
                    index: *index,
                    font,
                    units_per_em,
                    ascender,
                })
            });
        }
        self.loaded[slot].as_ref()
    }

    /// The first font that has the character, and its advance in em.
    fn glyph(&mut self, ch: char) -> (usize, f32) {
        if let Some(found) = self.glyphs.get(&ch) {
            return *found;
        }
        let mut found = None;
        for slot in 0..self.paths.len() {
            let Some(font) = self.load(slot) else {
                continue;
            };
            let Ok(face) = ttf_parser::Face::parse(&font.data, font.index) else {
                continue;
            };
            if let Some(glyph) = face.glyph_index(ch) {
                let advance =
                    f32::from(face.glyph_hor_advance(glyph).unwrap_or(0)) / font.units_per_em;
                found = Some((slot, advance));
                break;
            }
        }
        // Missing everywhere: draw the first font's .notdef box.
        let found = found.unwrap_or((self.first_loaded(), 0.5));
        self.glyphs.insert(ch, found);
        found
    }

    fn first_loaded(&mut self) -> usize {
        (0..self.paths.len())
            .find(|slot| self.load(*slot).is_some())
            .unwrap_or(0)
    }
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join(r"Microsoft\Windows\Fonts"));
    }
    dirs
}

/// Renders `text` onto A4 pages and returns the PDF bytes.
pub(crate) fn render_text_pdf(text: &str, font_size: f32) -> Result<Vec<u8>, String> {
    let font_size = font_size.clamp(2.0, 14.0);
    let mut fonts = FontSet::system()?;
    let max_width = (PAGE_WIDTH - MARGIN * 2.0) / font_size;
    let lines = wrap_lines(text, max_width, |ch| fonts.glyph(ch).1);

    let line_height = font_size * LINE_SPACING;
    let lines_per_page = (((PAGE_HEIGHT - MARGIN * 2.0) / line_height).floor() as usize).max(1);
    let base_slot = fonts.first_loaded();
    let ascender = fonts
        .load(base_slot)
        .map(|font| font.ascender)
        .unwrap_or(0.9);

    let mut document = Document::new();
    for page_lines in lines.chunks(lines_per_page) {
        let settings =
            PageSettings::from_wh(PAGE_WIDTH, PAGE_HEIGHT).ok_or("잘못된 페이지 크기")?;
        let mut page = document.start_page_with(settings);
        let mut surface = page.surface();
        for (row, line) in page_lines.iter().enumerate() {
            let baseline = MARGIN + row as f32 * line_height + ascender * font_size;
            let mut x = MARGIN;
            for (slot, run, width) in font_runs(line, &mut fonts) {
                if !run.trim().is_empty()
                    && let Some(font) = fonts.load(slot)
                {
                    surface.draw_text(
                        Point::from_xy(x, baseline),
                        font.font.clone(),
                        font_size,
                        &run,
                        false,
                        TextDirection::LeftToRight,
                    );
                }
                x += width * font_size;
            }
        }
        surface.finish();
        page.finish();
    }
    document
        .finish()
        .map_err(|error| format!("PDF를 만들지 못했습니다: {error:?}"))
}

/// Splits a line into runs that share a font: `(font slot, text, width in em)`.
fn font_runs(line: &str, fonts: &mut FontSet) -> Vec<(usize, String, f32)> {
    let mut runs: Vec<(usize, String, f32)> = Vec::new();
    for ch in line.chars() {
        let (slot, advance) = fonts.glyph(ch);
        match runs.last_mut() {
            Some((last, text, width)) if *last == slot => {
                text.push(ch);
                *width += advance;
            }
            _ => runs.push((slot, ch.to_string(), advance)),
        }
    }
    runs
}

/// Wraps text to `max_width` (in the same unit `advance` returns). Breaks at
/// the last space when one is reasonably close, otherwise mid-word, which is
/// what CJK text needs anyway. Tabs become spaces; blank lines are kept.
fn wrap_lines(text: &str, max_width: f32, mut advance: impl FnMut(char) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.replace("\r\n", "\n").split('\n') {
        let line = raw.replace('\t', &" ".repeat(TAB_WIDTH));
        let mut current = String::new();
        let mut width = 0.0;
        // Byte offset just after the last space in `current`, and the width up to it.
        let mut last_space: Option<(usize, f32)> = None;
        for ch in line.chars() {
            let ch_width = advance(ch);
            if width + ch_width > max_width && !current.is_empty() {
                match last_space.filter(|(_, space_width)| *space_width > max_width * 0.5) {
                    Some((offset, space_width)) => {
                        let rest = current.split_off(offset);
                        lines.push(current.trim_end().to_string());
                        current = rest;
                        width -= space_width;
                    }
                    None => {
                        lines.push(std::mem::take(&mut current));
                        width = 0.0;
                    }
                }
                last_space = None;
            }
            current.push(ch);
            width += ch_width;
            if ch == ' ' {
                last_space = Some((current.len(), width));
            }
        }
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_spaces_or_mid_word() {
        let lines = wrap_lines("aaaa bbbb cccc", 10.0, |_| 1.0);
        assert_eq!(lines, vec!["aaaa bbbb", "cccc"]);
        let lines = wrap_lines("가나다라마바사아자차카", 4.0, |_| 1.0);
        assert_eq!(lines, vec!["가나다라", "마바사아", "자차카"]);
        let lines = wrap_lines("a\n\n\tb", 10.0, |_| 1.0);
        assert_eq!(lines, vec!["a", "", "    b"]);
    }

    #[test]
    fn renders_mixed_script_text_with_subset_fonts() {
        let Ok(_) = FontSet::system() else {
            return;
        };
        let text = "너는 번역가다.\n```json\n{\"k001-ab\": \"[gold]谦逊[/gold] Strike 6\"}\n```\n"
            .repeat(80);
        let pdf = render_text_pdf(&text, DEFAULT_PDF_FONT_SIZE).expect("pdf");
        assert!(pdf.starts_with(b"%PDF"));
        // Subset fonts, not whole multi-megabyte font files.
        assert!(pdf.len() < 1_000_000, "{} bytes", pdf.len());
    }
}
