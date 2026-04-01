//! Reader for deMon2K format.

use crate::prelude::*;
use crate::readers::helpers;

lazy_static::lazy_static! {
    // Orbital entry: O-ELEMENT [repetition(s)] (basis_name)
    static ref ORBITAL_RE: Regex = Regex::new(
        &format!(r"^O-([A-Za-z]+)(?: ([A-Za-z]+))* \(({})\)\s*$", helpers::BASIS_NAME_RE.as_str())
    ).unwrap();
    // Shell entry: formal_qn, am, nprim
    static ref SHELL_RE: Regex = Regex::new(r"^\s*(\d+)\s+(\d+)\s+(\d+)\s*$").unwrap();
    // ECP block start
    static ref ECP_START_RE: Regex = Regex::new(r"^\s*ECP\s*$").unwrap();
    // ECP entry: symbol nelec N
    static ref ECP_ENTRY_RE: Regex = Regex::new(r"^([A-Za-z]+)\s+nelec\s+(\d+)\s*$").unwrap();
    // ECP shell entry: symbol am
    static ref ECP_SHELL_RE: Regex = Regex::new(r"^([A-Za-z]+)\s+([A-Za-z]+)\s*$").unwrap();
    // ECP data entry: rexp gexp gcoeff
    static ref ECP_DATA_RE: Regex = Regex::new(
        &format!(r"^\s*(\d+)\s+({})\s+({})\s*$", helpers::FLOATING_RE.as_str(), helpers::FLOATING_RE.as_str())
    ).unwrap();
    // Basis end
    static ref BASIS_END_RE: Regex = Regex::new(r"^\s*END\s*$").unwrap();
}

