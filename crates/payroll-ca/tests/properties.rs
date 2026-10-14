//! Property-style checks, written as deterministic sweeps (no property-testing crate).

use chrono::NaiveDate;
use payroll_ca::{Error, Pay, Province, calculate};
use rust_decimal::{Decimal, dec};

const ALL: [Province; 12] = [
    Province::AB,
    Province::BC,
    Province::MB,
    Province::NB,
    Province::NL,
    Province::NS,
    Province::NT,
    Province::NU,
    Province::ON,
    Province::PE,
    Province::SK,
    Province::YT,
];

fn base(province: Province, gross: Decimal) -> Pay {
    Pay {
        pay_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        province,
        periods_per_year: 26,
        gross_taxable: gross,
        pensionable: gross,
        insurable: gross,
        federal_claim: dec!(16452.00),
        provincial_claim: dec!(13000.00),
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

/// Small deterministic generator (linear congruential) for varied inputs.
struct Lcg(u64);

impl Lcg {
    fn next_cents(&mut self, max_dollars: i64) -> Decimal {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let cents = (self.0 >> 11) as i64 % (max_dollars * 100 + 1);
        Decimal::new(cents, 2)
    }
}

#[test]
fn cpp_stops_at_the_yearly_maximum() {
    let mut pay = base(Province::ON, dec!(2400.00));
    pay.ytd_cpp_base_and_first = dec!(4230.45);
    let d = calculate(&pay).unwrap();
    assert_eq!(d.cpp, dec!(0));
    assert_eq!(d.employer_cpp, dec!(0));

    // Only the room left under the maximum is deducted.
    pay.ytd_cpp_base_and_first = dec!(4200.00);
    let d = calculate(&pay).unwrap();
    assert_eq!(d.cpp, dec!(30.45));
}

#[test]
fn cpp2_stops_at_the_second_additional_maximum() {
    let mut pay = base(Province::ON, dec!(100000.00));
    pay.periods_per_year = 12;
    pay.ytd_pensionable = dec!(0);
    // Band between YMPE (74,600) and YAMPE (85,000) gives 0.04 × 10,400 = 416.
    assert_eq!(calculate(&pay).unwrap().cpp2, dec!(416.00));

    pay.ytd_cpp2 = dec!(416.00);
    assert_eq!(calculate(&pay).unwrap().cpp2, dec!(0));

    pay.ytd_cpp2 = dec!(400.00);
    assert_eq!(calculate(&pay).unwrap().cpp2, dec!(16.00));
}

#[test]
fn ei_stops_at_the_yearly_maximum() {
    let mut pay = base(Province::ON, dec!(5000.00));
    pay.ytd_ei = dec!(1123.07);
    assert_eq!(calculate(&pay).unwrap().ei, dec!(0));
    assert_eq!(calculate(&pay).unwrap().employer_ei, dec!(0));

    pay.ytd_ei = dec!(1100.00);
    let d = calculate(&pay).unwrap();
    assert_eq!(d.ei, dec!(23.07), "only the room left under 1,123.07");
    assert_eq!(d.employer_ei, dec!(32.30), "1.4 × 23.07 = 32.298");
}

#[test]
fn ei_is_the_rate_times_insurable_below_the_cap() {
    // 0.0163 × 1,000.00 = 16.30 per pay (the text's weekly example).
    let pay = base(Province::BC, dec!(1000.00));
    let d = calculate(&pay).unwrap();
    assert_eq!(d.ei, dec!(16.30));
}

#[test]
fn tax_is_never_negative_and_components_add_up() {
    let mut rng = Lcg(42);
    for province in ALL {
        for gross in [dec!(0), dec!(0.01), dec!(1), dec!(50), dec!(400)] {
            let d = calculate(&base(province, gross)).unwrap();
            assert!(d.federal_tax >= dec!(0), "{province:?} gross {gross}");
            assert!(d.provincial_tax >= dec!(0), "{province:?} gross {gross}");
            assert!(d.total_tax >= dec!(0), "{province:?} gross {gross}");
            assert_eq!(d.federal_tax + d.provincial_tax, d.total_tax);
        }
        for _ in 0..200 {
            let gross = rng.next_cents(20_000);
            let mut pay = base(province, gross);
            pay.federal_claim = rng.next_cents(20_000);
            pay.provincial_claim = rng.next_cents(20_000);
            pay.ytd_pensionable = rng.next_cents(200_000);
            pay.ytd_cpp_base_and_first = rng.next_cents(4_000);
            pay.ytd_cpp2 = rng.next_cents(400);
            pay.ytd_ei = rng.next_cents(1_100);
            let d = calculate(&pay).unwrap();
            assert!(d.total_tax >= dec!(0));
            assert!(d.federal_tax >= dec!(0) && d.provincial_tax >= dec!(0));
            assert!(d.cpp >= dec!(0) && d.cpp2 >= dec!(0) && d.ei >= dec!(0));
            assert!(d.total_tax.scale() <= 2, "{}", d.total_tax);
        }
    }
}

#[test]
fn quebec_is_an_error() {
    assert_eq!(Province::parse("QC"), Err(Error::QuebecNotSupported));
    assert_eq!(Province::parse("qc"), Err(Error::QuebecNotSupported));
}

#[test]
fn unknown_province_is_an_error() {
    assert_eq!(
        Province::parse("XX"),
        Err(Error::UnknownProvince("XX".into()))
    );
    assert!(matches!(
        Province::parse(""),
        Err(Error::UnknownProvince(_))
    ));
}

#[test]
fn every_supported_province_parses() {
    for code in [
        "AB", "BC", "MB", "NB", "NL", "NS", "NT", "NU", "ON", "PE", "SK", "YT",
    ] {
        assert!(Province::parse(code).is_ok(), "{code}");
    }
}

#[test]
fn a_date_outside_the_edition_is_an_error() {
    let mut pay = base(Province::ON, dec!(1000.00));
    pay.pay_date = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();
    assert!(matches!(calculate(&pay), Err(Error::DateOutOfRange { .. })));

    pay.pay_date = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
    assert!(matches!(calculate(&pay), Err(Error::DateOutOfRange { .. })));

    pay.pay_date = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    assert!(calculate(&pay).is_ok());
    pay.pay_date = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
    assert!(calculate(&pay).is_ok());
}

#[test]
fn invalid_inputs_are_errors() {
    let mut pay = base(Province::ON, dec!(1000.00));
    pay.periods_per_year = 0;
    assert!(matches!(calculate(&pay), Err(Error::InvalidPay(_))));

    let mut pay = base(Province::ON, dec!(-1.00));
    pay.periods_per_year = 26;
    assert!(matches!(calculate(&pay), Err(Error::InvalidPay(_))));
}

#[test]
fn cpp_starts_with_the_first_pay_in_the_month_after_age_18() {
    // Born 2008-09-15: 18th birthday in September 2026, so CPP starts with the October
    // 2026 pay. PM = 3 (October to December), so the year's maximum is 4,230.45 × 3/12.
    let mut pay = base(Province::ON, dec!(2400.00));
    pay.birth_date = Some(NaiveDate::from_ymd_opt(2008, 9, 15).unwrap());
    pay.pay_date = NaiveDate::from_ymd_opt(2026, 9, 25).unwrap();
    assert_eq!(calculate(&pay).unwrap().cpp, dec!(0));
    pay.pay_date = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let d = calculate(&pay).unwrap();
    assert_eq!(d.cpp, dec!(134.79));
    assert!(d.factors.iter().any(|(n, v)| n == "PM" && *v == dec!(3)));
}

#[test]
fn cpp_stops_after_the_month_of_age_70() {
    // Born 1956-06-20: turns 70 in June 2026; the June pay is the last with CPP.
    let mut pay = base(Province::ON, dec!(2400.00));
    pay.birth_date = Some(NaiveDate::from_ymd_opt(1956, 6, 20).unwrap());
    pay.pay_date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    assert_eq!(calculate(&pay).unwrap().cpp, dec!(0));
}
