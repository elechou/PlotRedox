// build_font_subset.rs — Subset CJK font at build time.
//
// Scans project source files for CJK characters, uses `subsetter` to strip
// unused glyphs, then injects a `cmap` table back (subsetter removes it
// because it targets PDF embedding). The result is a minimal but fully
// functional TrueType font written to OUT_DIR.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Extra Chinese punctuation to always include in the subset.
const EXTRA_CHARS: &str = "，。！？：；\u{201c}\u{201d}\u{2018}\u{2019}（）、—…【】《》";

// ── CJK codepoint collection ────────────────────────────────────────────

/// Scan project files and collect all CJK codepoints used.
fn collect_cjk_codepoints() -> Vec<char> {
    let mut scan_files: Vec<String> = vec![
        "src/i18n.rs".into(),
        "docs/scripting_help_zh.md".into(),
    ];
    if let Ok(entries) = fs::read_dir("example_scripts") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                scan_files.push(path.to_string_lossy().into_owned());
            }
        }
    }

    let mut chars = BTreeSet::new();
    for ch in EXTRA_CHARS.chars() {
        chars.insert(ch);
    }

    for fpath in &scan_files {
        if let Ok(content) = fs::read_to_string(fpath) {
            for ch in content.chars() {
                let cp = ch as u32;
                if (0x4E00..=0x9FFF).contains(&cp)   // CJK Unified Ideographs
                    || (0x3000..=0x303F).contains(&cp) // CJK Symbols & Punctuation
                    || (0xFF00..=0xFFEF).contains(&cp) // Halfwidth / Fullwidth Forms
                {
                    chars.insert(ch);
                }
            }
        }
    }

    chars.into_iter().collect()
}

// ── cmap table builder (format 12) ──────────────────────────────────────

/// Build a cmap table (format 12 — segmented coverage) from a set of
/// (unicode_codepoint, new_glyph_id) mappings.
fn build_cmap_table(mappings: &[(u32, u16)]) -> Vec<u8> {
    let mut sorted: Vec<(u32, u32)> = mappings.iter().map(|&(cp, gid)| (cp, gid as u32)).collect();
    sorted.sort_by_key(|&(cp, _)| cp);
    sorted.dedup_by_key(|m| m.0);

    // Build contiguous groups where codepoints AND glyph IDs are consecutive.
    let mut groups: Vec<(u32, u32, u32)> = Vec::new(); // (startChar, endChar, startGID)
    let mut i = 0;
    while i < sorted.len() {
        let start_cp = sorted[i].0;
        let start_gid = sorted[i].1;
        let mut end_cp = start_cp;
        while i + 1 < sorted.len()
            && sorted[i + 1].0 == end_cp + 1
            && sorted[i + 1].1 == start_gid + (end_cp - start_cp) + 1
        {
            end_cp += 1;
            i += 1;
        }
        groups.push((start_cp, end_cp, start_gid));
        i += 1;
    }

    let num_groups = groups.len() as u32;
    let subtable_len = 16 + num_groups * 12; // format-12 header (16) + groups

    let mut buf = Vec::new();

    // ── cmap header ──
    buf.extend_from_slice(&0u16.to_be_bytes()); // version
    buf.extend_from_slice(&1u16.to_be_bytes()); // numTables

    // ── encoding record (platform 3 = Windows, encoding 10 = Unicode UCS-4) ──
    buf.extend_from_slice(&3u16.to_be_bytes()); // platformID
    buf.extend_from_slice(&10u16.to_be_bytes()); // encodingID
    buf.extend_from_slice(&12u32.to_be_bytes()); // offset to subtable (4 + 8 = 12)

    // ── format 12 subtable ──
    buf.extend_from_slice(&12u16.to_be_bytes()); // format
    buf.extend_from_slice(&0u16.to_be_bytes());  // reserved
    buf.extend_from_slice(&subtable_len.to_be_bytes()); // length
    buf.extend_from_slice(&0u32.to_be_bytes());  // language
    buf.extend_from_slice(&num_groups.to_be_bytes());

    for &(start_cp, end_cp, start_gid) in &groups {
        buf.extend_from_slice(&start_cp.to_be_bytes());
        buf.extend_from_slice(&end_cp.to_be_bytes());
        buf.extend_from_slice(&start_gid.to_be_bytes());
    }

    buf
}

// ── Font table injection ────────────────────────────────────────────────

