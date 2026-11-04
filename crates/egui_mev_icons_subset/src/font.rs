//! Subsetting the Lucide font: `subsetter` writes PDF-oriented fonts, so this
//! adds back what egui needs from them.

use crate::Result;

/// The subset font for `chars`, cut from `full`.
pub fn build_subset(full: &[u8], chars: &[char]) -> Result<Vec<u8>> {
    let face = ttf_parser::Face::parse(full, 0)?;
    // `.notdef` first, then one glyph per used icon.
    let mut gids = Vec::with_capacity(chars.len() + 1);
    gids.push(0);
    for ch in chars {
        let gid = face
            .glyph_index(*ch)
            .ok_or_else(|| format!("the Lucide font lacks U+{:04X}", u32::from(*ch)))?;
        gids.push(gid.0);
    }
    let remapper = subsetter::GlyphRemapper::new_from_glyphs(&gids);
    let subset = subsetter::subset(full, 0, &remapper)?;
    // subsetter writes PDF-oriented fonts: it drops `cmap`, because PDFs
    // carry their own. egui needs one, so append a minimal format 4 cmap
    // (BMP covers every Lucide icon codepoint) and rebuild the sfnt.
    let remapped: Vec<(char, u16)> = chars
        .iter()
        .copied()
        .zip(gids.iter().skip(1))
        .map(|(ch, old)| {
            let new = remapper
                .get(*old)
                .ok_or_else(|| format!("glyph {old} missing from the subset remapping"))?;
            Ok((ch, new))
        })
        .collect::<Result<_>>()?;
    with_cmap(&subset, &remapped, full)
}

/// Rewrite `hmtx`/`hhea` so every glyph has its own advance and bearing
/// record, whatever `numberOfHMetrics` the subset has.
fn expand_hmtx(tables: &mut [(u32, Vec<u8>)]) {
    let tag = |name: &[u8; 4]| u32::from_be_bytes(*name);
    let num_glyphs = tables
        .iter()
        .find(|(t, _)| *t == tag(b"maxp"))
        .and_then(|(_, d)| d.get(4..6))
        .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]) as usize);
    let metrics = tables
        .iter()
        .find(|(t, _)| *t == tag(b"hhea"))
        .and_then(|(_, d)| d.get(34..36))
        .map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])));
    let hmtx = tables.iter().find(|(t, _)| *t == tag(b"hmtx"));
    let (Some(num_metrics), Some((_, hmtx))) = (metrics, hmtx) else {
        return;
    };
    if num_glyphs == 0 || num_metrics == 0 || hmtx.len() < num_metrics * 4 {
        return;
    }
    let mut out = Vec::with_capacity(num_glyphs * 4);
    for glyph in 0..num_glyphs {
        let (advance, lsb) = if glyph < num_metrics {
            let at = glyph * 4;
            (
                u16::from_be_bytes([hmtx[at], hmtx[at + 1]]),
                i16::from_be_bytes([hmtx[at + 2], hmtx[at + 3]]),
            )
        } else {
            // Trailing bearings after the last full record.
            let last = (num_metrics - 1) * 4;
            let at = num_metrics * 4 + (glyph - num_metrics) * 2;
            (
                u16::from_be_bytes([hmtx[last], hmtx[last + 1]]),
                hmtx.get(at..at + 2)
                    .map_or(0, |b| i16::from_be_bytes([b[0], b[1]])),
            )
        };
        out.extend(advance.to_be_bytes());
        out.extend(lsb.to_be_bytes());
    }
    for (t, data) in tables.iter_mut() {
        if *t == tag(b"hhea") {
            data[34..36].copy_from_slice(&(num_glyphs as u16).to_be_bytes());
        } else if *t == tag(b"hmtx") {
            data.clone_from(&out);
        }
    }
}

/// Append the tables egui needs to an sfnt subset that subsetter wrote for
/// PDF embedding: a `cmap` mapping `map` (codepoint → glyph) and the
/// original `OS/2`.
///
/// subsetter drops `OS/2`, which carries the ascent/descent metrics font
/// layout uses; without it text and icons are positioned differently.
fn with_cmap(font: &[u8], map: &[(char, u16)], source: &[u8]) -> Result<Vec<u8>> {
    let version: [u8; 4] = font
        .first_chunk::<4>()
        .copied()
        .ok_or("font shorter than its sfnt header")?;
    let mut tables = sfnt_tables(font)?;
    // Expand `hmtx` to one full record per glyph. subsetter writes the
    // minimum (`numberOfHMetrics` = 2) with trailing left-side bearings;
    // hinted rasterization then produces slightly different glyph bounds.
    // With an explicit record per glyph the hinted output matches the full
    // font exactly.
    expand_hmtx(&mut tables);
    tables.push((u32::from_be_bytes(*b"cmap"), cmap_format4(map)?));
    let os2 = u32::from_be_bytes(*b"OS/2");
    if let Some((_, metrics)) = sfnt_tables(source)?
        .into_iter()
        .find(|(tag, _)| *tag == os2)
    {
        tables.push((os2, metrics));
    }
    let out = rebuild_sfnt(version, tables);
    // Sanity: the table must parse and cover every codepoint.
    let face = ttf_parser::Face::parse(&out, 0)?;
    for (ch, _) in map {
        if face.glyph_index(*ch).is_none() {
            return Err(format!("rebuilt cmap lacks U+{:04X}", u32::from(*ch)).into());
        }
    }
    Ok(out)
}

