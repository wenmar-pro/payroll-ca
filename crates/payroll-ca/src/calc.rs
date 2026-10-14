//! The T4127 Option 1 formulas. Each named factor is rounded to cents where the text
//! and its worked examples round it (CPP, EI, K's, F5, A, T3, T4, V1, V2, S, T2);
//! rate ratios and products inside a formula stay unrounded.

use chrono::Datelike;
use rust_decimal::{Decimal, RoundingStrategy, dec};

use crate::rates::{Jurisdiction, OhpBand, RateSet, Reduction};
use crate::{Deductions, Error, Pay, Province};

/// Round half up to cents (T4127 Chapter 1).
fn cents(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Drop digits past the cent (the CPP basic exemption per pay period).
fn truncate_cents(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

/// PM: months in the pay's calendar year during which CPP is deducted, and whether the
/// pay's own month is one of them. Without a birth date CPP applies all year.
///
/// Starts with the first pay dated in the month after the 18th birthday; stops after the
/// last pay dated in the month of the 70th birthday (T4127 Chapter 6, "Special CPP
/// situations"). Only months matter, so the day of the birth date does not.
fn contribution_months(pay: &Pay) -> (u32, bool) {
    let index = |year: i32, month0: u32| year * 12 + month0 as i32;
    match pay.birth_date {
        None => (12, true),
        Some(birth) => {
            let start = index(birth.year() + 18, birth.month0()) + 1;
            let end = index(birth.year() + 70, birth.month0());
            let year = pay.pay_date.year();
            let pm = (0..12u32)
                .filter(|m| {
                    let i = index(year, *m);
                    i >= start && i <= end
                })
                .count() as u32;
            let pay_index = index(year, pay.pay_date.month0());
            (pm, pay_index >= start && pay_index <= end)
        }
    }
}

fn validate(pay: &Pay) -> Result<(), Error> {
    if !(1..=366).contains(&pay.periods_per_year) {
        return Err(Error::InvalidPay(
            "periods_per_year must be from 1 to 366".into(),
        ));
    }
    let amounts = [
        ("gross_taxable", pay.gross_taxable),
        ("pensionable", pay.pensionable),
        ("insurable", pay.insurable),
        ("federal_claim", pay.federal_claim),
        ("provincial_claim", pay.provincial_claim),
        ("ytd_cpp_base_and_first", pay.ytd_cpp_base_and_first),
        ("ytd_cpp2", pay.ytd_cpp2),
        ("ytd_ei", pay.ytd_ei),
        ("ytd_pensionable", pay.ytd_pensionable),
        ("additional_tax", pay.additional_tax),
    ];
    for (name, value) in amounts {
        if value < Decimal::ZERO {
            return Err(Error::InvalidPay(format!("{name} must not be negative")));
        }
    }
    Ok(())
}

/// Ontario Health Premium V2 on annual taxable income `a`.
fn ontario_health_premium(bands: &[OhpBand], a: Decimal) -> Decimal {
    for band in bands {
        let above = a > band.lower;
        let within = band.upper.is_none_or(|upper| a <= upper);
        if above && within {
            return (band.base + band.rate * (a - band.lower)).min(band.cap);
        }
    }
    Decimal::ZERO
}

/// Named factors, in the order they are computed.
#[derive(Default)]
struct Factors(Vec<(String, Decimal)>);

impl Factors {
    fn put(&mut self, name: &str, value: Decimal) -> Decimal {
        self.0.push((name.to_string(), value));
        value
    }
}

pub(crate) fn run(rates: &RateSet, pay: &Pay) -> Result<Deductions, Error> {
    validate(pay)?;
    let jur_p: &Jurisdiction = rates.province(pay.province);
    let mut f = Factors::default();

    let p = Decimal::from(pay.periods_per_year);
    let (pm, in_window) = contribution_months(pay);
    f.put("P", p);
    f.put("PM", Decimal::from(pm));
    let pm_ratio = Decimal::from(pm) / dec!(12);

    // Prorated annual CPP maxima for this year's PM.
    let cpp = &rates.cpp;
    let max_total = cpp.max_total * pm_ratio;
    let max_base = cpp.max_base * pm_ratio;
    let max_second = cpp.max_second * pm_ratio;
    let ympe_prorated = cpp.ympe * pm_ratio;
    let ei_max = rates.ei.max_employee;

    // C: CPP base and first additional for the pay (T4127 Chapter 6).
    let c = if pay.cpp_exempt || !in_window {
        Decimal::ZERO
    } else {
        let exemption = truncate_cents(cpp.basic_exemption / p);
        let limit_by_max = max_total - pay.ytd_cpp_base_and_first;
        let limit_by_pay = cpp.total_rate * (pay.pensionable - exemption);
        cents(limit_by_max.min(limit_by_pay).max(Decimal::ZERO))
    };
    f.put("C", c);

    // C2: second additional CPP (YMPE to YAMPE band, capped at 416 per year).
    let c2 = if pay.cpp_exempt || !in_window {
        Decimal::ZERO
    } else {
        let w = pay.ytd_pensionable.max(ympe_prorated);
        let limit_by_max = max_second - pay.ytd_cpp2;
        let limit_by_pay = (pay.ytd_pensionable + pay.pensionable - w) * cpp.second_rate;
        cents(limit_by_max.min(limit_by_pay).max(Decimal::ZERO))
    };
    f.put("C2", c2);

    // EI premium (Chapter 7).
    let ei = if pay.ei_exempt {
        Decimal::ZERO
    } else {
        let by_max = ei_max - pay.ytd_ei;
        let by_pay = rates.ei.rate * pay.insurable;
        cents(by_max.min(by_pay).max(Decimal::ZERO))
    };
    f.put("EI", ei);
    let employer_ei = cents(ei * rates.ei.employer_multiplier);

    // F5 and F5A: CPP additional contributions deductible from taxable income. B (bonus)
    // is not an input, so F5A = F5 × (PI − 0) / PI.
    let f5 = cents(c * (dec!(0.01) / cpp.total_rate) + c2);
    f.put("F5", f5);
    let f5a = if pay.pensionable > Decimal::ZERO {
        f5
    } else {
        Decimal::ZERO
    };
    f.put("F5A", f5a);

    // Step 1: annual taxable income A = P × (I − F5A) (F, F1, F2, U1, HD are not inputs).
    let a = cents(p * (pay.gross_taxable - f5a));
    f.put("A", a);

    let (federal_tax, provincial_tax_t2) = if a < Decimal::ZERO {
        // T4127: if A is negative, T = L.
        (Decimal::ZERO, Decimal::ZERO)
    } else {
        federal_and_provincial(
            rates,
            pay,
            jur_p,
            &mut f,
            RunValues {
                p,
                a,
                c,
                ei,
                max_base,
                cpp_at_max: c + pay.ytd_cpp_base_and_first >= max_total,
                ei_at_max: ei + pay.ytd_ei >= ei_max,
            },
        )
    };

    // Step 6: T = (T1 + T2) / P + L. Federal and provincial are each rounded to cents
    // and T is their sum, as PDOC shows them (rounding the total once can differ by a cent).
    let federal_per_pay = cents(federal_tax / p);
    let provincial_per_pay = cents(provincial_tax_t2 / p);
    let total_tax = federal_per_pay + provincial_per_pay + pay.additional_tax;
    f.put("L", pay.additional_tax);
    f.put("T", total_tax);

    Ok(Deductions {
        federal_tax: federal_per_pay,
        provincial_tax: provincial_per_pay,
        total_tax,
        cpp: c,
        cpp2: c2,
        ei,
        employer_cpp: c,
        employer_cpp2: c2,
        employer_ei,
        factors: f.0,
    })
}

/// Values shared by the federal and provincial steps.
struct RunValues {
    p: Decimal,
    a: Decimal,
    /// C and the employee EI premium for the pay (the K2 credit base).
    c: Decimal,
    ei: Decimal,
    /// The base CPP maximum prorated for PM.
    max_base: Decimal,
    /// Whether this pay reaches (or follows) the year's CPP or EI maximum.
    cpp_at_max: bool,
    ei_at_max: bool,
}

/// Steps 2 to 5: returns (T1, T2), the annual federal and provincial tax deductions.
fn federal_and_provincial(
    rates: &RateSet,
    pay: &Pay,
    jur_p: &Jurisdiction,
    f: &mut Factors,
    v: RunValues,
) -> (Decimal, Decimal) {
    let RunValues {
        p,
        a,
        c,
        ei,
        max_base,
        cpp_at_max,
        ei_at_max,
    } = v;
    let cpp = &rates.cpp;
    let ei_max = rates.ei.max_employee;
    // K2 base-CPP term: P × C × (base / total), capped at the prorated base maximum.
    // From the pay that reaches the year's maximum on, the term is the maximum itself
    // (T4127 Chapter 4 note after F5, followed by PDOC); likewise P × EI.
    let base_cpp_annual = if cpp_at_max {
        max_base
    } else {
        (p * c * (cpp.base_rate / cpp.total_rate)).min(max_base)
    };
    let ei_annual = if ei_at_max {
        ei_max
    } else {
        (p * ei).min(ei_max)
    };
    let gross_annual = p * pay.gross_taxable;

    // Step 2: federal T3.
    let fed = &rates.federal;
    let low_f = fed.lowest_rate();
    let fb = fed.bracket_for(a);
    f.put("R", fb.rate);
    f.put("K", fb.constant);
    let k1 = f.put("K1", cents(low_f * pay.federal_claim));
    let k2 = f.put("K2", cents(low_f * base_cpp_annual + low_f * ei_annual));
    let k3 = Decimal::ZERO;
    let cea_f = fed.cea.unwrap_or(Decimal::ZERO);
    let k4 = f.put("K4", cents((low_f * gross_annual).min(low_f * cea_f)));
    let t3 = f.put(
        "T3",
        cents((fb.rate * a - fb.constant - k1 - k2 - k3 - k4).max(Decimal::ZERO)),
    );
    let t1 = f.put("T1", t3); // LCF is not an input, so T1 = T3 − P × 0.

    // Steps 4 and 5: provincial T4, V1, V2, S, T2.
    let low_p = jur_p.lowest_rate();
    let pb = jur_p.bracket_for(a);
    f.put("V", pb.rate);
    f.put("KP", pb.constant);
    let k1p = f.put("K1P", cents(low_p * pay.provincial_claim));
    let k2p = f.put("K2P", cents(low_p * base_cpp_annual + low_p * ei_annual));
    let k4p = match jur_p.cea {
        Some(cea) => f.put("K4P", cents((low_p * gross_annual).min(low_p * cea))),
        None => Decimal::ZERO,
    };
    let k5p = match &jur_p.supplemental {
        Some(s) => f.put(
            "K5P",
            cents(((k1p + k2p - s.threshold) * s.rate).max(Decimal::ZERO)),
        ),
        None => Decimal::ZERO,
    };
    let t4 = f.put(
        "T4",
        cents((pb.rate * a - pb.constant - k1p - k2p - k4p - k5p).max(Decimal::ZERO)),
    );

    let v1 = match &jur_p.surtax {
        Some(s) if t4 > s.first_threshold => {
            let mut v1 = s.first_rate * (t4 - s.first_threshold);
            if t4 > s.second_threshold {
                v1 += s.second_rate * (t4 - s.second_threshold);
            }
            f.put("V1", cents(v1))
        }
        Some(_) => f.put("V1", Decimal::ZERO),
        None => Decimal::ZERO,
    };

    // V2 is the Ontario Health Premium; it is the only province with that item.
    let v2 = if pay.province == Province::ON {
        f.put("V2", cents(ontario_health_premium(&rates.ohp, a)))
    } else {
        Decimal::ZERO
    };

    let s = match &jur_p.reduction {
        Reduction::None => Decimal::ZERO,
        Reduction::Ontario { s2 } => {
            // Y (dependants) is not an input and is zero.
            let x = t4 + v1;
            f.put("S", cents(x.min(dec!(2) * *s2 - x).max(Decimal::ZERO)))
        }
        Reduction::BritishColumbia {
            s2,
            phase_start,
            phase_end,
            phase_rate,
        } => {
            let raw = if a <= *phase_start {
                t4.min(*s2)
            } else if a <= *phase_end {
                t4.min(*s2 - (a - *phase_start) * *phase_rate)
            } else {
                Decimal::ZERO
            };
            f.put("S", cents(raw.max(Decimal::ZERO)))
        }
    };

    // LCP (labour-sponsored credit) is not an input, so it is zero.
    let t2 = f.put("T2", cents((t4 + v1 + v2 - s).max(Decimal::ZERO)));
    (t1, t2)
}
