//! Repository tasks. `cargo xtask pdoc-due` lists the PDOC captures the newest edition
//! still needs, and the inputs to type into PDOC for each.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::Days;
use payroll_ca::Province;
use payroll_ca::rates::{RateSet, editions};
use rust_decimal::{Decimal, dec};

mod pdoc;
mod t4032;

/// Provinces captured in PDOC, in capture order.
const PROVINCES: [Province; 4] = [Province::ON, Province::BC, Province::AB, Province::NS];

/// Year-to-date figures a case enters in PDOC.
#[derive(Clone, Copy)]
enum Ytd {
    /// Every year-to-date field left blank.
    None,
    /// Six earlier months of 12,000.00, so this pay crosses from CPP into CPP2.
    Crossing,
    /// Everything at the edition's annual maximums.
    Maximums,
}

struct Case {
    id: &'static str,
    frequency: &'static str,
    periods: u32,
    gross: Decimal,
    ytd: Ytd,
    cpp_exempt: bool,
    claim_zero: bool,
}

/// The fixed case list of manual plan M28.
const CASES: [Case; 9] = [
    case(
        "c1",
        "Weekly (52)",
        52,
        dec!(800.00),
        Ytd::None,
        false,
        false,
    ),
    case(
        "c2",
        "Weekly (52)",
        52,
        dec!(1500.00),
        Ytd::None,
        false,
        false,
    ),
    case(
        "c3",
        "Biweekly (26)",
        26,
        dec!(2400.00),
        Ytd::None,
        false,
        false,
    ),
    case(
        "c4",
        "Biweekly (26)",
        26,
        dec!(6000.00),
        Ytd::None,
        false,
        false,
    ),
    case(
        "c5",
        "Semi-monthly (24)",
        24,
        dec!(3000.00),
        Ytd::None,
        false,
        false,
    ),
    case(
        "c6",
        "Monthly (12)",
        12,
        dec!(12000.00),
        Ytd::Crossing,
        false,
        false,
    ),
    case(
        "c7",
        "Biweekly (26)",
        26,
        dec!(2400.00),
        Ytd::Maximums,
        false,
        false,
    ),
    case(
        "c8",
        "Biweekly (26)",
        26,
        dec!(2400.00),
        Ytd::None,
        true,
        false,
    ),
    case(
        "c9",
        "Weekly (52)",
        52,
        dec!(800.00),
        Ytd::None,
        false,
        true,
    ),
];

const fn case(
    id: &'static str,
    frequency: &'static str,
    periods: u32,
    gross: Decimal,
    ytd: Ytd,
    cpp_exempt: bool,
    claim_zero: bool,
) -> Case {
    Case {
        id,
        frequency,
        periods,
        gross,
        ytd,
        cpp_exempt,
        claim_zero,
    }
}

/// The case a corpus row was captured for, from its inputs.
pub(crate) fn case_of(
    periods: u32,
    gross: Decimal,
    claim: Decimal,
    ytd_pensionable: Decimal,
    cpp_exempt: bool,
) -> Option<&'static str> {
    let found = CASES.iter().find(|c| {
        c.periods == periods
            && c.gross == gross
            && c.cpp_exempt == cpp_exempt
            && c.claim_zero == claim.is_zero()
            && matches!(c.ytd, Ytd::None) == ytd_pensionable.is_zero()
    });
    found.map(|c| c.id)
}

/// One captured row of `cases/pdoc-*.csv`.
pub(crate) struct Captured {
    edition: u32,
    province: Province,
    case: &'static str,
}