/// Every table of an sfnt font as (tag, data), directory order preserved.
fn sfnt_tables(font: &[u8]) -> Result<Vec<(u32, Vec<u8>)>> {
    let word = |at: usize| -> Result<u32> {
        let bytes = font.get(at..at + 4).ok_or("truncated sfnt header")?;
        Ok(u32::from_be_bytes(bytes.try_into()?))
    };
    let half = |at: usize| -> Result<u16> {
        let bytes = font.get(at..at + 2).ok_or("truncated sfnt header")?;
        Ok(u16::from_be_bytes(bytes.try_into()?))
    };
    // version: u32, numTables: u16, searchRange, entrySelector, rangeShift.
    let count = usize::from(half(4)?);
    let mut tables = Vec::with_capacity(count);
    for i in 0..count {
        let record = 12 + i * 16;
        let tag = word(record)?;
        let offset = word(record + 8)? as usize;
        let length = word(record + 12)? as usize;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= font.len())
            .ok_or("table extends past the end of the font")?;
        tables.push((tag, font[offset..end].to_vec()));
    }
    Ok(tables)
}

/// Serialize tables into a clean sfnt: sorted directory, 4-byte aligned,
/// per-table checksums, `head.checkSumAdjustment` correct.
fn rebuild_sfnt(version: [u8; 4], mut tables: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    tables.sort_unstable_by_key(|(tag, _)| *tag);
    // A table's checksum is taken with `checkSumAdjustment` zeroed.
    if let Some((_, head)) = tables
        .iter_mut()
        .find(|(tag, _)| *tag == u32::from_be_bytes(*b"head"))
        && let Some(adjustment) = head.get_mut(8..12)
    {
        adjustment.fill(0);
    }
    let count = tables.len();
    let floor_log2 = u32::BITS - 1 - (count as u32).leading_zeros();
    let search_range = ((1 << floor_log2) * 16) as u16;

    let mut out = Vec::new();
    out.extend_from_slice(&version);
    out.extend((count as u16).to_be_bytes());
    out.extend(search_range.to_be_bytes());
    out.extend((floor_log2 as u16).to_be_bytes());
    out.extend(((count as u16) * 16 - search_range).to_be_bytes());

    let mut offset = 12 + 16 * count;
    let mut records = Vec::with_capacity(count);
    for (tag, data) in &tables {
        records.push((*tag, checksum(data), offset, data.len()));
        offset += data.len() + (4 - data.len() % 4) % 4;
    }
    for (tag, sum, at, length) in &records {
        out.extend(tag.to_be_bytes());
        out.extend(sum.to_be_bytes());
        out.extend((*at as u32).to_be_bytes());
        out.extend((*length as u32).to_be_bytes());
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        out.extend(std::iter::repeat_n(0, (4 - data.len() % 4) % 4));
    }
    // With `head` zeroed, the whole-font checksum must equal 0xB1B0AFBA.
    if let Some(head) = records
        .iter()
        .find(|(tag, _, _, _)| *tag == u32::from_be_bytes(*b"head"))
    {
        let at = head.2 + 8;
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[at..at + 4].copy_from_slice(&adjustment.to_be_bytes());
    }
    out
}

