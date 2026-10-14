//! The JSON shape of `Pay` and `Deductions`: snake_case fields, decimals as strings,
//! provinces as two-letter codes.

use chrono::NaiveDate;
use payroll_ca::{Pay, Province, calculate};
use rust_decimal::dec;

fn ontario_pay() -> Pay {
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

#[test]
fn a_pay_round_trips_through_json() {
    let pay = ontario_pay();
    let json = serde_json::to_string(&pay).expect("serialize");
    let back: Pay = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, pay);
}

#[test]
fn a_pay_is_snake_case_with_string_decimals_and_codes() {
    let value = serde_json::to_value(ontario_pay()).expect("serialize");
    assert_eq!(value["pay_date"], "2026-08-14");
    assert_eq!(value["province"], "ON");
    assert_eq!(value["periods_per_year"], 26);
    assert_eq!(value["gross_taxable"], "2400.00");
    assert_eq!(value["federal_claim"], "16452.00");
    assert_eq!(value["birth_date"], serde_json::Value::Null);
    assert_eq!(value["cpp_exempt"], false);
}

#[test]
fn a_json_pay_calculates_the_same_deductions() {
    let json = r#"{
        "pay_date": "2026-08-14", "province": "ON", "periods_per_year": 26,
        "gross_taxable": "2400.00", "pensionable": "2400.00", "insurable": "2400.00",
        "federal_claim": "16452.00", "provincial_claim": "12989.00",
        "ytd_cpp_base_and_first": "0", "ytd_cpp2": "0", "ytd_ei": "0",
        "ytd_pensionable": "0", "cpp_exempt": false, "ei_exempt": false,
        "birth_date": null, "additional_tax": "0"
    }"#;
    let pay: Pay = serde_json::from_str(json).expect("deserialize");
    assert_eq!(pay, ontario_pay());
    let deductions = calculate(&pay).expect("calculates");
    let out = serde_json::to_value(&deductions).expect("serialize");
    assert_eq!(out["cpp"], "134.79");
    assert_eq!(out["ei"], "39.12");
    assert!(out["factors"].is_array());
}

#[test]
fn an_unknown_province_code_is_rejected() {
    let json = r#"{"pay_date": "2026-08-14", "province": "QC"}"#;
    assert!(serde_json::from_str::<Pay>(json).is_err());
}
