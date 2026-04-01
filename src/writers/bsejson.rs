//! Writer for JSON format.

use crate::prelude::*;

/// Converts a basis set to JSON format.
pub fn write_bsejson(basis: &BseBasis) -> String {
    serde_json::to_string_pretty(basis).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_bsejson() {
        let args = BseGetBasisArgsBuilder::default().elements("1, 6-O".to_string()).build().unwrap();
        let basis = get_basis("cc-pVTZ", args);
        let output = write_bsejson(&basis);
        println!("{output}");
    }
}
