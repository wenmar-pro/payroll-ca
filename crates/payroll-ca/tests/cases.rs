//! Loads every `cases/*.csv` file at the repository root and checks each row against the
//! crate. The column layout is described in `cases/README.md`.

use std::fs;
use std::path::PathBuf;

use chrono::NaiveDate;
use payroll_ca::{Pay, Province, calculate};
use rust_decimal::Decimal;

const COLUMNS: usize = 24;

struct Expected {
    federal: Decimal,
    provincial: Decimal,
    cpp: Decimal,
    cpp2: Decimal,
    ei: Decimal,
}

fn amount(field: &str) -> Result<Decimal, String> {
    field
        .parse::<Decimal>()
        .map_err(|e| format!("bad amount {field:?}: {e}"))
}

fn flag(field: &str) -> Result<bool, String> {
    match field {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("bad flag {other:?}")),
    }
}

fn parse_row(line: &str) -> Result<(Pay, Expected), String> {
    let f: Vec<&str> = line.split(',').collect();
    if f.len() != COLUMNS {
        return Err(format!("expected {COLUMNS} columns, found {}", f.len()));
    }
    if f[0] != "123" {
        return Err(format!("unsupported edition {:?}", f[0]));
    }
    let date = |s: &str| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("bad date {s:?}: {e}"))
    };
    let pay = Pay {
        pay_date: date(f[2])?,
        province: Province::parse(f[1]).map_err(|e| e.to_string())?,
        periods_per_year: f[3].parse().map_err(|e| format!("bad periods: {e}"))?,
        gross_taxable: amount(f[4])?,
        pensionable: amount(f[5])?,
        insurable: amount(f[6])?,
        federal_claim: amount(f[7])?,
        provincial_claim: amount(f[8])?,
        ytd_cpp_base_and_first: amount(f[9])?,
        ytd_cpp2: amount(f[10])?,
        ytd_ei: amount(f[11])?,
        ytd_pensionable: amount(f[12])?,
        cpp_exempt: flag(f[13])?,
        ei_exempt: flag(f[14])?,
        birth_date: if f[15].is_empty() {
            None
        } else {
            Some(date(f[15])?)
        },
        additional_tax: amount(f[16])?,
    };
    let expected = Expected {
        federal: amount(f[17])?,
        provincial: amount(f[18])?,
        cpp: amount(f[19])?,
        cpp2: amount(f[20])?,
        ei: amount(f[21])?,
    };
    Ok((pay, expected))
}

fn check_row(line: &str) -> Result<(), String> {
    let (pay, expected) = parse_row(line)?;
    let d = calculate(&pay).map_err(|e| e.to_string())?;
    let checks = [
        ("federal", d.federal_tax, expected.federal),
        ("provincial", d.provincial_tax, expected.provincial),
        ("cpp", d.cpp, expected.cpp),
        ("cpp2", d.cpp2, expected.cpp2),
        ("ei", d.ei, expected.ei),
    ];
    for (name, got, want) in checks {
        if got != want {
            return Err(format!("{name}: got {got}, expected {want} (row: {line})"));
        }
    }
    Ok(())
}

fn case_files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cases");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "csv"))
        .collect();
    files.sort();
    files
}

#[test]
fn every_case_row_matches_the_expected_deductions() {
    let mut failures = Vec::new();
    for path in case_files() {
        let text = fs::read_to_string(&path).expect("case file is readable");
        for (number, line) in text.lines().enumerate().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            if let Err(reason) = check_row(line) {
                failures.push(format!("{}:{}: {reason}", path.display(), number + 1));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_case_row_parses_and_checks() {
    // The format is exercised on the Ontario case from tests/ontario_case.rs.
    let row = "123,ON,2026-08-14,26,2400.00,2400.00,2400.00,16452.00,12989.00,0,0,0,0,false,false,,0,223.20,122.74,134.79,0.00,39.12,ontario-case,2026-10-14";
    check_row(row).expect("row matches");
}

#[test]
fn a_wrong_expectation_is_reported() {
    let row = "123,ON,2026-08-14,26,2400.00,2400.00,2400.00,16452.00,12989.00,0,0,0,0,false,false,,0,999.00,122.74,134.79,0.00,39.12,bad,2026-10-14";
    assert!(check_row(row).unwrap_err().contains("federal"));
}