pub(crate) fn read_corpus(root: &Path) -> Result<Vec<Captured>, String> {
    let mut rows = Vec::new();
    let dir = root.join("cases");
    let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("pdoc-") && name.ends_with(".csv")
        })
        .collect();
    files.sort();
    for file in files {
        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| *h == name)
                .ok_or(format!("{}: no column {name}", file.display()))
        };
        let (ed, prov, per, gross, claim, ytdp, exempt) = (
            col("edition")?,
            col("province")?,
            col("periods")?,
            col("gross")?,
            col("federal_claim")?,
            col("ytd_pensionable")?,
            col("cpp_exempt")?,
        );
        for (n, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let f: Vec<&str> = line.split(',').collect();
            let at =
                |what: &str, e: String| format!("{} line {}: {what}: {e}", file.display(), n + 2);
            let field = |i: usize| f.get(i).copied().unwrap_or("");
            let dec = |i: usize| {
                field(i)
                    .parse::<Decimal>()
                    .map_err(|e| at("amount", e.to_string()))
            };
            let edition = field(ed)
                .parse()
                .map_err(|e: std::num::ParseIntError| at("edition", e.to_string()))?;
            let province =
                Province::parse(field(prov)).map_err(|e| at("province", e.to_string()))?;
            let periods = field(per)
                .parse()
                .map_err(|e: std::num::ParseIntError| at("periods", e.to_string()))?;
            let found = case_of(
                periods,
                dec(gross)?,
                dec(claim)?,
                dec(ytdp)?,
                field(exempt) == "true",
            );
            // Rows that match no case (hand calculations, other sources) are not captures.
            if let Some(case) = found {
                rows.push(Captured {
                    edition,
                    province,
                    case,
                });
            }
        }
    }
    Ok(rows)
}

/// Whether every table a case in `province` reads is the same in both editions.
fn same_tables(a: &RateSet, b: &RateSet, province: Province) -> bool {
    a.federal == b.federal
        && a.cpp == b.cpp
        && a.ei == b.ei
        && a.province(province) == b.province(province)
        && (province != Province::ON || a.ohp == b.ohp)
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    /// A row for this edition is in `cases/`.
    Done,
    /// The PDF is saved for this edition; its row is not written yet.
    Transcribe,
    /// Captured for an earlier edition whose tables are the same for this province.
    Unchanged(u32),
    /// Needs a PDOC capture.
    Capture,
}

impl State {
    fn mark(&self) -> String {
        match self {
            State::Done => "ok".into(),
            State::Transcribe => "pdf".into(),
            State::Unchanged(e) => format!("={e}"),
            State::Capture => "--".into(),
        }
    }
}

/// Where one province's capture of one case stands for the newest of `all`.
fn state_of(
    all: &[RateSet],
    captured: &[Captured],
    pdf: bool,
    province: Province,
    case: &str,
) -> State {
    let latest = &all[all.len() - 1];
    let has = |edition: u32| {
        captured
            .iter()
            .any(|r| r.edition == edition && r.province == province && r.case == case)
    };
    if has(latest.edition) {
        State::Done
    } else if pdf {
        State::Transcribe
    } else if let Some(old) = all[..all.len() - 1]
        .iter()
        .rev()
        .find(|old| has(old.edition) && same_tables(old, latest, province))
    {
        State::Unchanged(old.edition)
    } else {
        State::Capture
    }
}

fn money(d: Decimal) -> String {
    let d = d.round_dp(2);
    let whole = d.trunc().abs().to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{grouped}.{:02}", (d.fract().abs() * dec!(100)).trunc())
}

/// The year-to-date fields to type into PDOC for a case under `rates` (blank = leave empty).
fn ytd_inputs(ytd: Ytd, rates: &RateSet) -> Vec<(&'static str, String)> {
    let (pensionable, cpp, cpp2, insurable, ei) = match ytd {
        Ytd::None => return Vec::new(),
        Ytd::Crossing => {
            let earned = dec!(72000);
            let per_month = ((dec!(12000) - rates.cpp.basic_exemption / dec!(12))
                * rates.cpp.total_rate)
                .round_dp(2);
            let insurable = earned.min(rates.ei.max_insurable);
            let ei = (insurable * rates.ei.rate)
                .round_dp(2)
                .min(rates.ei.max_employee);
            (earned, (per_month * dec!(6)).floor(), None, insurable, ei)
        }
        Ytd::Maximums => (
            rates.cpp.yampe,
            rates.cpp.max_total,
            Some(rates.cpp.max_second),
            rates.ei.max_insurable,
            rates.ei.max_employee,
        ),
    };
    vec![
        ("Pensionable earnings", money(pensionable)),
        ("CPP contributions", money(cpp)),
        (
            "Second additional CPP contributions",
            cpp2.map(money).unwrap_or_else(|| "(blank)".into()),
        ),
        ("Insurable earnings", money(insurable)),
        ("EI premiums", money(ei)),
    ]
}

