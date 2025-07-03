//! Reader for the Gaussian94 format

use crate::prelude::*;
use crate::readers::helpers;

lazy_static::lazy_static! {
    static ref ELEMENT_RE: Regex = Regex::new(r"^-?([A-Za-z]{1,3})(?:\s+0)?$").unwrap();
    static ref ECP_SHELL_START_RE: Regex = Regex::new(r"^([A-Za-z]{1,3})\s+(\d+)\s+(\d+)$").unwrap();
    static ref ECP_AM_NELEC_RE: Regex = Regex::new(r"^\S+\s+(\d+)\s+(\d+)$").unwrap();
    static ref AM_LINE_RE: Regex =
        Regex::new(&format!(r"^([A-Za-z]+)\s+(\d+)((?:\s+{})+)$", helpers::FLOATING_RE.as_str())).unwrap();
    static ref EXPLICIT_AM_LINE_RE: Regex =
        Regex::new(&format!(r"^\s*L=(\d+)\s+(\d+)((?:\s+{})+)$", helpers::FLOATING_RE.as_str())).unwrap();
}

/// Parses lines representing all the electron shells for a single element.
fn parse_electron_lines(
    elements: &mut HashMap<String, BseBasisElement>,
    basis_lines: &[String],
) -> Result<(), BseError> {
    // Last line should be "****"
    let last = basis_lines.last().map_or(bse_raise!(ValueError, "Empty basis lines"), Ok)?;
    if last != "****" {
        bse_raise!(ValueError, "Electron shell is missing terminating ****")?
    }

    // First line is "{element} 0"
    let element_sym = basis_lines[0]
        .split_whitespace()
        .next()
        .map_or(bse_raise!(ValueError, "Invalid element line: {}", basis_lines[0]), Ok)?;

    // In the format, if the element symbol starts with a dash, then Gaussian does
    // not crash with an error in case this element does not exist in the
    // molecule input (this is what you need for system basis set libraries).
    let element_sym = element_sym.trim_start_matches('-');
    let element_Z = lut::element_Z_from_sym(element_sym)
        .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_sym), Ok)?;

    // After that come shells. We determine the start of a shell
    // by if the line starts with an angular momentum (a non-numeric character)
    let shell_blocks = helpers::partition_lines(
        &basis_lines[1..basis_lines.len() - 1], // Skip first and last lines
        |x| x.chars().next().is_some_and(|c| c.is_alphabetic()),
        0,
        None,
        None,
        None,
        1,
        true,
    )?;

    for sh_lines in shell_blocks {
        let (shell_am, nprim, scaling_factors) = if AM_LINE_RE.is_match(&sh_lines[0]) {
            let parsed = helpers::parse_line_regex(&AM_LINE_RE, &sh_lines[0], "Shell AM, nprim, scaling")?;
            let shell_am = lut::amchar_to_int(&parsed[0], true)
                .map_or(bse_raise!(ValueError, "Unknown angular momentum: {}", parsed[0]), Ok)?;
            (shell_am, parsed[1].clone(), parsed[2].clone())
        } else if EXPLICIT_AM_LINE_RE.is_match(&sh_lines[0]) {
            let parsed = helpers::parse_line_regex(&EXPLICIT_AM_LINE_RE, &sh_lines[0], "Shell AM, nprim, scaling")?;
            let shell_am =
                vec![parsed[0].parse().map_or(bse_raise!(ValueError, "Invalid angular momentum: {}", parsed[0]), Ok)?];
            (shell_am, parsed[1].clone(), parsed[2].clone())
        } else {
            return bse_raise!(ValueError, "Failed to parse shell block starting on line: {}", sh_lines[0]);
        };

        // Determine shell type
        let func_type = lut::function_type_from_am(&shell_am, "gto", "spherical");

        // Handle gaussian scaling factors
        // The square of the scaling factor is applied to exponents.
        // Typically they are 1.0, but not always
        let scaling_factors = helpers::replace_d(&scaling_factors);
        let mut scaling_factors: Vec<f64> = scaling_factors
            .split_whitespace()
            .map(|s| s.parse().map_err(|_| BseError::ValueError(format!("Invalid scaling factor: {s}"))))
            .collect::<Result<_, _>>()?;

        // Remove any scaling factors that are 0.0
        scaling_factors.retain(|&x| x != 0.0);

        // We should always have at least one scaling factor
        if scaling_factors.is_empty() {
            bse_raise!(ValueError, "No scaling factors given for element {element_sym}: Line: {}", sh_lines[0])?;
        }

        // There can be multiple scaling factors, but we don't handle that. It seems to
        // be very rare
        if scaling_factors.len() > 1 {
            bse_raise!(NotImplementedError, "Number of scaling factors > 1")?;
        }

        let scaling_factor = scaling_factors[0].powi(2);
        let has_scaling = (scaling_factor - 1.0).abs() > f64::EPSILON;

        // How many columns of coefficients do we have?
        // Gaussian doesn't support general contractions, so only >1 if
        // you have a fused shell
        let ngen = shell_am.len();
        let nprim = nprim.parse().map_or(bse_raise!(ValueError, "Invalid nprim value: {nprim}"), Ok)?;

        // Now read the exponents and coefficients
        let (mut exponents, coefficients) =
            helpers::parse_primitive_matrix(&sh_lines[1..], Some(nprim), Some(ngen), None)?;

        // If there is a scaling factor, apply it
        // But we keep track of some significant-figure type stuff (as best we can)
        if has_scaling {
            exponents = exponents
                .into_iter()
                .map(|ex| {
                    let ex_val: f64 = ex.parse().map_or(bse_raise!(ValueError, "Invalid exponent: {ex}"), Ok)?;
                    let scaled = ex_val * scaling_factor;
                    let formatted = format!("{scaled:.16E}");
                    let mut parts = formatted.split('E');
                    let mantissa = parts.next().unwrap();
                    let exponent = parts.next().unwrap();

                    let trimmed_mantissa = mantissa.trim_end_matches('0');
                    let mantissa = if trimmed_mantissa.ends_with('.') {
                        format!("{trimmed_mantissa}0")
                    } else {
                        trimmed_mantissa.to_string()
                    };

                    Ok(format!("{mantissa}E{exponent}"))
                })
                .collect::<Result<Vec<String>, BseError>>()?;
        }

        let shell = BseElectronShell {
            function_type: func_type,
            region: "".to_string(),
            angular_momentum: shell_am,
            exponents,
            coefficients,
        };

        elements.entry(element_Z.to_string()).or_default().electron_shells.get_or_insert_default().push(shell);
    }

    Ok(())
}

