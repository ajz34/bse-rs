//! Reader for GBasis format.
//!
//! GBASIS only supports electronic shells (no ECP).

use crate::prelude::*;
use crate::readers::helpers;

lazy_static::lazy_static! {
    // Element entry: "Al:aug-cc-pV5+dZ:(21s13p6d4f3g2h) -> [8s7p6d4f3g2h]"
    static ref ELEMENT_ENTRY_RE: Regex = Regex::new(r"^([a-zA-Z]{1,3}):(.*):(.*)$").unwrap();
    // Shell info: amchar, nprim, ngen
    static ref SHELL_INFO_RE: Regex = Regex::new(r"^([a-zA-Z])\s+(\d+)\s+(\d+)$").unwrap();
}

pub fn read_gbasis(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    let basis_lines: Vec<String> = basis_str.lines().map(|s| s.trim().to_string()).collect();

    // Remove comments
    let basis_lines = helpers::prune_lines(&basis_lines, "!#", true, true);

    // Empty file?
    if basis_lines.is_empty() {
        return Ok(BseBasisMinimal {
            molssi_bse_schema: BseMolssiBseSchema {
                schema_type: "minimal".to_string(),
                schema_version: "0.1".to_string(),
            },
            elements: HashMap::new(),
            function_types: Vec::new(),
            name: "unknown_basis".to_string(),
            description: "no_description".to_string(),
        });
    }

    // Split into element sections
    let element_sections = helpers::partition_lines(
        &basis_lines,
        |x| ELEMENT_ENTRY_RE.is_match(x),
        0,
        None,
        None,
        None,
        4, // minimum size: header + max_am + at least one shell header + data
        true,
    )?;

    let mut basis_dict = BseBasisMinimal {
        molssi_bse_schema: BseMolssiBseSchema { schema_type: "minimal".to_string(), schema_version: "0.1".to_string() },
        elements: HashMap::new(),
        function_types: Vec::new(),
        name: "unknown_basis".to_string(),
        description: "no_description".to_string(),
    };

    let mut found_basis_names: HashSet<String> = HashSet::new();

    for element_lines in &element_sections {
        if element_lines.len() < 4 {
            continue;
        }

        // Parse first line: element:basis:pattern
        let parsed =
            helpers::parse_line_regex(&ELEMENT_ENTRY_RE, &element_lines[0], "Element entry: sym:basis:pattern")?;
        let element_sym = &parsed[0];
        let basis_name = &parsed[1];

        let element_Z = lut::element_Z_from_sym(element_sym)
            .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_sym), Ok)?;

        found_basis_names.insert(basis_name.to_lowercase());

        // Second line is max_am
        let max_am: i32 = element_lines[1].trim().parse().map_or(bse_raise!(ValueError, "Invalid max_am"), Ok)?;

        // Split into shell blocks (lines starting with alpha character)
        let shell_blocks = helpers::partition_lines(
            &element_lines[2..],
            |x| x.chars().next().is_some_and(|c| c.is_ascii_alphabetic()),
            0,
            None,
            None,
            None,
            2, // minimum size
            true,
        )?;

        // Verify we have the right number of blocks
        if (max_am + 1) as usize != shell_blocks.len() {
            eprintln!(
                "Warning: Expected {} blocks for element {}, found {}",
                max_am + 1,
                element_sym,
                shell_blocks.len()
            );
        }

        for shell_lines in &shell_blocks {
            // Parse shell info
            let parsed = helpers::parse_line_regex(&SHELL_INFO_RE, &shell_lines[0], "Shell: AM, nprim, ngen")?;
            let amchar = &parsed[0];
            let nprim: usize = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid nprim"), Ok)?;
            let ngen: usize = parsed[2].parse().map_or(bse_raise!(ValueError, "Invalid ngen"), Ok)?;

            // Convert angular momentum
            let shell_am = lut::amchar_to_int(amchar, HIK).unwrap_or_default();

            // GBASIS doesn't support fused AM
            if shell_am.len() > 1 {
                bse_raise!(ValueError, "Fused AM not supported by gbasis reader")?;
            }

            let func_type = lut::function_type_from_am(&shell_am, "gto", "spherical");

            // Parse exponents and coefficients
            let (exponents, coefficients) =
                helpers::parse_primitive_matrix(&shell_lines[1..], Some(nprim), Some(ngen), None)?;

            let shell = BseElectronShell {
                function_type: func_type,
                region: "".to_string(),
                angular_momentum: shell_am,
                exponents,
                coefficients,
            };

            basis_dict
                .elements
                .entry(element_Z.to_string())
                .or_default()
                .electron_shells
                .get_or_insert_with(Default::default)
                .push(shell);
        }
    }

    // Warn about multiple basis sets
    if found_basis_names.len() > 1 {
        eprintln!("Warning: Multiple basis sets found in file: {}", found_basis_names.iter().join(", "));
    }

    let function_types = compose::whole_basis_types(&basis_dict.elements);
    basis_dict.function_types = function_types;

    // Use the first basis name as the name
    if let Some(name) = found_basis_names.iter().next() {
        basis_dict.name = name.clone();
    }

    Ok(basis_dict)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: GBASIS format does not have a corresponding writer in bse-rs
    // This test is for manual verification with sample GBASIS files

    #[test]
    fn test_read_gbasis_sample() {
        // Sample GBASIS format
        let sample = r"H:sto-3g:(3s) -> [1s]
0
S    3    1
  0.4231582280E+01  0.1000000000E+01
  0.2303687730E+01  0.5000000000E+00
  0.1612775880E+01  0.1666666667E+00";
        let basis = read_gbasis(sample).unwrap();
        println!("{basis:#?}");
    }
}
