# Shared case corpus

Each `*.csv` file here is a set of pay cases with the deductions expected for them.
The crate's test (`tests/cases.rs`) loads every file, calculates each row and compares
the result to the expected columns to the cent. An empty file (header only) passes.

## Columns

One row per pay. The header line is the first line of each file and is required.

| Column | Meaning |
|---|---|
| `edition` | T4127 edition the case was captured against (`123`) |
| `province` | Two-letter code (`AB` ... `YT`); `QC` rows are not allowed |
| `pay_date` | `YYYY-MM-DD`, from 2026-07-01 to 2026-12-31 |
| `periods` | Pay periods in the year (P) |
| `gross` | Gross taxable remuneration for this pay (I) |
| `pensionable` | Pensionable earnings for this pay (PI) |
| `insurable` | Insurable earnings for this pay (IE) |
| `federal_claim` | Federal total claim amount (TC) |
| `provincial_claim` | Provincial or territorial total claim amount (TCP) |
| `ytd_cpp_base_and_first` | CPP base and first additional withheld before this pay (D) |
| `ytd_cpp2` | Second additional CPP withheld before this pay (D2) |
| `ytd_ei` | EI premiums withheld before this pay (D1) |
| `ytd_pensionable` | Pensionable earnings before this pay (PIYTD) |
| `cpp_exempt` | `true` or `false` |
| `ei_exempt` | `true` or `false` |
| `birth_date` | `YYYY-MM-DD`, or empty when CPP applies all year |
| `additional_tax` | Additional tax per pay requested on Form TD1 (L) |
| `expected_federal` | Federal tax per pay (`Deductions::federal_tax`) |
| `expected_provincial` | Provincial or territorial tax per pay (`Deductions::provincial_tax`) |
| `expected_cpp` | Employee CPP base and first additional (`Deductions::cpp`) |
| `expected_cpp2` | Employee second additional CPP (`Deductions::cpp2`) |
| `expected_ei` | Employee EI premium (`Deductions::ei`) |
| `source` | Where the expected values came from (for example `PDOC 2026-10-14` or `hand`) |
| `captured_on` | `YYYY-MM-DD` the expected values were captured |

Amounts are plain decimals with no thousands separators or currency signs. Fields
contain no commas and no quotes.

## Rules for rows

- Expected values are what the CRA's own tools return for the same inputs (for example
  PDOC), or a hand calculation that shows its arithmetic in a comment in the test.
- Rows must not contain personal data. Use invented or round figures and
  dates; no employee identifiers exist in this format.
- A row for a case the crate deliberately does not support (Quebec) is not allowed.
