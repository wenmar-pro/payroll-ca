//! Canadian payroll source deductions by CRA T4127 "Payroll Deductions Formulas",
//! Option 1 (the regular method), for pay dates from 2026-07-01 (123rd edition).
//!
//! Quebec is not supported (`Error::QuebecNotSupported`). Results are computed from
//! the published formulas and are not guaranteed; the employer remains liable to CRA.
//! Money is `rust_decimal::Decimal` throughout; the crate does no I/O.

use std::fmt;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

mod calc;
pub mod rates;

/// A province or territory that the T4127 formulas cover (everything except Quebec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Province {
    AB,
    BC,
    MB,
    NB,
    NL,
    NS,
    NT,
    NU,
    ON,
    PE,
    SK,
    YT,
}

impl Province {
    /// Parse a two-letter code (case-insensitive). `QC` is `QuebecNotSupported`.
    pub fn parse(code: &str) -> Result<Province, Error> {
        let upper = code.trim().to_ascii_uppercase();
        Ok(match upper.as_str() {
            "AB" => Province::AB,
            "BC" => Province::BC,
            "MB" => Province::MB,
            "NB" => Province::NB,
            "NL" => Province::NL,
            "NS" => Province::NS,
            "NT" => Province::NT,
            "NU" => Province::NU,
            "ON" => Province::ON,
            "PE" => Province::PE,
            "SK" => Province::SK,
            "YT" => Province::YT,
            "QC" => return Err(Error::QuebecNotSupported),
            _ => return Err(Error::UnknownProvince(code.to_string())),
        })
    }
}

/// One pay, with the year-to-date figures T4127 needs. Year-to-date values exclude
/// this pay. All amounts are dollars and must not be negative.
#[derive(Debug, Clone, PartialEq)]
pub struct Pay {
    /// Pay date; selects the rate set and the calendar year for CPP months.
    pub pay_date: NaiveDate,
    pub province: Province,
    /// Pay periods in the year (P): 52 or 53, 26 or 27, 24, 12, 1, 10, 13, 22, or any
    /// other number from 1 to 366 that T4127 allows.
    pub periods_per_year: u32,
    /// I: gross taxable remuneration for this pay (no bonuses or retroactive pay).
    pub gross_taxable: Decimal,
    /// PI: pensionable earnings for this pay.
    pub pensionable: Decimal,
    /// IE: insurable earnings for this pay.
    pub insurable: Decimal,
    /// TC: federal total claim amount from Form TD1 (claim code amount or BPAF).
    pub federal_claim: Decimal,
    /// TCP: provincial or territorial total claim amount from the provincial TD1.
    pub provincial_claim: Decimal,
    /// D: CPP base and first additional contributions withheld earlier this year.
    pub ytd_cpp_base_and_first: Decimal,
    /// D2: second additional CPP contributions withheld earlier this year.
    pub ytd_cpp2: Decimal,
    /// D1: EI premiums withheld earlier this year.
    pub ytd_ei: Decimal,
    /// PIYTD: pensionable earnings earlier this year.
    pub ytd_pensionable: Decimal,
    /// Employee is exempt from CPP (no CPP or CPP2 deducted).
    pub cpp_exempt: bool,
    /// Employee is exempt from EI (no EI premium).
    pub ei_exempt: bool,
    /// Date of birth, used for the CPP start (month after age 18) and stop (through the
    /// month of age 70). `None` means CPP applies for the whole year.
    pub birth_date: Option<NaiveDate>,
    /// L: additional tax per pay requested on Form TD1.
    pub additional_tax: Decimal,
}

/// The deductions for one pay. `cpp` is the employee's base plus first additional CPP
/// (C in T4127); `cpp2` is the second additional (C2). `provincial_tax` is the remainder
/// of `total_tax` after `federal_tax`, so the two always add up to `total_tax`.
#[derive(Debug, Clone, PartialEq)]
pub struct Deductions {
    pub federal_tax: Decimal,
    pub provincial_tax: Decimal,
    pub total_tax: Decimal,
    pub cpp: Decimal,
    pub cpp2: Decimal,
    pub ei: Decimal,
    pub employer_cpp: Decimal,
    pub employer_cpp2: Decimal,
    pub employer_ei: Decimal,
    /// Every named T4127 factor used, with the value used (after rounding), in order.
    pub factors: Vec<(String, Decimal)>,
}

/// Errors from parsing a province or calculating a pay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Quebec uses its own provincial tax and QPP; T4127 Step 5 does not cover it.
    QuebecNotSupported,
    UnknownProvince(String),
    /// The pay date is outside the validity of the 123rd edition (2026-07-01 to 2026-12-31).
    DateOutOfRange {
        date: NaiveDate,
        effective: NaiveDate,
        valid_until: NaiveDate,
    },
    InvalidPay(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::QuebecNotSupported => {
                write!(f, "Quebec is not supported (T4127 Option 1 excludes QC)")
            }
            Error::UnknownProvince(code) => write!(f, "unknown province or territory: {code:?}"),
            Error::DateOutOfRange {
                date,
                effective,
                valid_until,
            } => write!(
                f,
                "no rate set covers {date} (123rd edition is valid {effective} to {valid_until})"
            ),
            Error::InvalidPay(reason) => write!(f, "invalid pay: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

/// Calculate the CPP, CPP2, EI and income tax deductions for one pay (T4127 Option 1).
pub fn calculate(pay: &Pay) -> Result<Deductions, Error> {
    let rates = rates::rate_set_for(pay.pay_date)?;
    calc::run(rates, pay)
}