/// A minimal cmap format 4 subtable (BMP, which covers every Lucide icon
/// codepoint): one segment per codepoint plus the 0xFFFF sentinel, each
/// mapping straight to its glyph.
fn cmap_format4(map: &[(char, u16)]) -> Result<Vec<u8>> {
    let mut chars: Vec<(u32, u16)> = map.iter().map(|(ch, gid)| (u32::from(*ch), *gid)).collect();
    chars.sort_unstable();
    chars.dedup();
    if chars.iter().any(|(ch, _)| *ch > 0xFFFE) {
        return Err("codepoint above the BMP needs cmap format 12".into());
    }
    let segments = chars.len() + 1;
    let floor_log2 = u32::BITS - 1 - (segments as u32).leading_zeros();
    let search_range = ((1 << floor_log2) * 2) as u16;
    let length = 16 + 8 * segments;

    let mut out = Vec::with_capacity(4 + length);
    out.extend(0u16.to_be_bytes()); // cmap version
    out.extend(2u16.to_be_bytes()); // encoding records
    // Windows BMP and Unicode BMP both point at the one subtable.
    for (platform, encoding) in [(3u16, 1u16), (0, 3)] {
        out.extend(platform.to_be_bytes());
        out.extend(encoding.to_be_bytes());
        out.extend(20u32.to_be_bytes()); // 4-byte header + two records
    }
    let at = out.len();
    out.extend(4u16.to_be_bytes()); // subtable format
    out.extend((length as u16).to_be_bytes());
    out.extend(0u16.to_be_bytes()); // language
    out.extend(((segments * 2) as u16).to_be_bytes());
    out.extend(search_range.to_be_bytes());
    out.extend((floor_log2 as u16).to_be_bytes());
    out.extend(((segments as u16) * 2 - search_range).to_be_bytes());
    for (ch, _) in &chars {
        out.extend((*ch as u16).to_be_bytes());
    }
    out.extend(0xFFFFu16.to_be_bytes()); // end codes …
    out.extend(0u16.to_be_bytes()); // … reserved pad
    for (ch, _) in &chars {
        out.extend((*ch as u16).to_be_bytes());
    }
    out.extend(0xFFFFu16.to_be_bytes()); // … start codes
    for (ch, gid) in &chars {
        out.extend(gid.wrapping_sub(*ch as u16).to_be_bytes()); // … id deltas
    }
    out.extend(1u16.to_be_bytes()); // … sentinel delta maps 0xFFFF → 0
    for _ in 0..segments {
        out.extend(0u16.to_be_bytes()); // … id range offsets
    }
    debug_assert_eq!(out.len() - at, length);
    Ok(out)
}

/// The sfnt table checksum: big-endian u32 sum over zero-padded bytes.
fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::build_subset;
    use egui::{Context, FontData, FontDefinitions, FontFamily};
    use lucide_icons::{Icon, LUCIDE_FONT_BYTES};
    use std::sync::Arc;

    const ICONS: [Icon; 4] = [Icon::Image, Icon::Crop, Icon::X, Icon::Layers2];

    fn layout(font: Vec<u8>, text: &str) -> Vec<(usize, f32, f32, egui::Rect)> {
        let ctx = Context::default();
        let mut defs = FontDefinitions::default();
        let mut data = FontData::from_owned(font);
        data.tweak.hinting = Some(false);
        defs.font_data.insert("lucide".to_owned(), Arc::new(data));
        defs.families
            .entry(FontFamily::Name("lucide".into()))
            .or_default()
            .push("lucide".to_owned());
        ctx.set_fonts(defs);
        // Fonts exist only after a pass.
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let galley = ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                text.to_owned(),
                egui::FontId::new(20.0, FontFamily::Name("lucide".into())),
                egui::Color32::WHITE,
            )
        });
        galley
            .rows
            .iter()
            .flat_map(|row| row.glyphs.iter())
            .map(|g| {
                (
                    g.chr as usize,
                    g.pos.x,
                    g.pos.y,
                    egui::Rect::from_min_size(
                        egui::pos2(g.uv_rect.offset.x, g.uv_rect.offset.y),
                        g.uv_rect.size,
                    ),
                )
            })
            .collect()
    }

    /// The subset must render exactly like the font it was cut from; `--check`
    /// guards its coverage, this guards its metrics.
    #[test]
    fn subset_metrics_match_the_full_font() {
        let chars = ICONS.map(char::from);
        let subset = build_subset(LUCIDE_FONT_BYTES, &chars).expect("should subset");
        for icon in ICONS {
            let text = char::from(icon).to_string();
            let full = layout(LUCIDE_FONT_BYTES.to_vec(), &text);
            let sub = layout(subset.clone(), &text);
            assert_eq!(full, sub, "icon {icon:?} lays out differently");
        }
    }

    #[test]
    fn subset_is_smaller_and_covers_only_what_was_asked() {
        let chars = ICONS.map(char::from);
        let subset = build_subset(LUCIDE_FONT_BYTES, &chars).expect("should subset");
        assert!(
            subset.len() < LUCIDE_FONT_BYTES.len() / 10,
            "{}",
            subset.len()
        );
        let face = ttf_parser::Face::parse(&subset, 0).expect("should parse");
        for ch in chars {
            assert!(face.glyph_index(ch).is_some(), "U+{:04X}", u32::from(ch));
        }
        assert!(face.glyph_index(char::from(Icon::Anvil)).is_none());
        assert_eq!(usize::from(face.number_of_glyphs()), chars.len() + 1);
    }

    #[test]
    fn subset_is_deterministic() {
        let chars = ICONS.map(char::from);
        assert_eq!(
            build_subset(LUCIDE_FONT_BYTES, &chars).expect("should subset"),
            build_subset(LUCIDE_FONT_BYTES, &chars).expect("should subset"),
        );
    }

    #[test]
    fn a_codepoint_outside_the_font_is_an_error() {
        let error = build_subset(LUCIDE_FONT_BYTES, &['\u{2603}']).expect_err("should fail");
        assert!(error.to_string().contains("U+2603"), "{error}");
    }
}
