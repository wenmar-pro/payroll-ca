# payroll-ca

Canadian payroll source deductions, calculated by the formulas in the CRA publication
T4127, *Payroll Deductions Formulas*, **Option 1 (the regular method)**, for pay dates
from 2026-07-01 (123rd edition, effective July 1, 2026).

The crate is a Rust library. It does no I/O. It computes, for one pay:

- federal income tax and provincial or territorial income tax (every province and
  territory except Quebec, including Ontario surtax and Health Premium, and the Ontario
  and British Columbia tax reductions);
- employee CPP (base and first additional), CPP2 (second additional) and EI premiums;
- the employer's matching CPP, CPP2 and 1.4 times the employee EI premium.

Every named T4127 factor used in a calculation is returned, so a result can be explained.

## Scope and limits

- **Option 1 only.** Option 2 (cumulative averaging) and the commission and bonus
  (non-periodic payment) methods are not implemented.
- **Quebec is not supported.** `Province::parse("QC")` returns `Error::QuebecNotSupported`.
  Quebec's provincial tax and QPP are administered by Revenu Québec.
- **Rate set.** Only the 123rd edition (2026-07-01 to 2026-12-31) is built in. A pay date
  outside that range is an error; the crate never falls back to older tables.
- **Inputs not modelled.** Factors F, F1 to F4, U1, HD, K3, LCF and LCP (RPP and RRSP
  deductions, union dues, prescribed-zone and other credits, labour-sponsored credits)
  and Y (Ontario dependants) are treated as zero. Claim code E (T = 0) is not represented;
  pass the claim amounts instead of codes.
- **Results are not guaranteed.** The formulas are approved by CRA, but the calculation is
  this crate's implementation of them. The employer remains liable to CRA for the amounts it
  deducts and remits. Check results against CRA's Payroll Deductions Online Calculator
  before relying on them.

## Usage

```rust
use chrono::NaiveDate;
use payroll_ca::{Pay, Province, calculate};
use rust_decimal::dec;

let pay = Pay {
    pay_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
    province: Province::parse("ON")?,
    periods_per_year: 26,
    gross_taxable: dec!(2400.00),
    pensionable: dec!(2400.00),
    insurable: dec!(2400.00),
    federal_claim: dec!(16452.00),   // TC from TD1, or the claim code amount
    provincial_claim: dec!(12989.00), // TCP from the provincial TD1
    ytd_cpp_base_and_first: dec!(0),
    ytd_cpp2: dec!(0),
    ytd_ei: dec!(0),
    ytd_pensionable: dec!(0),
    cpp_exempt: false,
    ei_exempt: false,
    birth_date: None,
    additional_tax: dec!(0),
};
let deductions = calculate(&pay)?;
```

`federal_tax`, `provincial_tax` and `total_tax` are per pay. `provincial_tax` is
`total_tax` less `federal_tax`, so the three always add up.

## Rounding

Money is `rust_decimal::Decimal`; the crate never uses floating point. Each named factor
(C, C2, F5, A, the K's, T3, T4, V1, V2, S, T2) is rounded to cents, half up, at the point the
text and its worked examples round it. Rate ratios and products inside a formula are not
rounded. The CPP basic exemption per pay is truncated to cents (T4127 Chapter 1).

## Tests

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

- `tests/ontario_case.rs` checks one Ontario case, worked by hand; the arithmetic is in its
  comments.
- `tests/properties.rs` holds deterministic sweeps: CPP, CPP2 and EI stop at their maxima,
  tax is never negative, Quebec and unknown provinces are errors, dates outside the edition
  are errors, and the age window for CPP is applied.
- `tests/cases.rs` loads every `cases/*.csv` file and checks each row. The format is in
  `cases/README.md`. The shared corpus is empty for now.

## Maintenance

Maintained by Wenmar, <https://wenmarpro.com>. Licensed under the Apache License 2.0
(see `LICENSE`).
