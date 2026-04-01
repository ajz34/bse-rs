//! Parser for OpenMolcas' RICDlib format.

use crate::prelude::*;
use crate::readers::helpers;

lazy_static::lazy_static! {
    // Basis header: /H.cc-pVDZ....aCD-aux-basis.
    static ref BASIS_HEAD_RE: Regex = Regex::new(
        &format!(r"^/([a-zA-Z]+).({}|)....(aCD|acCD)-aux-basis.\s*$", helpers::BASIS_NAME_RE.as_str())
    ).unwrap();
    // Charge line: charge, lmax, nbasis
    static ref CHARGE_LINE_RE: Regex = Regex::new(
        &format!(r"^\s*({})\s+(\d+)\s+(\d+)\s*$", helpers::FLOATING_RE.as_str())
    ).unwrap();
    // Dummy reference line
    static ref DUMMY_LINE_RE: Regex = Regex::new(r"^\s*Dummy reference line.\s*$").unwrap();
    // Shell start: nprim, ncontr, amtype
    static ref SHELL_START_RE: Regex = Regex::new(r"^\s*(\d+)\s+(\d+)\s+(\d+)\s*$").unwrap();
}

/// Parses a basis block for a single element.
fn parse_basis(elements: &mut HashMap<String, BseBasisElement>, basis_lines: &[String]) -> Result<(), BseError> {
    if basis_lines.len() < 5 {
        return Ok(());
    }

    // Parse header
    let parsed = helpers::parse_line_regex(&BASIS_HEAD_RE, &basis_lines[0], "Symbol.Basis....a(c)CD-aux-basis.")?;
    let element_symbol = &parsed[0];
    let _basis_name = &parsed[1];
    let _basis_type = &parsed[2];

    // Initialize element
    let element_Z = lut::element_Z_from_sym(element_symbol)
        .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_symbol), Ok)?;

    // Parse charge line
    let parsed = helpers::parse_line_regex(&CHARGE_LINE_RE, &basis_lines[1], "charge, lmax, nbasis")?;
    let _charge = &parsed[0];
    let lmax: i32 = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid lmax"), Ok)?;
    let _nbasis: usize = parsed[2].parse().map_or(bse_raise!(ValueError, "Invalid nbasis"), Ok)?;

    // Verify dummy lines
    if !DUMMY_LINE_RE.is_match(&basis_lines[2]) {
        eprintln!("Warning: Expected dummy line, got: '{}'", basis_lines[2]);
    }
    if !DUMMY_LINE_RE.is_match(&basis_lines[3]) {
        eprintln!("Warning: Expected dummy line, got: '{}'", basis_lines[3]);
    }

    // Shell data starts at index 4
    let mut line_idx = 4;

    // Parse shells for each angular momentum from 0 to lmax
    for l in 0..=lmax {
        if line_idx >= basis_lines.len() {
            break;
        }

        // Parse shell start
        let parsed = helpers::parse_line_regex(&SHELL_START_RE, &basis_lines[line_idx], "nprim, ncontr, amtype")?;
        let nprim: usize = parsed[0].parse().map_or(bse_raise!(ValueError, "Invalid nprim"), Ok)?;
        let ncontr: usize = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid ncontr"), Ok)?;
        let amtype: i32 = parsed[2].parse().map_or(bse_raise!(ValueError, "Invalid amtype"), Ok)?;

        line_idx += 1;

        // Skip dummy entries
        if nprim == 0 || ncontr == 0 {
            continue;
        }

        // Read exponents
        let (exponents, remaining) = helpers::read_n_floats(&basis_lines[line_idx..], nprim, None)?;
        line_idx += basis_lines[line_idx..].len() - remaining.len();

        // Read coefficients
        let (coefficients_flat, remaining) = helpers::read_n_floats(&basis_lines[line_idx..], nprim * ncontr, None)?;
        line_idx += basis_lines[line_idx..].len() - remaining.len();

        // Reshape coefficients
        let coefficients = helpers::chunk_list(&coefficients_flat, nprim, ncontr)?;
        let coefficients = misc::transpose_matrix(&coefficients);

        // Determine function type: spherical if amtype == 3, cartesian otherwise
        let func_type = lut::function_type_from_am(&[l], "gto", if amtype == 3 { "spherical" } else { "cartesian" });

        let shell = BseElectronShell {
            function_type: func_type,
            region: "".to_string(),
            angular_momentum: vec![l],
            exponents,
            coefficients,
        };

        elements
            .entry(element_Z.to_string())
            .or_default()
            .electron_shells
            .get_or_insert_with(Default::default)
            .push(shell);
    }

    Ok(())
}

pub fn read_ricdlib(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    let basis_lines: Vec<String> = basis_str.lines().map(|s| s.trim().to_string()).collect();

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

    // Split into element blocks
    let element_blocks = helpers::partition_lines(
        &basis_lines,
        |x| BASIS_HEAD_RE.is_match(x),
        0,
        None,
        None,
        None,
        5, // minimum size: header + charge + 2 dummy + at least one shell
        true,
    )?;

    let mut basis_dict = BseBasisMinimal {
        molssi_bse_schema: BseMolssiBseSchema { schema_type: "minimal".to_string(), schema_version: "0.1".to_string() },
        elements: HashMap::new(),
        function_types: Vec::new(),
        name: "unknown_basis".to_string(),
        description: "no_description".to_string(),
    };

    // Track basis names found
    let mut basis_names_found: HashSet<String> = HashSet::new();

    for element_lines in &element_blocks {
        if element_lines.is_empty() {
            continue;
        }

        // Parse header to get basis name
        let parsed = helpers::parse_line_regex(&BASIS_HEAD_RE, &element_lines[0], "Symbol.Basis....a(c)CD-aux-basis.")?;
        let basis_name = &parsed[1];
        if !basis_name.is_empty() {
            basis_names_found.insert(basis_name.to_lowercase());
        }

        parse_basis(&mut basis_dict.elements, element_lines)?;
    }

    // Check for multiple basis sets
    if basis_names_found.len() > 1 {
        eprintln!("Warning: Multiple basis sets found in file: {}", basis_names_found.iter().join(", "));
    }

    // Use first basis name
    if let Some(name) = basis_names_found.iter().next() {
        basis_dict.name = name.clone();
    }

    let function_types = compose::whole_basis_types(&basis_dict.elements);
    basis_dict.function_types = function_types;

    Ok(basis_dict)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: RICDlib format does not have a corresponding writer in bse-rs
    // This test requires sample RICDlib files

    #[test]
    fn test_read_ricdlib_sample() {
        // Sample RICDlib format
        let sample = r"/H.cc-pVDZ....aCD-aux-basis.
     1.00   1   1
 Dummy reference line.
 Dummy reference line.
    3    1    1
 0.100000E+01 0.200000E+01 0.300000E+01
 0.123000E+00 0.234000E+00 0.345000E+00";
        let basis = read_ricdlib(sample).unwrap();
        println!("{basis:#?}");
    }
}
