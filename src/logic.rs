//! Reine Logik ohne Windows-Abhängigkeiten (lokal testbar).

use std::path::{Path, PathBuf};

/// Macht aus beliebigem Text einen gültigen Windows-Dateinamen (ohne Endung).
pub fn safe_name(text: &str, max_chars: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed: String = collapsed
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = collapsed.trim_matches(|c| c == ' ' || c == '.');
    let cut: String = trimmed.chars().take(max_chars).collect();
    let cut = cut.trim_end_matches(|c| c == ' ' || c == '.');
    if cut.is_empty() {
        "Druck".to_string()
    } else {
        cut.to_string()
    }
}

/// Erster freier Pfad `ordner/stem.pdf`, sonst `stem_2.pdf`, `stem_3.pdf`, …
pub fn unique_path(folder: &Path, stem: &str) -> PathBuf {
    let mut candidate = folder.join(format!("{stem}.pdf"));
    let mut n = 2;
    while candidate.exists() {
        candidate = folder.join(format!("{stem}_{n}.pdf"));
        n += 1;
    }
    candidate
}

const PREFIXES: [&str; 3] = ["Microsoft Word - ", "Microsoft Excel - ", "Microsoft PowerPoint - "];
const EXTENSIONS: [&str; 16] = [
    ".pdf", ".docx", ".doc", ".xlsx", ".xls", ".pptx", ".ppt", ".txt", ".html", ".htm", ".odt",
    ".ods", ".rtf", ".jpg", ".jpeg", ".png",
];

/// Bereinigt den Dokumentnamen aus dem Druckauftrag ("Microsoft Word - Rechnung.docx" -> "Rechnung").
pub fn clean_document_name(doc: &str) -> String {
    let mut n = doc.trim();
    for pre in PREFIXES {
        if let Some(rest) = n.strip_prefix(pre) {
            n = rest;
        }
    }
    for ext in EXTENSIONS {
        let cut = n.len().saturating_sub(ext.len());
        if n.len() > ext.len() && n.is_char_boundary(cut) && n[cut..].eq_ignore_ascii_case(ext) {
            n = &n[..cut];
            break;
        }
    }
    n.trim().to_string()
}

/// Setzt {datum}, {zeit} und {titel} in das Dateinamen-Muster ein.
pub fn render_pattern(pattern: &str, datum: &str, zeit: &str, titel: &str) -> String {
    pattern.replace("{datum}", datum).replace("{zeit}", zeit).replace("{titel}", titel)
}

