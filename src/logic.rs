//! Reine Logik ohne Windows-Abhängigkeiten (lokal testbar).

use std::path::{Path, PathBuf};

/// Gerätenamen, die Windows nicht als Dateinamen zulässt (auch nicht mit Endung).
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Macht aus beliebigem Text einen gültigen Windows-Dateinamen (ohne Endung) mit höchstens
/// `max_len` UTF-16-Zeichen (so zählt Windows die Länge).
pub fn safe_name(text: &str, max_len: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed: String = collapsed
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = collapsed.trim_matches(|c| c == ' ' || c == '.');
    let mut cut = String::new();
    let mut len = 0;
    for c in trimmed.chars() {
        len += c.len_utf16();
        if len > max_len {
            break;
        }
        cut.push(c);
    }
    let cut = cut.trim_end_matches(|c| c == ' ' || c == '.');
    if cut.is_empty() {
        return "Druck".to_string();
    }
    // "CON", "nul.txt" usw. sind unter Windows keine gültigen Dateinamen
    let base = cut.split('.').next().unwrap_or("").trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(base)) {
        return format!("_{cut}");
    }
    cut.to_string()
}

/// Längster erlaubter Dateiname (ohne „.pdf“) für alle Speicherorte: Der vollständige Pfad
/// bleibt unter 260 Zeichen – auch mit Nummer „_9999“ –, damit Explorer, Backup-Programme
/// und ältere Software die Datei problemlos öffnen. Mindestens 20, höchstens 150 Zeichen.
pub fn max_stem_len<S: AsRef<str>>(dirs: &[S]) -> usize {
    const MAX_PATH: usize = 259;
    const RESERVE: usize = 1 + "_9999".len() + ".pdf".len(); // Trenner, Nummer, Endung
    let longest = dirs.iter().map(|d| d.as_ref().trim_end_matches(['\\', '/']).encode_utf16().count()).max().unwrap_or(0);
    MAX_PATH.saturating_sub(longest + RESERVE).clamp(20, 150)
}

