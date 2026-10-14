//! One Ontario case, worked by hand from the January 2026 text (T4127 Chapters 4 to 7,
//! Table 8.1 and Table 8.2 values for 123rd edition) and checked against the crate.
//!
//! Inputs: Ontario, biweekly (P = 26), gross I = 2,400.00, pensionable = insurable =
//! 2,400.00, no year-to-date amounts, no birth date, claim code 1 (TC = BPAF 16,452.00,
//! TCP = Ontario BPA 12,989.00), no additional tax (L = 0). Pay date 2026-08-14.
//!
//! Hand computation (each factor rounded to cents, as the text's own examples do):
//!
//! CPP (Chapter 6)
//!   basic exemption per pay = trunc(3,500 / 26, 2) = trunc(134.6153..) = 134.61
//!   (ii) 0.0595 × (2,400.00 − 134.61) = 0.0595 × 2,265.39 = 134.790705
//!   (i)  4,230.45 − 0 = 4,230.45
//!   C = lesser, to cents = 134.79
//!   C2: W = max(0, 74,600) = 74,600; (0 + 2,400 − 74,600) × 0.04 < 0 → C2 = 0.00
//!
//! EI (Chapter 7)
//!   0.0163 × 2,400.00 = 39.12; (1,123.07 − 0) is larger → EI = 39.12
//!   employer EI = 1.4 × 39.12 = 54.768 → 54.77; employer CPP = 134.79, CPP2 = 0.00
//!
//! Step 1: F5 = 134.79 × (0.01 / 0.0595) + 0 = 22.653782 → 22.65; F5A = 22.65 (B = 0)
//!   A = 26 × (2,400.00 − 22.65) = 26 × 2,377.35 = 61,811.10
//!
//! Step 2 (federal): A is in the 58,523–117,045 bracket: R = 0.205, K = 3,804
//!   K1 = 0.14 × 16,452 = 2,303.28
//!   K2 = 0.14 × min(26 × 134.79 × (0.0495/0.0595), 3,519.45) + 0.14 × min(26 × 39.12, 1,123.07)
//!      134.79 × 0.0495 / 0.0595 = 112.13621…; × 26 = 2,915.5416 (below 3,519.45)
//!      = 0.14 × 2,915.5416 + 0.14 × 1,017.12 = 408.1758 + 142.3968 = 550.5726 → 550.57
//!   K4 = min(0.14 × 26 × 2,400, 0.14 × 1,501) = min(8,736.00, 210.14) = 210.14
//!   T3 = 0.205 × 61,811.10 − 3,804 − 2,303.28 − 550.57 − 210.14
//!      = 12,671.2755 − 6,867.99 = 5,803.2855 → 5,803.29
//!   T1 = T3 (LCF = 0) = 5,803.29
//!
//! Step 4 (Ontario): A in 53,891–107,785 bracket: V = 0.0915, KP = 2,210
//!   K1P = 0.0505 × 12,989 = 655.9445 → 655.94
//!   K2P = 0.0505 × 2,915.5416 + 0.0505 × 1,017.12 = 147.2349 + 51.3676 = 198.6024 → 198.60
//!   T4 = 0.0915 × 61,811.10 − 2,210 − 655.94 − 198.60
//!      = 5,655.71565 − 3,064.54 = 2,591.17565 → 2,591.18
//!
//! Step 5 (Ontario): V1: T4 ≤ 5,818 → 0.00
//!   V2: 48,000 < A ≤ 72,000 → min(600, 450 + 0.25 × (61,811.10 − 48,000)) = min(600, 3,902.78) = 600.00
//!   S: x = T4 + V1 = 2,591.18; min(x, 2 × 300 − x) < 0 → S = 0.00
//!   T2 = 2,591.18 + 0 + 600.00 − 0 = 3,191.18
//!
//! Step 6: T = (T1 + T2) / 26 = 8,994.47 / 26 = 345.9411 → 345.94
//!   federal = round(5,803.29 / 26) = round(223.2034) = 223.20
//!   provincial = 345.94 − 223.20 = 122.74

use chrono::NaiveDate;
use payroll_ca::{Pay, Province, calculate};
use rust_decimal::dec;

fn ontario_biweekly() -> Pay {
    Pay {
        pay_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
        province: Province::ON,
        periods_per_year: 26,
        gross_taxable: dec!(2400.00),
        pensionable: dec!(2400.00),
        insurable: dec!(2400.00),
        federal_claim: dec!(16452.00),
        provincial_claim: dec!(12989.00),
        ytd_cpp_base_and_first: dec!(0),
        ytd_cpp2: dec!(0),
        ytd_ei: dec!(0),
        ytd_pensionable: dec!(0),
        cpp_exempt: false,
        ei_exempt: false,
        birth_date: None,
        additional_tax: dec!(0),
    }
}

fn factor(factors: &[(String, rust_decimal::Decimal)], name: &str) -> rust_decimal::Decimal {
    factors
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("factor {name} missing"))
        .1
}

#[test]
fn ontario_biweekly_2400_claim_code_1() {
    let d = calculate(&ontario_biweekly()).expect("calculates");

    assert_eq!(d.cpp, dec!(134.79), "C");
    assert_eq!(d.cpp2, dec!(0.00), "C2");
    assert_eq!(d.ei, dec!(39.12), "EI");
    assert_eq!(d.employer_cpp, dec!(134.79));
    assert_eq!(d.employer_cpp2, dec!(0.00));
    assert_eq!(d.employer_ei, dec!(54.77), "1.4 × EI");

    assert_eq!(d.federal_tax, dec!(223.20), "T1 / 26");
    assert_eq!(d.provincial_tax, dec!(122.74), "T / 26 less federal");
    assert_eq!(d.total_tax, dec!(345.94), "(T1 + T2) / 26");
    assert_eq!(d.federal_tax + d.provincial_tax, d.total_tax);

    let f = &d.factors;
    assert_eq!(factor(f, "A"), dec!(61811.10));
    assert_eq!(factor(f, "F5"), dec!(22.65));
    assert_eq!(factor(f, "R"), dec!(0.205));
    assert_eq!(factor(f, "K"), dec!(3804));
    assert_eq!(factor(f, "K1"), dec!(2303.28));
    assert_eq!(factor(f, "K2"), dec!(550.57));
    assert_eq!(factor(f, "K4"), dec!(210.14));
    assert_eq!(factor(f, "T3"), dec!(5803.29));
    assert_eq!(factor(f, "T1"), dec!(5803.29));
    assert_eq!(factor(f, "V"), dec!(0.0915));
    assert_eq!(factor(f, "KP"), dec!(2210));
    assert_eq!(factor(f, "K1P"), dec!(655.94));
    assert_eq!(factor(f, "K2P"), dec!(198.60));
    assert_eq!(factor(f, "T4"), dec!(2591.18));
    assert_eq!(factor(f, "V1"), dec!(0.00));
    assert_eq!(factor(f, "V2"), dec!(600.00));
    assert_eq!(factor(f, "S"), dec!(0.00));
    assert_eq!(factor(f, "T2"), dec!(3191.18));
    assert_eq!(factor(f, "T"), dec!(345.94));
}
