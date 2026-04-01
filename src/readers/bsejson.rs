//! Reader for JSON/BSE JSON format.

use crate::prelude::*;

/// Reads a basis set from JSON format.
///
/// This reader parses JSON-formatted basis set data that follows the BSE
/// schema. It can handle both complete BseBasis and minimal BseBasisMinimal
/// formats.
pub fn read_bsejson(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    // Try to parse as BseBasis first (complete format)
    if let Ok(basis) = serde_json::from_str::<BseBasis>(basis_str) {
        return Ok(BseBasisMinimal {
            molssi_bse_schema: basis.molssi_bse_schema,
            elements: basis.elements,
            function_types: basis.function_types,
            name: basis.name,
            description: basis.description,
        });
    }

    // Try to parse as BseBasisMinimal
    let basis: BseBasisMinimal = serde_json::from_str(basis_str)
        .map_or(bse_raise!(ValueError, "Failed to parse JSON as BSE basis format"), Ok)?;

    Ok(basis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writers::bsejson::write_bsejson;

    #[test]
    fn test_read_bsejson() {
        let args = BseGetBasisArgsBuilder::default().elements("H, C-O".to_string()).build().unwrap();
        let basis = get_basis("cc-pVTZ", args);
        let json_str = write_bsejson(&basis);
        let parsed = read_bsejson(&json_str).unwrap();
        println!("{parsed:#?}");
    }

    #[test]
    fn test_read_bsejson_minimal() {
        let args = BseGetBasisArgsBuilder::default().elements("1, 6-O".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("cc-pVTZ", "nwchem", args);
        let basis_minimal = read_formatted_basis_str(&basis_str, "nwchem");
        let json_str = serde_json::to_string_pretty(&basis_minimal).unwrap();
        let parsed = read_bsejson(&json_str).unwrap();
        println!("{parsed:#?}");
    }
}
