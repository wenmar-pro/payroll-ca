//! `cargo xtask t4032-import <dir> <province> <out.csv>`: turns CRA's T4032 federal and
//! provincial tax tables (PDFs, one per pay frequency) into rows the `t4032` test checks.
//! Claim amounts come from the T4127 claim-code CSVs in `reference/t4127-<edition>/`.
//! Needs `pdftotext` (poppler).

use std::fs;
use std::path::Path;
use std::process::Command;

use payroll_ca::Province;
use payroll_ca::rates::editions;

/// Table rows: one line per income range, the tax for claim codes 0 to 10.
pub const HEADER: &str =
    "province,periods,table,from,to,cc0,cc1,cc2,cc3,cc4,cc5,cc6,cc7,cc8,cc9,cc10";
/// Claim amounts the tables use for each claim code ("Option 1, TC" of T4127).
pub const CLAIMS_HEADER: &str = "province,table,claim_code,amount";

/// Pay periods named on a table page ("Biweekly (26 pay periods a year)").
fn periods_of(line: &str) -> Option<u32> {
    let (_, rest) = line.split_once('(')?;
    let n = rest.split_whitespace().next()?;
    rest.contains("pay periods a year")
        .then(|| n.parse().ok())?
}

/// An amount as printed in a table (`.35`, `1,416.05`, `3659.30`).
fn cents(token: &str) -> Option<String> {
    let t = token.replace(',', "");
    let (whole, frac) = t.split_once('.')?;
    let whole = if whole.is_empty() { "0" } else { whole };
    (whole.chars().all(|c| c.is_ascii_digit())
        && frac.len() == 2
        && frac.chars().all(|c| c.is_ascii_digit()))
    .then(|| format!("{whole}.{frac}"))
}

/// One table line: `from - to` then one amount per claim code, left to right. A claim
/// code with no amount at the end of the line has no tax (the tables leave it blank).
fn parse_line(line: &str) -> Option<(u32, u32, Vec<String>)> {
    let mut parts = line.split_whitespace();
    let from = parts.next()?.replace(',', "").parse().ok()?;
    if parts.next()? != "-" {
        return None;
    }
    let to = parts.next()?.replace(',', "").parse().ok()?;
    let amounts: Option<Vec<String>> = parts.map(cents).collect();
    let mut amounts = amounts?;
    if amounts.is_empty() || amounts.len() > 11 {
        return None;
    }
    amounts.resize(11, "0.00".into());
    Some((from, to, amounts))
}

/// Which table a page holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Federal,
    Provincial,
}

fn rows_of(text: &str, province: Province) -> Vec<(u32, Table, u32, u32, Vec<String>)> {
    let mut rows = Vec::new();
    let (mut table, mut periods) = (None, None);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Federal tax deductions") {
            table = Some(Table::Federal);
        } else if trimmed.ends_with("provincial tax deductions") {
            table = Some(Table::Provincial);
        } else if let Some(p) = periods_of(trimmed) {
            periods = Some(p);
        } else if let (Some(t), Some(p), Some((from, to, amounts))) =
            (table, periods, parse_line(trimmed))
        {
            rows.push((p, t, from, to, amounts));
        }
    }
    let _ = province;
    rows
}

/// Claim amounts by claim code 0 to 10 ("Option 1, TC" column) from a T4127 claim-code CSV.
fn claim_amounts(csv: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(csv).map_err(|e| format!("{}: {e}", csv.display()))?;
    let mut amounts = Vec::new();
    for line in text.lines().skip(2) {
        // Quoted fields hold thousands separators: split on `,` outside quotes.
        let mut fields = Vec::new();
        let (mut field, mut quoted) = (String::new(), false);
        for ch in line.chars() {
            match ch {
                '"' => quoted = !quoted,
                ',' if !quoted => fields.push(std::mem::take(&mut field)),
                ',' => {}
                c => field.push(c),
            }
        }
        fields.push(field);
        if let Some(tc) = fields.get(3).filter(|_| amounts.len() < 11) {
            let tc: f64 = tc
                .trim()
                .parse()
                .map_err(|e| format!("{}: {tc:?}: {e}", csv.display()))?;
            amounts.push(format!("{tc:.2}"));
        }
    }
    if amounts.len() != 11 {
        return Err(format!("{}: expected claim codes 0 to 10", csv.display()));
    }
    Ok(amounts)
}