/// Legt die fertig geschriebene Datei `part` als `ordner/stem.pdf` ab – oder, wenn es die schon
/// gibt, als `stem_2.pdf`, `stem_3.pdf`, … Eine vorhandene Datei wird nie überschrieben: Der
/// Zielname wird per Hardlink angelegt, und das schlägt fehl, wenn er schon existiert (auch
/// wenn ein anderes Programm ihn gerade erst angelegt hat). Auf Laufwerken ohne Hardlinks
/// (z. B. FAT32, manche NAS) wird stattdessen umbenannt.
pub fn place_file(part: &Path, folder: &Path, stem: &str) -> std::io::Result<PathBuf> {
    for n in 1..=9999u32 {
        let candidate =
            if n == 1 { folder.join(format!("{stem}.pdf")) } else { folder.join(format!("{stem}_{n}.pdf")) };
        if candidate.exists() {
            continue;
        }
        match std::fs::hard_link(part, &candidate) {
            Ok(()) => {
                let _ = std::fs::remove_file(part);
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => {
                if candidate.exists() {
                    continue;
                }
                std::fs::rename(part, &candidate)?;
                return Ok(candidate);
            }
        }
    }
    Err(std::io::Error::other(format!("Kein freier Dateiname für „{stem}“ gefunden.")))
}

const PREFIXES: [(&str, &str); 3] =
    [("Microsoft Word - ", "Word"), ("Microsoft Excel - ", "Excel"), ("Microsoft PowerPoint - ", "PowerPoint")];
const EXTENSIONS: [&str; 16] = [
    ".pdf", ".docx", ".doc", ".xlsx", ".xls", ".pptx", ".ppt", ".txt", ".html", ".htm", ".odt",
    ".ods", ".rtf", ".jpg", ".jpeg", ".png",
];

/// Bereinigt den Dokumentnamen aus dem Druckauftrag ("Microsoft Word - Rechnung.docx" -> "Rechnung").
pub fn clean_document_name(doc: &str) -> String {
    let mut n = doc.trim();
    for (pre, _) in PREFIXES {
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

/// Programmname aus dem Dokumentnamen, falls das Programm ihn voranstellt ("Microsoft Word - …").
pub fn program_from_document(doc: &str) -> Option<String> {
    PREFIXES.iter().find(|(pre, _)| doc.trim_start().starts_with(pre)).map(|(_, name)| name.to_string())
}

/// Lesbarer Programmname aus dem Namen der .exe ("WINWORD.EXE" -> "Word", "Lexware.exe" -> "Lexware").
/// `None` für Windows-Bestandteile, die nur „vorne“ liegen, aber nicht drucken (Explorer, Taskleiste …).
pub fn program_from_exe(exe: &str) -> Option<String> {
    let file = exe.rsplit(['\\', '/']).next().unwrap_or(exe).trim();
    let stem = if file.len() > 4 && file.is_char_boundary(file.len() - 4) && file[file.len() - 4..].eq_ignore_ascii_case(".exe")
    {
        &file[..file.len() - 4]
    } else {
        file
    };
    if stem.is_empty() {
        return None;
    }
    const IGNORE: [&str; 6] = ["explorer", "ShellExperienceHost", "SearchHost", "StartMenuExperienceHost", "LockApp", "PDF-Ablage"];
    if IGNORE.iter().any(|i| i.eq_ignore_ascii_case(stem)) {
        return None;
    }
    const KNOWN: [(&str, &str); 12] = [
        ("WINWORD", "Word"),
        ("EXCEL", "Excel"),
        ("POWERPNT", "PowerPoint"),
        ("OUTLOOK", "Outlook"),
        ("olk", "Outlook"),
        ("msedge", "Edge"),
        ("chrome", "Chrome"),
        ("firefox", "Firefox"),
        ("AcroRd32", "Acrobat"),
        ("Acrobat", "Acrobat"),
        ("notepad", "Editor"),
        ("soffice.bin", "LibreOffice"),
    ];
    Some(KNOWN.iter().find(|(e, _)| e.eq_ignore_ascii_case(stem)).map(|(_, n)| n.to_string()).unwrap_or_else(|| stem.to_string()))
}

/// Platzhalter der Dateinamen-Vorlage. {zeit} ist der ältere Name von {uhrzeit}.
pub const PLACEHOLDERS: [&str; 5] = ["titel", "datum", "uhrzeit", "zeit", "programm"];

/// Werte für die Platzhalter.
#[derive(Clone, Debug, Default)]
pub struct NameParts {
    pub titel: String,
    pub datum: String,
    pub uhrzeit: String,
    pub programm: String,
}

impl NameParts {
    fn get(&self, key: &str) -> Option<&str> {
        match key {
            "titel" => Some(&self.titel),
            "datum" => Some(&self.datum),
            "uhrzeit" | "zeit" => Some(&self.uhrzeit),
            "programm" => Some(&self.programm),
            _ => None,
        }
    }
}

/// Entfernt eine abschließende „.pdf“ aus der Vorlage (die Endung hängt PDF-Ablage selbst an).
fn strip_pdf(pattern: &str) -> &str {
    let p = pattern.trim();
    let cut = p.len().saturating_sub(4);
    if p.len() >= 4 && p.is_char_boundary(cut) && p[cut..].eq_ignore_ascii_case(".pdf") {
        &p[..cut]
    } else {
        p
    }
}

fn is_sep(c: char) -> bool {
    matches!(c, '_' | '-' | ' ' | '.')
}

/// Setzt die Platzhalter in die Dateinamen-Vorlage ein (Ergebnis ohne „.pdf“).
///
/// Ist ein Platzhalter leer (z. B. {programm}, wenn das Programm nicht erkennbar war), fällt
/// auch ein Trennzeichen daneben weg: „{datum}_{programm}_{titel}“ ergibt dann
/// „2026-11-15_Rechnung“ statt „2026-11-15__Rechnung“.
pub fn render_pattern(pattern: &str, parts: &NameParts) -> String {
    let pattern = strip_pdf(pattern);
    let mut out = String::new();
    let mut rest = pattern;
    let mut skip_sep = false;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            if let Some(end) = rest.find('}') {
                if let Some(value) = parts.get(&rest[1..end].to_lowercase()) {
                    let value = value.trim();
                    if value.is_empty() {
                        skip_sep = true;
                    } else {
                        out.push_str(value);
                        skip_sep = false;
                    }
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        if skip_sep && is_sep(c) && (out.is_empty() || out.ends_with(is_sep)) {
            // Trennzeichen eines leeren Platzhalters weglassen
        } else {
            out.push(c);
        }
        skip_sep = false;
        rest = &rest[c.len_utf8()..];
    }
    if skip_sep {
        // Vorlage endet mit einem leeren Platzhalter
        out.truncate(out.trim_end_matches(is_sep).len());
    }
    out
}

/// Prüft eine Dateinamen-Vorlage. `None` = in Ordnung, sonst eine Meldung für den Nutzer.
pub fn pattern_problem(pattern: &str) -> Option<String> {
    let p = strip_pdf(pattern);
    if p.trim().is_empty() {
        return Some("Die Dateinamen-Vorlage darf nicht leer sein.".into());
    }
    let mut rest = p;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let key = &rest[start + 1..start + len];
        if !PLACEHOLDERS.contains(&key.to_lowercase().as_str()) {
            return Some(format!(
                "Unbekannter Platzhalter „{{{key}}}“ in der Dateinamen-Vorlage.\n\
                 Möglich sind: {{titel}} {{datum}} {{uhrzeit}} {{programm}}"
            ));
        }
        rest = &rest[start + len + 1..];
    }
    None
}

/// Anschluss und Treiber eines Druckers in Windows.
#[derive(Clone, Debug, PartialEq)]
pub struct PrinterState {
    pub port: String,
    pub driver: String,
}

/// Was an der Einrichtung in Windows nicht (mehr) stimmt – z. B. nach einem Windows-Update.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// Der Treiber „Microsoft Print to PDF“ ist nicht installiert
    DriverMissing,
    /// Der Anschluss (Named Pipe) ist nicht mehr registriert
    PortMissing,
    /// Den Drucker gibt es nicht (mehr)
    PrinterMissing(String),
    /// Der Drucker druckt an einen anderen Anschluss (z. B. „PORTPROMPT:“ → Speichern-Dialog)
    WrongPort { printer: String, port: String },
    /// Anschluss einer älteren PDF-Ablage-Version
    OldPort(String),
    /// Der Drucker nutzt einen anderen Treiber
    WrongDriver { printer: String, driver: String },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::DriverMissing => write!(f, "Der Windows-Treiber „Microsoft Print to PDF“ fehlt."),
            Problem::PortMissing => write!(f, "Der Anschluss von PDF-Ablage fehlt in Windows."),
            Problem::PrinterMissing(n) => write!(f, "Der Drucker „{n}“ fehlt in Windows."),
            Problem::WrongPort { printer, port } => {
                write!(f, "„{printer}“ druckt nicht mehr an PDF-Ablage (Anschluss jetzt: {port}).")
            }
            Problem::OldPort(n) => write!(f, "„{n}“ ist noch von einer älteren Version eingerichtet."),
            Problem::WrongDriver { printer, driver } => {
                write!(f, "„{printer}“ nutzt einen anderen Treiber („{driver}“).")
            }
        }
    }
}

