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

## JavaScript / npm

The `payroll-ca-wasm` crate builds the same calculation to WebAssembly for browsers and
Node bundlers. The npm package is `@wenmar/payroll-ca`.

```js
import { calculate, edition } from "@wenmar/payroll-ca";

const deductions = calculate({
  pay_date: "2026-07-15",          // YYYY-MM-DD; selects the rate set
  province: "ON",                  // two-letter code; "QC" throws
  periods_per_year: 26,            // P
  gross_taxable: "2400.00",        // I; decimals are strings
  pensionable: "2400.00",          // PI
  insurable: "2400.00",            // IE
  federal_claim: "16452.00",       // TC from TD1 (or the claim code amount)
  provincial_claim: "12989.00",    // TCP from the provincial TD1
  ytd_cpp_base_and_first: "0.00",  // D: CPP base and first additional earlier this year
  ytd_cpp2: "0.00",                // D2: second additional CPP earlier this year
  ytd_ei: "0.00",                  // D1: EI earlier this year
  ytd_pensionable: "0.00",         // PIYTD: pensionable earnings earlier this year
  cpp_exempt: false,
  ei_exempt: false,
  birth_date: null,                // "YYYY-MM-DD" or null (CPP applies all year)
  additional_tax: "0.00",          // L: additional tax per pay from TD1
});

deductions.federal_tax;   // decimal string
deductions.cpp;           // employee CPP (base and first additional), decimal string
deductions.factors;       // [[name, value], ...]: every named T4127 factor used

edition();                // "123 (2026-07-01 to 2026-12-31)"
```

`calculate` throws an `Error` whose message says why: an unknown province, Quebec, invalid
input, or a pay date outside the edition. Input field names are exactly the snake_case names
above. Output fields are `federal_tax`, `provincial_tax`, `total_tax`, `cpp`, `cpp2`, `ei`,
`employer_cpp`, `employer_cpp2`, `employer_ei` (all decimal strings) and `factors`.

The same JSON shape is what the Rust `Pay` and `Deductions` types serialize to.

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
  before relying on them (see [Check a result yourself](#check-a-result-yourself)).

## Check a result yourself

CRA's own [Payroll Deductions Online Calculator (PDOC)](https://www.canada.ca/en/revenue-agency/services/e-services/digital-services-businesses/payroll-deductions-online-calculator.html)
gives the same figures for the same pay. To compare one result:

1. Open PDOC, accept the terms and choose **Salary**.
2. Enter the province, the pay period frequency, the date paid and the gross salary for the
   period, with nothing else in the income fields.
3. Enter the claim amounts you passed as `federal_claim` and `provincial_claim` (claim code 1 is
   the basic personal amount), and any year-to-date CPP, CPP2, EI and pensionable earnings.
   Leave a zero year-to-date field blank: PDOC refuses 0.
4. Compare PDOC's federal tax, provincial tax, CPP, CPP2 and EI with `Deductions`. They agree to
   the cent.

The cases already compared are in [`cases/pdoc-123.csv`](cases/pdoc-123.csv): each row is the
inputs typed into PDOC and the deductions it returned, with the date captured. The crate's tests
check every row (`cargo test --test cases`). How they were captured: [`cases/README.md`](cases/README.md).
CRA's formulas themselves are the T4127 publication,
[Payroll Deductions Formulas](https://www.canada.ca/en/revenue-agency/services/forms-publications/payroll/t4127-payroll-deductions-formulas.html).

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
- `tests/json.rs` checks that `Pay` round-trips through JSON, with snake_case fields,
  decimal strings and two-letter province codes.
- `tests/cases.rs` loads every `cases/*.csv` file and checks each row. The format is in
  `cases/README.md`. The shared corpus is empty for now.

## Releasing (maintainers)

Releases are made by hand. CI never publishes and holds no registry credentials.

```sh
# crates.io (payroll-ca only; payroll-ca-wasm is publish = false)
cargo publish -p payroll-ca

# npm: build the package, set its name, then publish it
wasm-pack build crates/payroll-ca-wasm --target bundler --scope wenmar --release
#   wasm-pack names the package @wenmar/payroll-ca-wasm (from the crate name). Rename it
#   to the published name before publishing:
node -e 'const f="crates/payroll-ca-wasm/pkg/package.json";const p=require("./"+f);p.name="@wenmar/payroll-ca";require("fs").writeFileSync(f,JSON.stringify(p,null,2)+"\n")'
wasm-pack publish crates/payroll-ca-wasm/pkg --access public
```

Check that `crates/payroll-ca-wasm/pkg/package.json` names `@wenmar/payroll-ca` and that
`edition()` returns the edition you intend before publishing. Bump `version` in both crates
first. The `pkg/` directory is gitignored.

## Maintenance

Each new T4127 edition (January and July) gets PDOC reference captures, taken by hand in
CRA's calculator (it refuses scripted requests). `bin/pdoc-due` lists the captures the
newest edition still needs and the inputs to type for each: a case whose rate tables did not
change since its last capture is not captured again. It compares rate tables only; an edition
that changes a formula in the T4127 text needs every case captured again.

    bin/pdoc-due [PROVINCE...]   # what to capture, e.g. bin/pdoc-due ON
    bin/pdoc-import              # PDFs -> cases/pdoc-<edition>.csv, then the case test

PDFs go in `reference/pdoc/<edition>/<province>-<case>.pdf` (not committed). `pdoc-import`
needs `pdftotext` (poppler) and refuses a PDF whose inputs are not the case its name says.

Maintained by Wenmar, <https://wenmarpro.com>. Licensed under the Apache License 2.0
(see `LICENSE`).