/// "v1.2.3" / "1.2" / "0.0.0-dev" -> (1, 2, 3)
pub fn parse_version(v: &str) -> (u32, u32, u32) {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split('-').next().unwrap_or("");
    let mut it = core
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// Dreht ein Bild (32-Bit-Pixel, zeilenweise) um 90° im Uhrzeigersinn.
pub fn rotate90(width: usize, height: usize, px: &[u32]) -> (usize, usize, Vec<u32>) {
    let (nw, nh) = (height, width);
    let mut out = vec![0u32; px.len()];
    for y in 0..height {
        for x in 0..width {
            out[x * nw + (nw - 1 - y)] = px[y * width + x];
        }
    }
    (nw, nh, out)
}

/// Passt ein Bild zentriert auf das Papier ein. Koordinaten relativ zum bedruckbaren Bereich.
pub fn fit_on_paper(
    paper_w: i32,
    paper_h: i32,
    offset_x: i32,
    offset_y: i32,
    img_w: usize,
    img_h: usize,
) -> (i32, i32, i32, i32) {
    let scale = f64::min(paper_w as f64 / img_w as f64, paper_h as f64 / img_h as f64);
    let w = (img_w as f64 * scale).floor() as i32;
    let h = (img_h as f64 * scale).floor() as i32;
    let x = (paper_w - w) / 2 - offset_x;
    let y = (paper_h - h) / 2 - offset_y;
    (x, y, w, h)
}

/// Zweiseitige Test-PDF (Hoch- und Querformat).
pub fn test_pdf_bytes() -> Vec<u8> {
    let pages = [
        (595, 842, "PDF-Ablage Testseite 1 - Hochformat"),
        (842, 595, "PDF-Ablage Testseite 2 - Querformat"),
    ];
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".into(),
    ];
    for (i, (w, h, text)) in pages.iter().enumerate() {
        let content = format!(
            "2 w 20 20 {} {} re S\nBT /F1 24 Tf 60 {} Td ({}) Tj ET\n",
            w - 40,
            h - 40,
            h - 80,
            text
        );
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] \
             /Resources << /Font << /F1 7 0 R >> >> /Contents {} 0 R >>",
            4 + i * 2
        ));
        objs.push(format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content));
    }
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{}\nendobj\n", i + 1, o).as_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objs.len() + 1,
            xref
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dateinamen() {
        assert_eq!(safe_name("Rechnung: 12/2026?", 80), "Rechnung_ 12_2026_");
        assert_eq!(safe_name("  ..  ", 80), "Druck");
        assert_eq!(safe_name("a   b\tc", 80), "a b c");
        assert_eq!(safe_name("Überweisung ÄÖÜ", 5), "Überw");
        assert_eq!(safe_name("abc.", 80), "abc");
    }

    #[test]
    fn dokumentnamen() {
        assert_eq!(clean_document_name("Microsoft Word - Rechnung.docx"), "Rechnung");
        assert_eq!(clean_document_name("Testdokument.pdf"), "Testdokument");
        assert_eq!(clean_document_name("Angebot.PDF"), "Angebot");
        assert_eq!(clean_document_name("Google - Suche"), "Google - Suche");
        assert_eq!(clean_document_name(".pdf"), ".pdf");
        assert_eq!(clean_document_name("Größe.txt"), "Größe");
    }

    #[test]
    fn muster() {
        assert_eq!(
            render_pattern("{datum}_{zeit}_{titel}", "2026-11-01", "16-06-00", "Rechnung"),
            "2026-11-01_16-06-00_Rechnung"
        );
    }

    #[test]
    fn versionen() {
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("0.0.0-dev"), (0, 0, 0));
        assert_eq!(parse_version("v1.10"), (1, 10, 0));
        assert!(parse_version("v2.0.0") > parse_version("1.2.0"));
        assert!(parse_version("1.10.0") > parse_version("1.9.9"));
    }

    #[test]
    fn drehen() {
        // 3x2 -> 2x3, im Uhrzeigersinn
        let px = [1, 2, 3, 4, 5, 6];
        let (w, h, r) = rotate90(3, 2, &px);
        assert_eq!((w, h), (2, 3));
        assert_eq!(r, vec![4, 1, 5, 2, 6, 3]);
    }

    #[test]
    fn einpassen() {
        // A4 bei 600 dpi, Bild A4 bei 300 dpi -> füllt das Papier
        let (x, y, w, h) = fit_on_paper(4960, 7016, 100, 100, 2480, 3508);
        assert_eq!((w, h), (4960, 7016));
        assert_eq!((x, y), (-100, -100));
    }

    #[test]
    fn eindeutige_pfade() {
        let dir = std::env::temp_dir().join(format!("pdfablage-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = unique_path(&dir, "x");
        std::fs::write(&a, b"1").unwrap();
        let b = unique_path(&dir, "x");
        assert_eq!(b.file_name().unwrap(), "x_2.pdf");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_pdf() {
        let pdf = test_pdf_bytes();
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.trim_end().ends_with("%%EOF"));
        // xref-Offsets zeigen auf die Objekte
        let xref_pos: usize = s.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(s[xref_pos..].starts_with("xref"));
        for (i, line) in s[xref_pos..].lines().skip(3).take(7).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(s[off..].starts_with(&format!("{} 0 obj", i + 1)), "Objekt {}", i + 1);
        }
    }
}
