//! `cargo xtask pdoc-import`: reads the PDOC result PDFs of the newest edition
//! (`reference/pdoc/<edition>/<province>-<case>.pdf`) and appends a corpus row for each
//! one not yet in `cases/pdoc-<edition>.csv`. Needs `pdftotext` (poppler).

use std::fs;
use std::path::Path;
use std::process::Command;

use payroll_ca::Province;
use payroll_ca::rates::editions;
use rust_decimal::Decimal;

use crate::{CASES, case_of, read_corpus};

/// The corpus header (`cases/README.md`).
const HEADER: &str = "edition,province,pay_date,periods,gross,pensionable,insurable,federal_claim,provincial_claim,ytd_cpp_base_and_first,ytd_cpp2,ytd_ei,ytd_pensionable,cpp_exempt,ei_exempt,birth_date,additional_tax,expected_federal,expected_provincial,expected_cpp,expected_cpp2,expected_ei,source,captured_on";

/// What one PDOC result page says.
#[derive(Debug, PartialEq, Eq)]
struct Page {
    province: Province,
    pay_date: String,
    periods: u32,
    gross: Decimal,
    federal_claim: Decimal,
    provincial_claim: Decimal,
    ytd_pensionable: Decimal,
    ytd_cpp: Decimal,
    ytd_cpp2: Decimal,
    ytd_ei: Decimal,
    federal: Decimal,
    provincial: Decimal,
    cpp: Decimal,
    cpp2: Decimal,
    ei: Decimal,
    saved_on: String,
}

fn province_named(name: &str) -> Result<Province, String> {
    Ok(match name {
        "Alberta" => Province::AB,
        "British Columbia" => Province::BC,
        "Manitoba" => Province::MB,
        "New Brunswick" => Province::NB,
        "Newfoundland and Labrador" => Province::NL,
        "Nova Scotia" => Province::NS,
        "Northwest Territories" => Province::NT,
        "Nunavut" => Province::NU,
        "Ontario" => Province::ON,
        "Prince Edward Island" => Province::PE,
        "Saskatchewan" => Province::SK,
        "Yukon" => Province::YT,
        other => return Err(format!("unknown province {other:?}")),
    })
}

/// The text after `label` on the line that starts with it, when that text starts with a
/// colon or a number (so `Pensionable earnings` does not match `Pensionable earnings for ...`).
fn after<'t>(text: &'t str, label: &str) -> Result<&'t str, String> {
    text.lines()
        .map(str::trim_start)
        .filter_map(|line| line.strip_prefix(label))
        .find(|rest| {
            rest.starts_with(':') || rest.trim_start().starts_with(|c: char| c.is_ascii_digit())
        })
        .map(|rest| rest.trim_start_matches(':').trim())
        .ok_or(format!("no line {label:?}"))
}

/// The `n`th amount after `label` (`12,000.00` → 12000.00).
fn amount(text: &str, label: &str, n: usize) -> Result<Decimal, String> {
    let rest = after(text, label)?;
    let token = rest
        .split_whitespace()
        .filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .nth(n)
        .ok_or(format!("no amount {n} after {label:?}"))?;
    token
        .replace(',', "")
        .parse()
        .map_err(|e| format!("{label:?}: {token:?}: {e}"))
}

