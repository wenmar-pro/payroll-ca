# T4032 table extracts

CRA's payroll deductions tables (T4032), read from the PDFs with
`cargo xtask t4032-import <pdf-dir> <province> <out.csv>` (PDFs from canada.ca, kept in
`reference/`, not committed).

- `<province>-<effective>.csv`: one row per income range of a federal or provincial tax table:
  `province,periods,table,from,to,cc0..cc10` (the tax for claim codes 0 to 10; a blank cell in the
  table is `0.00`).
- `claims-<province>.csv`: the claim amount each claim code stands for (T4127 "Option 1, TC").

CRA computes a row at the middle of its range and rounds it to the nearest 5 cents.

## Status: not yet a passing check

`tests/t4032.rs` (ignored) compares the engine with every amount. On Ontario 2026 it agrees with
14,690 of 33,792. Two differences remain:

1. **Annual income above the YMPE.** The tables are built on a year of the same pay, so they
   include CPP2 and cap the CPP additional deduction (F5) at the yearly maximums. The engine
   follows the T4127 per-pay formula with no year-to-date, as PDOC does, so it has no CPP2 on that
   pay and no cap. Provincial amounts differ by up to $24 a month at $250,000 a year.
2. **A federal bias of 2.5 to 4.5 cents** below the YMPE, where the table is always the higher.
   Cause not found yet.

Enabling the test needs the table method modelled in the test (not in the engine, which matches
PDOC).