/// Parses lines representing all the ECP potentials for a single element.
fn parse_ecp_lines(elements: &mut HashMap<String, BseBasisElement>, basis_lines: &[String]) -> Result<(), BseError> {
    // First line is "{element} 0", with the zero being optional
    let element_sym = basis_lines[0]
        .split_whitespace()
        .next()
        .map_or(bse_raise!(ValueError, "Invalid element line: {}", basis_lines[0]), Ok)?;
    let element_Z = lut::element_Z_from_sym(element_sym)
        .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_sym), Ok)?;

    // Second line is information about the ECP
    let parsed = helpers::parse_line_regex(&ECP_AM_NELEC_RE, &basis_lines[1], "ECP max_am, nelec")?;
    let max_am = parsed[0].parse().map_or(bse_raise!(ValueError, "Invalid max_am value: {}", parsed[0]), Ok)?;
    let ecp_electrons = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid nelec value: {}", parsed[1]), Ok)?;

    // Partition all the potentials
    // Look for lines containing only an integer, but include the line
    // before it (is a comment line)
    let ecp_blocks = helpers::partition_lines(&basis_lines[2..], helpers::is_integer, 1, None, None, None, 1, true)?;

    for pot_lines in ecp_blocks {
        // first line is comment
        // second line is number of lines

        // Check that the number of lines is consistent
        let nlines: i32 = pot_lines[1]
            .parse()
            .map_or(bse_raise!(ValueError, "Number of lines for potential is not an integer: {}", pot_lines[1]), Ok)?;

        if nlines <= 0 {
            bse_raise!(ValueError, "Number of lines for potential is <= 0")?;
        }

        if pot_lines.len() as i32 != (nlines + 2) {
            bse_raise!(ValueError, "Number of lines is incorrect. Expected {nlines}, got {}", pot_lines.len() - 2)?;
        }

        let ecp_data = helpers::parse_ecp_table(&pot_lines[2..], &["r_exp", "g_exp", "coeff"], None)?;

        let ecp_pot = BseEcpPotential {
            angular_momentum: vec![],
            coefficients: ecp_data.coeff,
            ecp_type: "scalar_ecp".to_string(),
            r_exponents: ecp_data.r_exp,
            gaussian_exponents: ecp_data.g_exp,
        };

        elements.entry(element_Z.to_string()).or_default().ecp_potentials.get_or_insert_default().push(ecp_pot);
    }

    // Determine the AM of the potentials
    // Highest AM first, then the rest in order
    let all_pot_am = helpers::potential_am_list(max_am);

    // Were there as many potentials as we thought there should be?
    let element_data = elements.get_mut(&element_Z.to_string()).unwrap();
    let ecp_potentials = element_data.ecp_potentials.as_mut().unwrap();

    if all_pot_am.len() != ecp_potentials.len() {
        bse_raise!(
            ValueError,
            "Found incorrect number of potentials for {element_sym}: Expected {}, got {}",
            all_pot_am.len(),
            ecp_potentials.len()
        )?;
    }

    for (idx, pot) in ecp_potentials.iter_mut().enumerate() {
        pot.angular_momentum = vec![all_pot_am[idx]];
    }

    // Set the number of electrons
    elements.entry(element_Z.to_string()).or_default().ecp_electrons = Some(ecp_electrons);

    Ok(())
}

pub fn read_g94(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    // Removes comments
    let basis_lines =
        helpers::prune_lines(&basis_str.lines().map(|s| s.trim().to_string()).collect_vec(), "!", true, true);

    // Empty file?
    if basis_lines.is_empty() {
        return Ok(BseBasisMinimal::default());
    }

    let mut basis_dict = BseBasisMinimal {
        molssi_bse_schema: BseMolssiBseSchema { schema_type: "minimal".to_string(), schema_version: "0.1".to_string() },
        elements: HashMap::new(),
        function_types: Vec::new(),
        name: "unknown_basis".to_string(),
        description: "no_description".to_string(),
    };

    // split into element sections (may be electronic or ecp)
    let element_sections =
        helpers::partition_lines(&basis_lines, |x| ELEMENT_RE.is_match(x), 0, None, None, None, 3, true)?;

    for es in element_sections {
        // Try to guess if this is an ecp
        // Each element block starts with the element symbol
        // If the number of lines > 3, and the 4th line is just an integer, then it is
        // an ECP
        if es.len() > 3 && helpers::is_integer(&es[3]) {
            parse_ecp_lines(&mut basis_dict.elements, &es)?;
        } else {
            parse_electron_lines(&mut basis_dict.elements, &es)?;
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
    fn test_read_g94() {
        let args = BseGetBasisArgsBuilder::default().elements("H, O".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("cc-pVDZ", "gaussian94", args);
        let basis = read_g94(&basis_str).unwrap();
        println!("{basis:#?}");
    }
}