fn parse(text: &str) -> Result<Page, String> {
    let periods = after(text, "Pay period frequency")?
        .split_once('(')
        .and_then(|(_, r)| r.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .ok_or("no pay periods")?;
    let first_word = |label| -> Result<String, String> {
        Ok(after(text, label)?
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string())
    };
    Ok(Page {
        province: province_named(after(text, "Province of employment")?)?,
        pay_date: first_word("Date the employee is paid")?,
        periods,
        gross: amount(text, "Salary or wages income", 0)?,
        federal_claim: amount(text, "Federal amount from TD1", 0)?,
        provincial_claim: amount(text, "Provincial amount from TD1", 0)?,
        ytd_pensionable: amount(text, "Pensionable earnings", 0)?,
        ytd_cpp: amount(text, "CPP contributions", 0)?,
        ytd_cpp2: amount(text, "CPP2 contributions", 0)?,
        ytd_ei: amount(text, "EI premiums", 0)?,
        federal: amount(text, "Federal tax deduction", 0)?,
        provincial: amount(text, "Provincial tax deduction", 0)?,
        cpp: amount(text, "CPP deductions", 0)?,
        cpp2: amount(text, "CPP2 deductions", 0)?,
        ei: amount(text, "EI deductions", 0)?,
        saved_on: text
            .split("Saved on")
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .ok_or("no \"Saved on\" date")?
            .to_string(),
    })
}

/// The corpus row for a result captured as `case`. PDOC prints the earnings counted this
/// pay (0.00 once a maximum is reached); the row's pensionable and insurable are the gross.
fn row(edition: u32, case: &str, r: &Page) -> String {
    let exempt = CASES.iter().any(|c| c.id == case && c.cpp_exempt);
    [
        edition.to_string(),
        format!("{:?}", r.province),
        r.pay_date.clone(),
        r.periods.to_string(),
        r.gross.to_string(),
        r.gross.to_string(),
        r.gross.to_string(),
        r.federal_claim.to_string(),
        r.provincial_claim.to_string(),
        r.ytd_cpp.to_string(),
        r.ytd_cpp2.to_string(),
        r.ytd_ei.to_string(),
        r.ytd_pensionable.to_string(),
        exempt.to_string(),
        "false".into(),
        String::new(),
        "0.00".into(),
        r.federal.to_string(),
        r.provincial.to_string(),
        r.cpp.to_string(),
        r.cpp2.to_string(),
        r.ei.to_string(),
        format!("PDOC {}", r.saved_on),
        r.saved_on.clone(),
    ]
    .join(",")
}

/// Checks that a PDF holds the case its file name says.
fn check(file: &str, province: Province, case: &str, r: &Page) -> Result<(), String> {
    if r.province != province {
        return Err(format!("{file}: the PDF is for {:?}", r.province));
    }
    let exempt = CASES.iter().any(|c| c.id == case && c.cpp_exempt);
    if exempt && (r.cpp != Decimal::ZERO || r.cpp2 != Decimal::ZERO) {
        return Err(format!(
            "{file}: CPP was deducted; was CPP marked exempt in PDOC?"
        ));
    }
    let claim = r.federal_claim;
    match case_of(r.periods, r.gross, claim, r.ytd_pensionable, exempt) {
        Some(found) if found == case => Ok(()),
        Some(found) => Err(format!("{file}: these inputs are case {found}, not {case}")),
        None => Err(format!(
            "{file}: the inputs match no case (frequency, gross, claim code or year-to-date)"
        )),
    }
}

pub fn import(root: &Path) -> Result<(), String> {
    let all = editions();
    let latest = &all[all.len() - 1];
    let dir = root.join("reference/pdoc").join(latest.edition.to_string());
    let csv = root.join(format!("cases/pdoc-{}.csv", latest.edition));
    let captured = read_corpus(root)?;

    let mut names: Vec<String> = fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".pdf"))
        .collect();
    names.sort();

    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for name in &names {
        let Some((prov, case)) = name.trim_end_matches(".pdf").split_once('-') else {
            errors.push(format!("{name}: expected <province>-<case>.pdf"));
            continue;
        };
        let (Ok(province), true) = (Province::parse(prov), CASES.iter().any(|c| c.id == case))
        else {
            errors.push(format!("{name}: expected <province>-<case>.pdf"));
            continue;
        };
        let done = captured
            .iter()
            .any(|r| r.edition == latest.edition && r.province == province && r.case == case);
        if done {
            continue;
        }
        let out = Command::new("pdftotext")
            .args(["-layout", &dir.join(name).to_string_lossy(), "-"])
            .output()
            .map_err(|e| format!("pdftotext (poppler) is needed: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let result = parse(&text)
            .map_err(|e| format!("{name}: {e}"))
            .and_then(|r| check(name, province, case, &r).map(|()| r));
        match result {
            Ok(r) => {
                println!("  {name}");
                rows.push(row(latest.edition, case, &r));
            }
            Err(e) => errors.push(e),
        }
    }

    if !rows.is_empty() {
        let mut text = fs::read_to_string(&csv).unwrap_or_else(|_| format!("{HEADER}\n"));
        if !text.ends_with('\n') {
            text.push('\n');
        }
        for r in &rows {
            text.push_str(r);
            text.push('\n');
        }
        fs::write(&csv, text).map_err(|e| format!("{}: {e}", csv.display()))?;
    }
    println!("{} row(s) added to {}", rows.len(), csv.display());
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("Not imported:\n  {}", errors.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    /// The layout `pdftotext -layout` gives for a PDOC salary result (ZZ test inputs).
    const C6: &str = "
                          Payroll Deductions Online Calculator
 Employee's name:                                    ZZ Test
 Pay period frequency:                               Monthly (12 pay periods a year)
 Date the employee is paid:                          2026-07-15 (YYYY-MM-DD)
 Province of employment:                             Ontario
 Federal amount from TD1:                            16,452.00
 Provincial amount from TD1:                         12,989.00
 Salary or wages income                                             12,000.00
 Federal tax deduction                        1,902.96
 Provincial tax deduction                     1,079.71
 CPP deductions                                                        51.45
 CPP2 deductions                                                      376.00
 EI deductions                                                          0.00
 Taxable income for the pay period                                 11,615.35
 Pensionable earnings for the pay period                           12,000.00
 Insurable earnings for the pay period                                  0.00
 Year-to-Date Amounts               Inputted Value       Total for this Record
 Pensionable earnings                    72,000.00            84,000.00
 CPP contributions                        4,179.00             4,230.45 (Maximum)
 CPP2 contributions                           0.00               376.00
 Insurable earnings                      68,900.00            68,900.00 (Maximum)
 EI premiums                              1,123.07             1,123.07 (Maximum)
 Created by the Payroll Deductions Online Calculator   Page 1 of 1   Saved on 2026-10-14
";

    #[test]
    fn a_result_page_becomes_a_corpus_row() {
        let r = parse(C6).expect("parses");
        assert_eq!(r.periods, 12);
        assert_eq!(r.ytd_pensionable, dec!(72000.00));
        assert_eq!(r.federal, dec!(1902.96));
        check("ON-c6.pdf", Province::ON, "c6", &r).expect("is c6");
        assert_eq!(
            row(123, "c6", &r),
            "123,ON,2026-07-15,12,12000.00,12000.00,12000.00,16452.00,12989.00,4179.00,0.00,1123.07,72000.00,false,false,,0.00,1902.96,1079.71,51.45,376.00,0.00,PDOC 2026-10-14,2026-10-14"
        );
    }

    #[test]
    fn a_misnamed_or_mistyped_pdf_is_refused() {
        let r = parse(C6).expect("parses");
        assert!(check("BC-c6.pdf", Province::BC, "c6", &r).is_err());
        assert!(check("ON-c7.pdf", Province::ON, "c7", &r).is_err());
        assert!(check("ON-c8.pdf", Province::ON, "c8", &r).is_err());
    }

    #[test]
    fn rows_have_every_corpus_column() {
        let r = parse(C6).expect("parses");
        assert_eq!(
            row(123, "c6", &r).split(',').count(),
            HEADER.split(',').count()
        );
    }
}
