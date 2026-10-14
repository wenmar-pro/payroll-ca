//! WebAssembly bindings for `payroll-ca`. `calculate` takes and returns plain JavaScript
//! objects shaped like the crate's serde JSON: snake_case fields, decimals as strings,
//! provinces as two-letter codes.

use payroll_ca::{Pay, calculate as calculate_pay, rates};
use wasm_bindgen::prelude::*;

/// Calculate the deductions for one pay (T4127 Option 1).
///
/// `pay` is an object with the fields of `Pay` (see the README). The result is an object
/// with `federal_tax`, `provincial_tax`, `total_tax`, `cpp`, `cpp2`, `ei`, `employer_cpp`,
/// `employer_cpp2`, `employer_ei` (decimal strings) and `factors` (`[name, value]` pairs).
/// Invalid input, Quebec, and dates outside the edition throw an error with the message.
#[wasm_bindgen]
pub fn calculate(pay: JsValue) -> Result<JsValue, JsError> {
    let pay: Pay = serde_wasm_bindgen::from_value(pay).map_err(|e| JsError::new(&e.to_string()))?;
    let deductions = calculate_pay(&pay).map_err(|e| JsError::new(&e.to_string()))?;
    serde_wasm_bindgen::to_value(&deductions).map_err(|e| JsError::new(&e.to_string()))
}

/// The T4127 edition the calculation uses, for example `"123 (2026-07-01 to 2026-12-31)"`.
#[wasm_bindgen]
pub fn edition() -> String {
    let set = rates::ed123::rate_set();
    format!("{} ({} to {})", set.edition, set.effective, set.valid_until)
}

#[cfg(test)]
mod tests {
    #[test]
    fn edition_names_the_123rd_edition_and_its_dates() {
        assert_eq!(super::edition(), "123 (2026-07-01 to 2026-12-31)");
    }
}
