//! Checks the engine against CRA's T4032 payroll deductions tables (`cases/t4032/`).
//!
//! A table row holds the tax for an income range under claim codes 0 to 10. CRA computes
//! it at the middle of the range, with CPP and EI for that pay and nothing year-to-date,
//! and rounds it to the nearest 5 cents; so does this test. The tables effective
//! 2026-01-01 are checked at a July 2026 pay date: the 123rd edition changed only BC, NL
//! and PE, so their federal and Ontario figures are the same.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::NaiveDate;
use payroll_ca::{Pay, Province, calculate};
use rust_decimal::{Decimal, RoundingStrategy, dec};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cases/t4032")
}

/// Claim amounts by (province, table, claim code).
fn claims() -> HashMap<(String, String, usize), Decimal> {
    let mut claims = HashMap::new();
    for entry in fs::read_dir(dir()).expect("cases/t4032 exists") {
        let path = entry.expect("entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("claims-") {
            continue;
        }
        for line in fs::read_to_string(&path).expect("readable").lines().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            claims.insert(
                (
                    f[0].to_string(),
                    f[1].to_string(),
                    f[2].parse().expect("code"),
                ),
                f[3].parse().expect("amount"),
            );
        }
    }
    claims
}

fn nickel(x: Decimal) -> Decimal {
    (x * dec!(20)).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero) / dec!(20)
}

#[test]
#[ignore = "the tables are built on annual CPP/CPP2 amounts; see cases/t4032/README.md"]
fn every_t4032_row_matches_the_engine() {
    let claims = claims();
    let mut checked = 0;
    let mut failures = Vec::new();
    let mut files: Vec<PathBuf> = fs::read_dir(dir())
        .expect("cases/t4032 exists")
        .map(|e| e.expect("entry").path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with(".csv") && !name.starts_with("claims-")
        })
        .collect();
    files.sort();
    for path in files {
        let text = fs::read_to_string(&path).expect("readable");
        for (n, line) in text.lines().enumerate().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            let province = Province::parse(f[0]).expect("province");
            let periods: u32 = f[1].parse().expect("periods");
            let table = f[2];
            let from: Decimal = f[3].parse().expect("from");
            let to: Decimal = f[4].parse().expect("to");
            let pay = (from + to) / dec!(2);
            for code in 0..=10usize {
                let expected: Decimal = f[5 + code].parse().expect("amount");
                let claim = |t: &str| claims[&(f[0].to_string(), t.to_string(), code)];
                let d = calculate(&Pay {
                    pay_date: NaiveDate::from_ymd_opt(2026, 7, 15).expect("date"),
                    province,
                    periods_per_year: periods,
                    gross_taxable: pay,
                    pensionable: pay,
                    insurable: pay,
                    federal_claim: claim("federal"),
                    provincial_claim: claim("provincial"),
                    ytd_cpp_base_and_first: Decimal::ZERO,
                    ytd_cpp2: Decimal::ZERO,
                    ytd_ei: Decimal::ZERO,
                    ytd_pensionable: Decimal::ZERO,
                    cpp_exempt: false,
                    ei_exempt: false,
                    birth_date: None,
                    additional_tax: Decimal::ZERO,
                })
                .expect("calculates");
                let got = nickel(if table == "federal" {
                    d.federal_tax
                } else {
                    d.provincial_tax
                });
                checked += 1;
                if got != expected {
                    failures.push(format!(
                        "{}:{}: {table} {periods}pp {from}-{to} CC{code}: got {got}, table {expected}",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(checked > 0, "no T4032 rows found");
    assert!(
        failures.is_empty(),
        "{} of {checked} table amounts differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