fn pdoc_due(root: &Path, provinces: &[Province]) -> Result<(), String> {
    let all = editions();
    let latest = &all[all.len() - 1];
    let captured = read_corpus(root)?;
    let pdf_dir = root.join("reference/pdoc").join(latest.edition.to_string());
    let pay_date = latest.effective + Days::new(14);

    println!(
        "PDOC captures for edition {} (pay dates {} to {})\n",
        latest.edition, latest.effective, latest.valid_until
    );
    print!("     ");
    for c in &CASES {
        print!(" {:>4}", c.id);
    }
    println!();

    let mut due: Vec<(Province, &Case)> = Vec::new();
    let mut transcribe = 0;
    for &province in provinces {
        print!("  {province:?} ");
        for c in &CASES {
            let pdf = pdf_dir.join(format!("{province:?}-{}.pdf", c.id)).is_file();
            let state = state_of(all, &captured, pdf, province, c.id);
            match state {
                State::Transcribe => transcribe += 1,
                State::Capture => due.push((province, c)),
                State::Done | State::Unchanged(_) => {}
            }
            print!(" {:>4}", state.mark());
        }
        println!();
    }
    println!(
        "\n  ok = row in cases/   pdf = PDF saved, row not written   =N = tables unchanged since edition N   -- = capture\n"
    );

    if transcribe > 0 {
        println!(
            "{transcribe} PDF(s) in {} to write into cases/pdoc-{}.csv.",
            pdf_dir.display(),
            latest.edition
        );
    }
    let stray: BTreeSet<String> = fs::read_dir(root.join("reference/pdoc"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "pdf"))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    if !stray.is_empty() {
        println!(
            "Move these into reference/pdoc/<edition>/: {}",
            stray.into_iter().collect::<Vec<_>>().join(" ")
        );
    }
    if due.is_empty() {
        println!("Nothing to capture.");
        return Ok(());
    }

    println!(
        "{} capture(s) due. Save each as {}/<province>-<case>.pdf.",
        due.len(),
        pdf_dir.display()
    );
    println!(
        "Every capture: Salary, name ZZ Test, paid {pay_date}, nothing else in the income fields.\n"
    );
    for c in CASES
        .iter()
        .filter(|c| due.iter().any(|(_, d)| d.id == c.id))
    {
        let provinces: Vec<String> = due
            .iter()
            .filter(|(_, d)| d.id == c.id)
            .map(|(p, _)| format!("{p:?}"))
            .collect();
        println!(
            "  {} [{}]: {}, gross {}",
            c.id,
            provinces.join(" "),
            c.frequency,
            money(c.gross)
        );
        println!("      claim codes {}", if c.claim_zero { "0" } else { "1" });
        if c.cpp_exempt {
            println!("      CPP: exempt (employee aged 70)");
        }
        for (field, value) in ytd_inputs(c.ytd, latest) {
            println!("      year-to-date {field}: {value}");
        }
    }
    println!(
        "\nThis compares rate tables only. If the edition changed a formula in the T4127 text,\ncapture every case again."
    );
    Ok(())
}

/// Provinces named on the command line, or all of `PROVINCES`.
fn provinces_from(args: &[String]) -> Result<Vec<Province>, String> {
    if args.is_empty() {
        return Ok(PROVINCES.to_vec());
    }
    args.iter()
        .map(|a| {
            let p = Province::parse(a).map_err(|e| e.to_string())?;
            if PROVINCES.contains(&p) {
                Ok(p)
            } else {
                Err(format!(
                    "{p:?} is not in the PDOC case list ({PROVINCES:?})"
                ))
            }
        })
        .collect()
}

const USAGE: &str = "usage: cargo xtask pdoc-due [PROVINCE...] | pdoc-import | t4032-import <pdf-dir> <province> <out.csv>";

