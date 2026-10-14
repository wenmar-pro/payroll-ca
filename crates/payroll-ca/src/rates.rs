//! Rate tables for one T4127 edition, and the lookup from a pay date to its edition.
//!
//! Values for each edition live in their own module (`rates/ed123.rs`). Nothing
//! falls back to an older table: a date outside the edition's validity is an error.

use std::sync::OnceLock;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::{RoundingStrategy, dec};

use crate::{Error, Province};

pub mod ed123;

/// One bracket of Table 8.1: the lower threshold of A, the rate (R or V) and the
/// constant (K or KP) that makes `rate × A − constant` the tax on the whole of A.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bracket {
    pub threshold: Decimal,
    pub rate: Decimal,
    pub constant: Decimal,
}

/// Ontario surtax (V1) on the basic provincial tax T4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surtax {
    pub first_threshold: Decimal,
    pub first_rate: Decimal,
    pub second_threshold: Decimal,
    pub second_rate: Decimal,
}

/// Provincial tax reduction S (T4127 Step 5). `Y` (dependants) is not an input
/// and is treated as zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reduction {
    None,
    /// Ontario: S = lesser of (T4 + V1) and 2 × (S2 + Y) − (T4 + V1), not below zero.
    Ontario {
        s2: Decimal,
    },
    /// British Columbia (July 2026 text): S2 up to `phase_start` of A, then reduced by
    /// `phase_rate` × (A − phase_start), nothing above `phase_end`.
    BritishColumbia {
        s2: Decimal,
        phase_start: Decimal,
        phase_end: Decimal,
        phase_rate: Decimal,
    },
}

/// Alberta supplemental credit K5P: ((K1P + K2P) − threshold) × rate, not below zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supplemental {
    pub threshold: Decimal,
    pub rate: Decimal,
}

/// One band of the Ontario Health Premium V2: V2 = min(cap, base + rate × (A − lower)),
/// for lower < A ≤ upper (no upper bound on the last band).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OhpBand {
    pub lower: Decimal,
    pub upper: Option<Decimal>,
    pub base: Decimal,
    pub rate: Decimal,
    pub cap: Decimal,
}

/// The tax tables of one jurisdiction (federal, or one province or territory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jurisdiction {
    /// Table 8.1 brackets, ascending. The first bracket's rate is the lowest rate.
    pub brackets: Vec<Bracket>,
    /// Basic personal amount (Table 8.2) used when no TD1 is on file. Informational
    /// for callers; the calculation uses `Pay::federal_claim` / `provincial_claim`.
    pub basic_amount: Decimal,
    /// Canada employment amount (CEA), where the jurisdiction has one (federal, Yukon).
    pub cea: Option<Decimal>,
    pub reduction: Reduction,
    pub surtax: Option<Surtax>,
    pub supplemental: Option<Supplemental>,
}

impl Jurisdiction {
    /// The lowest rate of the jurisdiction (used for K1, K2, K4 and K1P/K2P).
    pub fn lowest_rate(&self) -> Decimal {
        self.brackets[0].rate
    }

    /// The bracket in which the annual taxable income `a` falls.
    pub fn bracket_for(&self, a: Decimal) -> &Bracket {
        self.brackets
            .iter()
            .rev()
            .find(|b| a >= b.threshold)
            .unwrap_or(&self.brackets[0])
    }
}

/// Table 8.3 / 8.4 / 8.5 / 8.6: CPP and the second additional contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppRates {
    /// YMPE: Year's Maximum Pensionable Earnings.
    pub ympe: Decimal,
    /// YAMPE: Year's Additional Maximum Pensionable Earnings.
    pub yampe: Decimal,
    /// Annual basic exemption (3,500.00), divided by the pay periods.
    pub basic_exemption: Decimal,
    /// Base rate (0.0495) and total base + first additional rate (0.0595).
    pub base_rate: Decimal,
    pub total_rate: Decimal,
    /// Maximum base contribution (3,519.45) and maximum total base + first additional
    /// employee contribution (4,230.45), both per full year.
    pub max_base: Decimal,
    pub max_total: Decimal,
    /// Second additional rate (0.04) and its annual maximum (416.00).
    pub second_rate: Decimal,
    pub max_second: Decimal,
}

/// Table 8.7: Employment Insurance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EiRates {
    /// Maximum annual insurable earnings (68,900.00).
    pub max_insurable: Decimal,
    /// Employee rate (0.0163) and maximum annual employee premium (1,123.07).
    pub rate: Decimal,
    pub max_employee: Decimal,
    /// Employer premium = this multiple of the employee premium (1.4).
    pub employer_multiplier: Decimal,
}

/// A complete rate set for one T4127 edition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateSet {
    pub edition: u32,
    pub effective: NaiveDate,
    pub valid_until: NaiveDate,
    pub federal: Jurisdiction,
    pub provinces: Vec<(Province, Jurisdiction)>,
    pub cpp: CppRates,
    pub ei: EiRates,
    /// Ontario Health Premium bands (V2), ascending by `lower`.
    pub ohp: Vec<OhpBand>,
}

impl RateSet {
    /// The tables for a provincial or territorial jurisdiction.
    pub fn province(&self, province: Province) -> &Jurisdiction {
        self.provinces
            .iter()
            .find(|(p, _)| *p == province)
            .map(|(_, j)| j)
            .expect("every Province has a jurisdiction in each rate set")
    }
}

/// Every edition the crate holds, oldest first.
pub fn editions() -> &'static [RateSet] {
    static ALL: OnceLock<Vec<RateSet>> = OnceLock::new();
    ALL.get_or_init(|| vec![ed123::rate_set()])
}

/// The rate set in force on `date`. Errors (never falls back) when no edition covers it.
pub fn rate_set_for(date: NaiveDate) -> Result<&'static RateSet, Error> {
    let all = editions();
    all.iter()
        .find(|set| set.effective <= date && date <= set.valid_until)
        .ok_or(Error::DateOutOfRange {
            date,
            effective: all[0].effective,
            valid_until: all[all.len() - 1].valid_until,
        })
}

/// Federal basic personal amount formula (BPAF) at net income `ni` (A + HD), for use
/// when no TD1 is on file. The maximum amount is 16,452 for 2026.
pub fn federal_basic_personal_amount(ni: Decimal) -> Decimal {
    let max = dec!(16452);
    let min = dec!(14829);
    let amount = if ni <= dec!(181440) {
        max
    } else if ni < dec!(258482) {
        max - (ni - dec!(181440)) * (dec!(1623) / dec!(77042))
    } else {
        min
    };
    to_cents(amount)
}

fn to_cents(amount: Decimal) -> Decimal {
    amount.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Manitoba basic personal amount formula (BPAMB) at net income `ni`, for use when no
/// TD1MB is on file. The maximum for 2026 is 15,780.
pub fn manitoba_basic_personal_amount(ni: Decimal) -> Decimal {
    let max = dec!(15780);
    let amount = if ni <= dec!(200000) {
        max
    } else if ni < dec!(400000) {
        max - (ni - dec!(200000)) * (max / dec!(200000))
    } else {
        Decimal::ZERO
    };
    to_cents(amount)
}