pub fn import(root: &Path, args: &[String]) -> Result<(), String> {
    let [dir, province, out] = args else {
        return Err("usage: cargo xtask t4032-import <pdf-dir> <province> <out.csv>".into());
    };
    let province = Province::parse(province).map_err(|e| e.to_string())?;
    let code = format!("{province:?}").to_ascii_lowercase();
    let edition = editions()[editions().len() - 1].edition;
    let cc = root.join(format!("reference/t4127-{edition}"));
    let find = |prefix: &str| -> Result<std::path::PathBuf, String> {
        let mut found: Vec<_> = fs::read_dir(&cc)
            .map_err(|e| format!("{}: {e}", cc.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(prefix))
            })
            .collect();
        found.sort();
        found
            .pop()
            .ok_or(format!("no {prefix}*.csv in {}", cc.display()))
    };
    let federal = claim_amounts(&find("cc-fd-")?)?;
    let provincial = claim_amounts(&find(&format!("cc-{code}-"))?)?;

    let mut pdfs: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{dir}: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("t4032-{code}-")) && n.ends_with(".pdf"))
        })
        .collect();
    pdfs.sort();

    let mut out_text = format!("{HEADER}\n");
    let mut count = 0;
    for pdf in &pdfs {
        let output = Command::new("pdftotext")
            .args(["-layout", &pdf.to_string_lossy(), "-"])
            .output()
            .map_err(|e| format!("pdftotext (poppler) is needed: {e}"))?;
        let text = String::from_utf8_lossy(&output.stdout);
        for (periods, table, from, to, amounts) in rows_of(&text, province) {
            let name = match table {
                Table::Federal => "federal",
                Table::Provincial => "provincial",
            };
            out_text.push_str(&format!(
                "{province:?},{periods},{name},{from},{to},{}\n",
                amounts.join(",")
            ));
            count += 1;
        }
    }
    let claims_path = Path::new(out).with_file_name(format!("claims-{code}.csv"));
    let mut claims = format!("{CLAIMS_HEADER}\n");
    for (name, amounts) in [("federal", &federal), ("provincial", &provincial)] {
        for (claim_code, amount) in amounts.iter().enumerate() {
            claims.push_str(&format!("{province:?},{name},{claim_code},{amount}\n"));
        }
    }
    fs::write(&claims_path, claims).map_err(|e| format!("{}: {e}", claims_path.display()))?;
    fs::write(out, out_text).map_err(|e| format!("{out}: {e}"))?;
    println!(
        "{count} rows from {} PDF(s) written to {out}; claim amounts to {}",
        pdfs.len(),
        claims_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_line_with_blank_claim_codes() {
        let (from, to, amounts) =
            parse_line("739   -   743      88.90           .35").expect("parses");
        assert_eq!((from, to), (739, 743));
        assert_eq!(amounts[..3], ["88.90", "0.35", "0.00"]);
        assert_eq!(amounts.len(), 11);
    }

    #[test]
    fn a_full_table_line() {
        let line = "24308   - 24444     3659.30    3574.05   3564.85   3546.50   3528.10    3509.75   3491.40   3473.00   3454.65   3436.25   3417.90";
        let (from, _, amounts) = parse_line(line).expect("parses");
        assert_eq!(from, 24308);
        assert_eq!(amounts[10], "3417.90");
    }

    #[test]
    fn headings_and_the_first_open_row_are_not_rows() {
        assert!(parse_line("From Less than     CC 0         CC 1").is_none());
        assert!(parse_line("739        * (1)         .00").is_none());
    }

    #[test]
    fn pages_name_their_table_and_frequency() {
        let text = "Federal tax deductions\nEffective January 1, 2026\nBiweekly (26 pay periods a year)\n  739   -   743      88.90           .35\nOntario provincial tax deductions\nEffective January 1, 2026\nWeekly (52 pay periods a year)\n  370   -   372      44.50           .25\n";
        let rows = rows_of(text, Province::ON);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].0 == 26 && rows[0].1 == Table::Federal);
        assert!(rows[1].0 == 52 && rows[1].1 == Table::Provincial);
    }
}