/// Parses lines representing all the electron shells for a single element.
fn parse_electron_lines(
    elements: &mut HashMap<String, BseBasisElement>,
    element_lines: &[String],
) -> Result<(), BseError> {
    if element_lines.is_empty() {
        return Ok(());
    }

    // First line should be element and basis name
    let parsed = helpers::parse_line_regex(&ORBITAL_RE, &element_lines[0], "O-element (basis_name)")?;
    let element_name = &parsed[0];

    // Get element Z from name (deMon2K uses element name, not symbol)
    let element_Z = lut::element_Z_from_name(element_name)
        .map_or(bse_raise!(ValueError, "Unknown element name: {}", element_name), Ok)?;

    // Second line is number of shells
    let n_shells: usize = element_lines[1]
        .split_whitespace()
        .next()
        .map_or(bse_raise!(ValueError, "Missing number of shells"), Ok)?
        .parse()
        .map_or(bse_raise!(ValueError, "Invalid number of shells"), Ok)?;

    // Partition into shell blocks
    let shell_blocks = helpers::partition_lines(
        &element_lines[2..],
        |x| SHELL_RE.is_match(x),
        0,
        None,
        None,
        None,
        2, // minimum size
        true,
    )?;

    if shell_blocks.len() != n_shells {
        eprintln!("Warning: Expected {} shells, found {}", n_shells, shell_blocks.len());
    }

    for shell_lines in &shell_blocks {
        if !SHELL_RE.is_match(&shell_lines[0]) {
            continue;
        }

        let parsed = helpers::parse_line_regex(&SHELL_RE, &shell_lines[0], "formal_qn, am, nprim")?;
        let _formal_qn: i32 = parsed[0].parse().map_or(bse_raise!(ValueError, "Invalid formal qn"), Ok)?;
        let shell_am: i32 = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid shell am"), Ok)?;
        let nprim: usize = parsed[2].parse().map_or(bse_raise!(ValueError, "Invalid nprim"), Ok)?;

        // Parse exponents and coefficients
        // deMon2K format has ngen = 1 (uncontracted)
        let ngen = 1;
        let (exponents, coefficients) =
            helpers::parse_primitive_matrix(&shell_lines[1..1 + nprim], Some(nprim), Some(ngen), None)?;

        let func_type = lut::function_type_from_am(&[shell_am], "gto", "spherical");

        let shell = BseElectronShell {
            function_type: func_type,
            region: "".to_string(),
            angular_momentum: vec![shell_am],
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

/// Parses lines representing all the ECP potentials for a single element.
fn parse_ecp_lines(elements: &mut HashMap<String, BseBasisElement>, ecp_lines: &[String]) -> Result<(), BseError> {
    if ecp_lines.is_empty() {
        return Ok(());
    }

    // Skip the initial "ECP" line if present
    let ecp_lines = if !ecp_lines.is_empty() && ecp_lines[0].trim() == "ECP" { &ecp_lines[1..] } else { ecp_lines };

    // Split by element symbol using ECP entry pattern
    let element_blocks = helpers::partition_lines(
        ecp_lines,
        |x| ECP_ENTRY_RE.is_match(x),
        0,
        None,
        None,
        None,
        2, // minimum size: nelec line + at least one shell header
        true,
    )?;

    for element_lines in &element_blocks {
        if !ECP_ENTRY_RE.is_match(&element_lines[0]) {
            continue;
        }

        let parsed = helpers::parse_line_regex(&ECP_ENTRY_RE, &element_lines[0], "symbol nelec N")?;
        let element_sym = &parsed[0];
        let ecp_electrons: i32 = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid nelec"), Ok)?;

        let element_Z = lut::element_Z_from_sym(element_sym)
            .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_sym), Ok)?;

        // Set ECP electrons
        elements.entry(element_Z.to_string()).or_default().ecp_electrons = Some(ecp_electrons);

        // Parse ECP shell blocks
        let mut am_blocks: Vec<(String, Vec<Vec<String>>)> = Vec::new();
        let mut current_block: Option<(String, Vec<Vec<String>>)> = None;

        for line in &element_lines[1..] {
            if ECP_SHELL_RE.is_match(line) {
                // Start new shell block
                if let Some(block) = current_block.take() {
                    am_blocks.push(block);
                }

                let parsed = helpers::parse_line_regex(&ECP_SHELL_RE, line, "symbol ecp-am")?;
                let ecp_am = &parsed[1];
                current_block = Some((ecp_am.clone(), Vec::new()));
            } else if ECP_DATA_RE.is_match(line) {
                // Add data to current block
                if let Some((_, data)) = &mut current_block {
                    let parsed = helpers::parse_line_regex(&ECP_DATA_RE, line, "rexp gexp gcoeff")?;
                    data.push(parsed);
                }
            } else if BASIS_END_RE.is_match(line) {
                break;
            }
        }

        // Add last block
        if let Some(block) = current_block.take() {
            am_blocks.push(block);
        }

        // Process AM blocks
        let n_blocks = am_blocks.len();
        for (iblock, (am_str, data)) in am_blocks.into_iter().enumerate() {
            // First entry is highest projector (ul), then S, P, ...
            let current_am = if iblock == 0 {
                // "ul" means highest AM
                (n_blocks - 1) as i32
            } else {
                (iblock - 1) as i32
            };

            // Verify AM matches expected
            if iblock > 0 {
                let expected_amchar = lut::amint_to_char(&[current_am], HIK).to_lowercase();
                if am_str.to_lowercase() != expected_amchar {
                    eprintln!("Warning: Expected ECP AM {} but found {}", expected_amchar, am_str);
                }
            }

            // Collect data
            let r_exp: Vec<i32> = data.iter().map(|x| x[0].parse().unwrap()).collect();
            let g_exp: Vec<String> = data.iter().map(|x| x[1].clone()).collect();
            let coeff: Vec<String> = data.iter().map(|x| x[2].clone()).collect();

            let ecp_pot = BseEcpPotential {
                angular_momentum: vec![current_am],
                coefficients: vec![coeff],
                ecp_type: "scalar_ecp".to_string(),
                r_exponents: r_exp,
                gaussian_exponents: g_exp,
            };

            elements
                .entry(element_Z.to_string())
                .or_default()
                .ecp_potentials
                .get_or_insert_with(Default::default)
                .push(ecp_pot);
        }
    }

    Ok(())
}

pub fn read_demon2k(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    let basis_lines: Vec<String> = basis_str.lines().map(|s| s.trim().to_string()).collect();

    // Remove comments
    let basis_lines = helpers::prune_lines(&basis_lines, "#", true, true);

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

    let mut basis_dict = BseBasisMinimal {
        molssi_bse_schema: BseMolssiBseSchema { schema_type: "minimal".to_string(), schema_version: "0.1".to_string() },
        elements: HashMap::new(),
        function_types: Vec::new(),
        name: "unknown_basis".to_string(),
        description: "no_description".to_string(),
    };

    // Split orbital basis into element sections
    let orbital_sections = helpers::partition_lines(
        &basis_lines,
        |x| ORBITAL_RE.is_match(x),
        0,
        None,
        None,
        None,
        3, // minimum size
        true,
    )?;

    for section in &orbital_sections {
        // Verify this section starts with an orbital header
        if !section.is_empty() && ORBITAL_RE.is_match(&section[0]) {
            parse_electron_lines(&mut basis_dict.elements, section)?;
        }
    }

    // Split ECP sections - look for lines starting with ECP
    let ecp_sections = helpers::partition_lines(
        &basis_lines,
        |x| ECP_START_RE.is_match(x),
        0,
        None,
        None,
        None,
        2, // minimum size: "ECP" + at least one entry
        true,
    )?;

    for section in &ecp_sections {
        // Check if this section contains ECP data (starts with "ECP" line)
        if !section.is_empty() && ECP_START_RE.is_match(&section[0]) {
            parse_ecp_lines(&mut basis_dict.elements, section)?;
        }
    }

    let function_types = compose::whole_basis_types(&basis_dict.elements);
    basis_dict.function_types = function_types;

    Ok(basis_dict)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_demon2k() {
        let args = BseGetBasisArgsBuilder::default().elements("H, C-O".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("cc-pVDZ", "demon2k", args);
        let basis = read_demon2k(&basis_str).unwrap();
        println!("{basis:#?}");
    }

    #[test]
    fn test_read_demon2k_ecp() {
        let args = BseGetBasisArgsBuilder::default().elements("49-51".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("def2-ECP", "demon2k", args);
        let basis = read_demon2k(&basis_str).unwrap();
        println!("{basis:#?}");
    }
}