/// Parse a raw TrueType/OpenType font, add extra tables, and reconstruct.
fn inject_tables(font_data: &[u8], extra_tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    // ── Parse offset table ──
    let sf_version = u32::from_be_bytes(font_data[0..4].try_into().unwrap());
    let num_tables = u16::from_be_bytes(font_data[4..6].try_into().unwrap()) as usize;

    // ── Read existing table records ──
    struct Tbl {
        tag: [u8; 4],
        data: Vec<u8>,
    }
    let mut tables: Vec<Tbl> = Vec::new();
    for i in 0..num_tables {
        let rec_off = 12 + i * 16;
        let tag: [u8; 4] = font_data[rec_off..rec_off + 4].try_into().unwrap();
        let offset = u32::from_be_bytes(font_data[rec_off + 8..rec_off + 12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(font_data[rec_off + 12..rec_off + 16].try_into().unwrap()) as usize;
        tables.push(Tbl {
            tag,
            data: font_data[offset..offset + length].to_vec(),
        });
    }

    // ── Add extra tables (skip if tag already exists) ──
    for (tag, data) in extra_tables {
        if !tables.iter().any(|t| t.tag == *tag) {
            tables.push(Tbl {
                tag: *tag,
                data: data.clone(),
            });
        }
    }

    // ── Sort by tag (OpenType requirement) ──
    tables.sort_by_key(|t| t.tag);

    // ── Reconstruct font ──
    let new_count = tables.len() as u16;
    let entry_selector = (new_count as f32).log2().floor() as u16;
    let search_range = 2u16.pow(entry_selector as u32) * 16;
    let range_shift = new_count * 16 - search_range;

    let header_size = 12 + tables.len() * 16;
    let mut out = Vec::new();

    // Offset table
    out.extend_from_slice(&sf_version.to_be_bytes());
    out.extend_from_slice(&new_count.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    // Calculate offsets and write table records
    let mut data_offset = header_size;
    let mut head_checksum_adj_offset: Option<usize> = None;

    for tbl in &tables {
        let padded_len = (tbl.data.len() + 3) & !3;

        if &tbl.tag == b"head" {
            head_checksum_adj_offset = Some(data_offset + 8);
        }

        out.extend_from_slice(&tbl.tag);
        out.extend_from_slice(&table_checksum(&tbl.data).to_be_bytes());
        out.extend_from_slice(&(data_offset as u32).to_be_bytes());
        out.extend_from_slice(&(tbl.data.len() as u32).to_be_bytes());

        data_offset += padded_len;
    }

    // Write table data (4-byte aligned)
    for tbl in &tables {
        out.extend_from_slice(&tbl.data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }

    // Fix head table checksum adjustment
    if let Some(adj) = head_checksum_adj_offset {
        if adj + 4 <= out.len() {
            out[adj..adj + 4].fill(0);
            let sum = table_checksum(&out);
            let val = 0xB1B0AFBA_u32.wrapping_sub(sum);
            out[adj..adj + 4].copy_from_slice(&val.to_be_bytes());
        }
    }

    out
}

fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut bytes = [0u8; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(bytes));
    }
    sum
}

// ── Public entry point ──────────────────────────────────────────────────

/// Generate a subsetted font file in `out_dir`.
///
/// Source font: `assets/fonts/SarasaUiSC-Regular.ttf`
/// Output:      `<out_dir>/SarasaUiSC-Regular-subset.ttf`
pub fn generate_subset_font(out_dir: &str) {
    let source_font_path = Path::new("assets/fonts/SarasaUiSC-Regular.ttf");
    if !source_font_path.exists() {
        panic!(
            "Source font not found at {}. Please place the full SarasaUiSC-Regular.ttf there.",
            source_font_path.display()
        );
    }

    let font_data = fs::read(source_font_path).expect("Failed to read source font");
    let face = ttf_parser::Face::parse(&font_data, 0).expect("Failed to parse font");

    let cjk_chars = collect_cjk_codepoints();

    // ── Collect (unicode_codepoint, old_glyph_id) pairs ──
    let mut codepoint_to_old_gid: Vec<(u32, u16)> = Vec::new();

    // Basic Latin (U+0000–00FF)
    for cp in 0u32..=0xFF {
        if let Some(ch) = char::from_u32(cp) {
            if let Some(gid) = face.glyph_index(ch) {
                codepoint_to_old_gid.push((cp, gid.0));
            }
        }
    }
    // General Punctuation (U+2000–206F)
    for cp in 0x2000u32..=0x206F {
        if let Some(ch) = char::from_u32(cp) {
            if let Some(gid) = face.glyph_index(ch) {
                codepoint_to_old_gid.push((cp, gid.0));
            }
        }
    }
    // CJK characters
    for &ch in &cjk_chars {
        if let Some(gid) = face.glyph_index(ch) {
            codepoint_to_old_gid.push((ch as u32, gid.0));
        }
    }

    // ── Build glyph remapper and subset ──
    let mut old_gids: BTreeSet<u16> = BTreeSet::new();
    old_gids.insert(0); // .notdef
    for &(_, gid) in &codepoint_to_old_gid {
        old_gids.insert(gid);
    }
    let gids_vec: Vec<u16> = old_gids.into_iter().collect();

    let remapper = subsetter::GlyphRemapper::new_from_glyphs_sorted(&gids_vec);
    let subset = subsetter::subset(&font_data, 0, &remapper).expect("Failed to subset font");

    // ── Build cmap mapping: unicode → new glyph ID ──
    let cmap_mappings: Vec<(u32, u16)> = codepoint_to_old_gid
        .iter()
        .filter_map(|&(cp, old_gid)| {
            remapper.get(old_gid).map(|new_gid| (cp, new_gid))
        })
        .collect();

    let cmap_table = build_cmap_table(&cmap_mappings);

    // ── Also copy OS/2 table from original font (needed for proper metrics) ──
    let mut extra_tables: Vec<([u8; 4], Vec<u8>)> = vec![(*b"cmap", cmap_table)];
    if let Some(os2) = face.raw_face().table(ttf_parser::Tag::from_bytes(b"OS/2")) {
        extra_tables.push((*b"OS/2", os2.to_vec()));
    }

    // ── Inject tables into subsetted font ──
    let final_font = inject_tables(&subset, &extra_tables);

    let out_path = Path::new(out_dir).join("SarasaUiSC-Regular-subset.ttf");
    fs::write(&out_path, &final_font).expect("Failed to write subset font");

    println!(
        "cargo:warning=Font subset: {} glyphs, {} CJK chars, {} KB",
        remapper.num_gids(),
        cjk_chars.len(),
        final_font.len() / 1024
    );
}