fn main() -> ExitCode {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap_or(manifest);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("pdoc-due") => provinces_from(&args[1..]).and_then(|p| pdoc_due(root, &p)),
        Some("pdoc-import") => pdoc::import(root),
        Some("t4032-import") => t4032::import(root, &args[1..]),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_edition(change: impl FnOnce(&mut RateSet)) -> Vec<RateSet> {
        let old = editions()[0].clone();
        let mut new = old.clone();
        new.edition = old.edition + 1;
        change(&mut new);
        vec![old, new]
    }

    fn captured_on(edition: u32) -> Vec<Captured> {
        PROVINCES
            .iter()
            .map(|&province| Captured {
                edition,
                province,
                case: "c1",
            })
            .collect()
    }

    #[test]
    fn unchanged_tables_carry_a_capture_over() {
        let all = next_edition(|_| {});
        let rows = captured_on(all[0].edition);
        assert_eq!(
            state_of(&all, &rows, false, Province::BC, "c1"),
            State::Unchanged(all[0].edition)
        );
    }

    #[test]
    fn a_province_change_is_due_only_in_that_province() {
        let all = next_edition(|set| {
            let bc = set
                .provinces
                .iter_mut()
                .find(|(p, _)| *p == Province::BC)
                .expect("BC");
            bc.1.basic_amount += dec!(100);
        });
        let rows = captured_on(all[0].edition);
        assert_eq!(
            state_of(&all, &rows, false, Province::BC, "c1"),
            State::Capture
        );
        assert_eq!(
            state_of(&all, &rows, false, Province::ON, "c1"),
            State::Unchanged(all[0].edition)
        );
    }

    #[test]
    fn a_federal_cpp_or_ei_change_is_due_everywhere() {
        for change in [
            (|s: &mut RateSet| s.federal.basic_amount += dec!(100)) as fn(&mut RateSet),
            |s| s.cpp.ympe += dec!(100),
            |s| s.ei.rate += dec!(0.0001),
        ] {
            let all = next_edition(change);
            let rows = captured_on(all[0].edition);
            for province in PROVINCES {
                assert_eq!(state_of(&all, &rows, false, province, "c1"), State::Capture);
            }
        }
    }

    #[test]
    fn the_ontario_health_premium_is_ontario_only() {
        let all = next_edition(|set| set.ohp[0].cap += dec!(1));
        let rows = captured_on(all[0].edition);
        assert_eq!(
            state_of(&all, &rows, false, Province::ON, "c1"),
            State::Capture
        );
        assert_eq!(
            state_of(&all, &rows, false, Province::NS, "c1"),
            State::Unchanged(all[0].edition)
        );
    }

    #[test]
    fn a_row_or_a_pdf_for_the_newest_edition_wins() {
        let all = next_edition(|s| s.cpp.ympe += dec!(100));
        let rows = captured_on(all[1].edition);
        assert_eq!(
            state_of(&all, &rows, false, Province::AB, "c1"),
            State::Done
        );
        assert_eq!(
            state_of(&all, &[], true, Province::AB, "c1"),
            State::Transcribe
        );
    }

    #[test]
    fn each_case_is_recognised_from_its_row() {
        let ed = &editions()[0];
        let claim = ed.federal.basic_amount;
        let zero = Decimal::ZERO;
        assert_eq!(case_of(52, dec!(800.00), claim, zero, false), Some("c1"));
        assert_eq!(case_of(52, dec!(800.00), zero, zero, false), Some("c9"));
        assert_eq!(case_of(26, dec!(2400.00), claim, zero, false), Some("c3"));
        assert_eq!(
            case_of(26, dec!(2400.00), claim, ed.cpp.yampe, false),
            Some("c7")
        );
        assert_eq!(case_of(26, dec!(2400.00), claim, zero, true), Some("c8"));
        assert_eq!(
            case_of(12, dec!(12000.00), claim, dec!(72000), false),
            Some("c6")
        );
        assert_eq!(case_of(12, dec!(12000.00), claim, zero, false), None);
    }

    #[test]
    fn year_to_date_inputs_for_the_123rd_edition() {
        let ed = &editions()[0];
        let crossing: Vec<String> = ytd_inputs(Ytd::Crossing, ed)
            .into_iter()
            .map(|(_, v)| v)
            .collect();
        assert_eq!(
            crossing,
            ["72,000.00", "4,179.00", "(blank)", "68,900.00", "1,123.07"]
        );
        let max: Vec<String> = ytd_inputs(Ytd::Maximums, ed)
            .into_iter()
            .map(|(_, v)| v)
            .collect();
        assert_eq!(
            max,
            ["85,000.00", "4,230.45", "416.00", "68,900.00", "1,123.07"]
        );
    }
}