/// Erkennt den Treiber „Microsoft Print to PDF“ (wie bei der Einrichtung: Name passt auf *Print*PDF*).
pub fn is_pdf_driver(name: &str) -> bool {
    let n = name.to_lowercase();
    n.find("print").is_some_and(|i| n[i..].contains("pdf"))
}

/// Vergleicht die Einstellungen mit dem, was Windows meldet.
///
/// * `printers`: Namen der eingestellten Drucker
/// * `pipe`: Anschluss, auf den alle zeigen müssen
/// * `is_old_port`: erkennt Anschlüsse älterer Versionen
/// * `lookup`: Anschluss/Treiber eines Druckers, `None` = nicht vorhanden
/// * `ports`, `drivers`: Listen aus Windows, `None` = nicht lesbar (dann nicht prüfen)
pub fn diagnose(
    printers: &[String],
    pipe: &str,
    is_old_port: impl Fn(&str) -> bool,
    lookup: impl Fn(&str) -> Option<PrinterState>,
    ports: Option<&[String]>,
    drivers: Option<&[String]>,
) -> Vec<Problem> {
    let mut out = Vec::new();
    if drivers.is_some_and(|d| !d.iter().any(|n| is_pdf_driver(n))) {
        out.push(Problem::DriverMissing);
    }
    if ports.is_some_and(|p| !p.iter().any(|n| n.eq_ignore_ascii_case(pipe))) {
        out.push(Problem::PortMissing);
    }
    for name in printers {
        match lookup(name) {
            None => out.push(Problem::PrinterMissing(name.clone())),
            Some(st) => {
                if is_old_port(&st.port) {
                    out.push(Problem::OldPort(name.clone()));
                } else if !st.port.eq_ignore_ascii_case(pipe) {
                    out.push(Problem::WrongPort { printer: name.clone(), port: st.port });
                }
                if !is_pdf_driver(&st.driver) {
                    out.push(Problem::WrongDriver { printer: name.clone(), driver: st.driver });
                }
            }
        }
    }
    out
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
        assert_eq!(safe_name("", 80), "Druck");
        assert_eq!(safe_name("a   b\tc\r\nd", 80), "a b c d");
        assert_eq!(safe_name("Überweisung ÄÖÜ", 5), "Überw");
        assert_eq!(safe_name("abc.", 80), "abc");
        assert_eq!(safe_name("<>|*\"", 80), "_____");
        assert_eq!(safe_name("Bericht\u{0}\u{7}", 80), "Bericht__");
    }

    #[test]
    fn reservierte_namen() {
        assert_eq!(safe_name("CON", 80), "_CON");
        assert_eq!(safe_name("nul.txt", 80), "_nul.txt");
        assert_eq!(safe_name("com1", 80), "_com1");
        assert_eq!(safe_name("Console", 80), "Console");
        assert_eq!(safe_name("LPT10", 80), "LPT10");
    }

    #[test]
    fn kuerzen() {
        let lang = "x".repeat(500);
        assert_eq!(safe_name(&lang, 150).len(), 150);
        // Gekürzt wird nie mitten in einem Zeichen; Emojis zählen als zwei UTF-16-Zeichen
        assert_eq!(safe_name("ab😀cd", 3), "ab");
        assert_eq!(safe_name("ab😀cd", 4), "ab😀");
        // Kein Leerzeichen/Punkt am Ende nach dem Kürzen
        assert_eq!(safe_name("abc defg", 4), "abc");
        // Pfadlänge: kurzer Ordner -> 150, langer Ordner -> weniger, nie unter 20
        assert_eq!(max_stem_len(&[r"C:\Ablage"]), 150);
        let tief = format!(r"C:\{}", "o".repeat(150));
        assert_eq!(max_stem_len(&[r"C:\Ablage".to_string(), tief.clone()]), 259 - tief.len() - 10);
        assert_eq!(max_stem_len(&[format!(r"C:\{}", "o".repeat(300))]), 20);
        assert_eq!(max_stem_len::<&str>(&[]), 150);
        // Ordner + Name + Nummer + Endung bleiben unter 260 Zeichen
        let name = safe_name(&"y".repeat(400), max_stem_len(&[tief.as_str()]));
        assert!(format!(r"{tief}\{name}_9999.pdf").len() <= 259);
    }

    fn teile() -> NameParts {
        NameParts {
            titel: "Rechnung".into(),
            datum: "2026-11-15".into(),
            uhrzeit: "21-55-03".into(),
            programm: "Word".into(),
        }
    }

    #[test]
    fn vorlage() {
        let t = teile();
        assert_eq!(render_pattern("{datum}_{uhrzeit}_{titel}.pdf", &t), "2026-11-15_21-55-03_Rechnung");
        assert_eq!(render_pattern("{datum}_{uhrzeit}_{titel}", &t), "2026-11-15_21-55-03_Rechnung");
        // Älterer Platzhalter {zeit} und Groß-/Kleinschreibung
        assert_eq!(render_pattern("{datum}_{zeit}_{TITEL}.PDF", &t), "2026-11-15_21-55-03_Rechnung");
        assert_eq!(render_pattern("{programm} - {titel}", &t), "Word - Rechnung");
        assert_eq!(render_pattern("Beleg {titel} {titel}", &t), "Beleg Rechnung Rechnung");
        // Unbekannte Platzhalter und einzelne Klammern bleiben stehen
        assert_eq!(render_pattern("{foo}_{titel}", &t), "{foo}_Rechnung");
        assert_eq!(render_pattern("a{b_{titel}", &t), "a{b_Rechnung");
        // Werte werden nicht noch einmal ersetzt
        let mut t2 = teile();
        t2.titel = "{datum}".into();
        assert_eq!(render_pattern("{titel}", &t2), "{datum}");
    }

    #[test]
    fn vorlage_leerer_platzhalter() {
        let mut t = teile();
        t.programm.clear();
        assert_eq!(render_pattern("{datum}_{programm}_{titel}", &t), "2026-11-15_Rechnung");
        assert_eq!(render_pattern("{programm}_{titel}", &t), "Rechnung");
        assert_eq!(render_pattern("{titel}_{programm}", &t), "Rechnung");
        assert_eq!(render_pattern("{titel} - {programm}.pdf", &t), "Rechnung");
        // Gewollte Trennzeichen ohne leeren Platzhalter bleiben
        assert_eq!(render_pattern("{titel}__{datum}", &t), "Rechnung__2026-11-15");
    }

    #[test]
    fn vorlage_und_bereinigung_zusammen() {
        let mut t = teile();
        t.titel = clean_document_name("Microsoft Word - Angebot: Müller/Schmidt?.docx");
        t.programm = program_from_document("Microsoft Word - Angebot.docx").unwrap();
        let stem = safe_name(&render_pattern(crate_default(), &t), 150);
        assert_eq!(stem, "2026-11-15_21-55-03_Angebot_ Müller_Schmidt_");
        assert!(!stem.chars().any(|c| "<>:\"/\\|?*".contains(c)));
    }

    fn crate_default() -> &'static str {
        "{datum}_{uhrzeit}_{titel}.pdf"
    }

    #[test]
    fn vorlage_pruefen() {
        assert_eq!(pattern_problem("{datum}_{uhrzeit}_{titel}.pdf"), None);
        assert_eq!(pattern_problem("{datum}_{zeit}_{titel}"), None);
        assert_eq!(pattern_problem("Rechnung"), None);
        assert_eq!(pattern_problem("{Programm}-{TITEL}"), None);
        assert!(pattern_problem("").is_some());
        assert!(pattern_problem(".pdf").is_some());
        assert!(pattern_problem("{datum}_{name}").unwrap().contains("{name}"));
    }

    #[test]
    fn programme() {
        assert_eq!(program_from_document("Microsoft Excel - Liste.xlsx").as_deref(), Some("Excel"));
        assert_eq!(program_from_document("Rechnung 4711"), None);
        assert_eq!(program_from_exe(r"C:\Program Files\Microsoft Office\root\Office16\WINWORD.EXE").as_deref(), Some("Word"));
        assert_eq!(program_from_exe("msedge.exe").as_deref(), Some("Edge"));
        assert_eq!(program_from_exe(r"C:\Lexware\Faktura.exe").as_deref(), Some("Faktura"));
        assert_eq!(program_from_exe(r"C:\Windows\explorer.exe"), None);
        assert_eq!(program_from_exe("PDF-Ablage.exe"), None);
        assert_eq!(program_from_exe(""), None);
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
    fn druckerpruefung() {
        let pipe = r"\\.\pipe\PDFAblage";
        let gut = || Some(PrinterState { port: pipe.into(), driver: "Microsoft Print To PDF".into() });
        let namen = vec!["PDF-Ablage".to_string(), "PDF-Ablage 2".to_string()];
        let ports = vec!["LPT1:".to_string(), "PORTPROMPT:".to_string(), pipe.to_lowercase()];
        let treiber = vec!["Microsoft XPS Document Writer v4".to_string(), "Microsoft Print To PDF".to_string()];
        let alt = |p: &str| p.ends_with("eingang.pdf");

        // Alles in Ordnung
        assert!(diagnose(&namen, pipe, alt, |_| gut(), Some(&ports), Some(&treiber)).is_empty());
        // Listen nicht lesbar -> nur Drucker prüfen
        assert!(diagnose(&namen, pipe, alt, |_| gut(), None, None).is_empty());

        // Nach einem Update: Drucker 2 weg, Drucker 1 auf Speichern-Dialog umgestellt
        let p = diagnose(
            &namen,
            pipe,
            alt,
            |n| (n == "PDF-Ablage").then(|| PrinterState { port: "PORTPROMPT:".into(), driver: "Microsoft Print To PDF".into() }),
            Some(&ports),
            Some(&treiber),
        );
        assert_eq!(
            p,
            vec![
                Problem::WrongPort { printer: "PDF-Ablage".into(), port: "PORTPROMPT:".into() },
                Problem::PrinterMissing("PDF-Ablage 2".into()),
            ]
        );
        assert!(p[0].to_string().contains("PORTPROMPT:"));

        // Anschluss und Treiber entfernt, Drucker mit fremdem Treiber
        let p = diagnose(
            &namen[..1],
            pipe,
            alt,
            |_| Some(PrinterState { port: pipe.into(), driver: "Microsoft XPS Document Writer v4".into() }),
            Some(&ports[..2]),
            Some(&treiber[..1]),
        );
        assert_eq!(
            p,
            vec![
                Problem::DriverMissing,
                Problem::PortMissing,
                Problem::WrongDriver { printer: "PDF-Ablage".into(), driver: "Microsoft XPS Document Writer v4".into() },
            ]
        );

        // Anschluss einer älteren Version
        let p = diagnose(
            &namen[..1],
            pipe,
            alt,
            |_| Some(PrinterState { port: r"C:\ProgramData\PDFDrucker\spool\eingang.pdf".into(), driver: "Microsoft Print to PDF".into() }),
            None,
            None,
        );
        assert_eq!(p, vec![Problem::OldPort("PDF-Ablage".into())]);

        assert!(is_pdf_driver("Microsoft Print To PDF"));
        assert!(!is_pdf_driver("PDF Printer Print"));
        assert!(!is_pdf_driver("Microsoft XPS Document Writer"));
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
    fn nie_ueberschreiben() {
        let dir = std::env::temp_dir().join(format!("pdfablage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ablegen = |inhalt: &[u8]| {
            let part = dir.join("x.pdf.part");
            std::fs::write(&part, inhalt).unwrap();
            let ziel = place_file(&part, &dir, "x").unwrap();
            assert!(!part.exists(), "Zwischendatei bleibt liegen");
            ziel
        };
        let a = ablegen(b"eins");
        let b = ablegen(b"zwei");
        let c = ablegen(b"drei");
        assert_eq!(a.file_name().unwrap(), "x.pdf");
        assert_eq!(b.file_name().unwrap(), "x_2.pdf");
        assert_eq!(c.file_name().unwrap(), "x_3.pdf");
        assert_eq!(std::fs::read(&a).unwrap(), b"eins");
        assert_eq!(std::fs::read(&b).unwrap(), b"zwei");
        // Lücke wird genutzt, vorhandene Dateien bleiben unangetastet
        std::fs::remove_file(&b).unwrap();
        assert_eq!(ablegen(b"vier").file_name().unwrap(), "x_2.pdf");
        assert_eq!(std::fs::read(&c).unwrap(), b"drei");
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
