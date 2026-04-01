//! Reader for Molpro system library format (libmol).

use crate::prelude::*;
use crate::readers::helpers;

lazy_static::lazy_static! {
    // Shell entry: 'element am (aliases) : nprim ncontr start1.end1 start2.end2 ...'
    // We capture the whole line and parse manually
    static ref ELEMENT_SHELL_RE: Regex = Regex::new(
        r"^\s*(?P<sym>\w+)\s+(?P<am>[spdfghikSPDFGHIK])\s+.*:\s*(?P<nprim>\d+)\s+(?P<ncontr>\d+)\s+(?P<ranges>.*)$"
    ).unwrap();
    // Exponent/coefficient entry: floating point values
    static ref ENTRY_RE: Regex = Regex::new(&format!(
        r"^\s*(?:\s*(?P<val>({}|{}))\s*)+\s*$",
        helpers::FLOATING_RE.as_str(), helpers::INTEGER_RE.as_str())
    ).unwrap();
    // ECP entry: symbol ECP names : ncore lmax lmaxso ndata
    static ref ECP_RE: Regex = Regex::new(
        r"^\s*(?P<sym>\w+)\s+ECP\s+.*:\s*(?P<ncore>\d+)\s+(?P<lmax>\d+)\s+(?P<lmaxso>\d+)\s+(?P<ndata>\d+)\s*$"
    ).unwrap();
    // ECP block start: nterms (rexp1 expn1 coeff1) (rexp2 expn2 coeff2) ...
    static ref ECP_BLOCK_START_RE: Regex = Regex::new(
        &format!(r"^\s*(?P<nterms>\d+)(?:\s+(?P<rexp>\d+)\s+(?P<expn>{0})\s+(?P<coeff>{0}))+\s*$",
        helpers::FLOATING_RE.as_str())
    ).unwrap();
    // ECP block continuation
    static ref ECP_BLOCK_CONT_RE: Regex = Regex::new(
        &format!(r"^\s*(?:(?P<rexp>\d+)\s+(?P<expn>{0})\s+(?P<coeff>{0})\s*)+$",
        helpers::FLOATING_RE.as_str())
    ).unwrap();
}

// Default function type for libmol is spherical
const FUNC_TYPE: &str = "gto_spherical";

/// Reads a shell from the input.
fn read_shell(
    basis_lines: &[String],
    elements: &mut HashMap<String, BseBasisElement>,
    iline: usize,
) -> Result<usize, BseError> {
    // Parse shell entry using regex dict
    let shell =
        helpers::parse_line_regex_dict(&ELEMENT_SHELL_RE, &basis_lines[iline], "element am ... : nprim ncontr ranges")?;

    // Skip comment line
    let mut iline = iline + 1;

    // Get element symbol and angular momentum
    let element_sym = shell.get("sym").map_or("", |v| v.as_str());
    let amchar = shell.get("am").map_or("", |v| v.as_str());
    let nprim: usize = shell.get("nprim").map_or(0, |v| v.parse().unwrap_or(0));
    let ncontr: usize = shell.get("ncontr").map_or(0, |v| v.parse().unwrap_or(0));
    let ranges_str = shell.get("ranges").map_or("", |v| v.as_str());

    if nprim == 0 || ncontr == 0 {
        bse_raise!(ValueError, "Invalid nprim or ncontr in libmol format")?;
    }

    // Parse angular momentum
    let shell_am = lut::amchar_to_int(amchar, HIK).unwrap_or_default();
    if shell_am.len() != 1 {
        bse_raise!(ValueError, "Fused AM not supported in libmol reader")?;
    }

    // Parse contraction ranges (start.end pairs separated by whitespace)
    let cranges: Vec<(usize, usize)> = ranges_str
        .split_whitespace()
        .filter_map(|range_str| {
            let parts: Vec<&str> = range_str.split('.').collect();
            if parts.len() == 2 {
                let start: usize = parts[0].parse().ok()?;
                let end: usize = parts[1].parse().ok()?;
                Some((start, end))
            } else {
                None
            }
        })
        .collect();

    if cranges.len() != ncontr {
        eprintln!("Warning: Expected {} contraction ranges, found {}", ncontr, cranges.len());
    }

    // Get element Z
    let element_Z = lut::element_Z_from_sym(element_sym)
        .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_sym), Ok)?;

    // Calculate number of values to read
    let nread = nprim + cranges.iter().map(|(s, e)| e - s + 1).sum::<usize>();

    // Read in data
    let mut rawdata: Vec<String> = Vec::new();
    while rawdata.len() < nread {
        iline += 1;
        if iline >= basis_lines.len() {
            bse_raise!(ValueError, "Ran out of lines reading libmol shell data (need {} more)", nread - rawdata.len())?;
        }

        // Skip comment lines and empty lines
        let line = basis_lines[iline].trim();
        if line.is_empty() || line.starts_with('!') {
            continue;
        }

        // Skip lines that look like shell or ECP headers
        if ELEMENT_SHELL_RE.is_match(&basis_lines[iline]) || ECP_RE.is_match(&basis_lines[iline]) {
            // We've hit the next shell/ECP without finishing the current one
            eprintln!("Warning: Unexpected header line while reading shell data");
            break;
        }

        // Split by whitespace and parse as floating point numbers
        let parts: Vec<&str> = line.split_whitespace().collect();
        for part in parts {
            let val = helpers::replace_d(part);
            // Verify it's a valid floating point number
            if helpers::is_floating(&val) {
                // Ensure it has a decimal point
                let val = if val.contains('.') { val } else { format!("{}.0", val) };
                rawdata.push(val);
            }
        }
    }

    // Collect exponents
    let exponents: Vec<String> = rawdata[..nprim].to_vec();

    // Collect contraction coefficients with padding
    let mut coefficients: Vec<Vec<String>> = Vec::new();
    let mut offset = nprim;
    for (start, end) in &cranges {
        let nentries = end - start + 1;
        let cc: Vec<String> = rawdata[offset..offset + nentries].to_vec();
        offset += nentries;

        // Pad with zeros if needed
        let mut padded_cc = Vec::new();
        if *start > 1 {
            padded_cc.extend((1..*start).map(|_| "0.0".to_string()));
        }
        padded_cc.extend(cc);
        if *end < nprim {
            padded_cc.extend((*end..nprim).map(|_| "0.0".to_string()));
        }
        coefficients.push(padded_cc);
    }

    // Determine function type
    let func_type = if shell_am[0] < 2 { "gto" } else { FUNC_TYPE };

    let shell_data = BseElectronShell {
        function_type: func_type.to_string(),
        region: "".to_string(),
        angular_momentum: shell_am,
        exponents,
        coefficients,
    };

    elements
        .entry(element_Z.to_string())
        .or_default()
        .electron_shells
        .get_or_insert_with(Default::default)
        .push(shell_data);

    Ok(iline)
}

/// Reads an ECP from the input.
fn read_ecp(
    basis_lines: &[String],
    elements: &mut HashMap<String, BseBasisElement>,
    iline: usize,
) -> Result<usize, BseError> {
    // Parse ECP entry
    let ecp = helpers::parse_line_regex_dict(&ECP_RE, &basis_lines[iline], "symbol ECP ... : ncore lmax lmaxso ndata")?;

    let element_sym = ecp.get("sym").map_or("", |v| v.as_str());
    let ncore: i32 = ecp.get("ncore").map_or(0, |v| v.parse().unwrap_or(0));
    let lmax: i32 = ecp.get("lmax").map_or(0, |v| v.parse().unwrap_or(0));
    let lmaxso: i32 = ecp.get("lmaxso").map_or(0, |v| v.parse().unwrap_or(0));

    // Skip comment line
    let mut iline = iline + 1;

    let element_Z = lut::element_Z_from_sym(element_sym)
        .map_or(bse_raise!(ValueError, "Unknown element symbol: {}", element_sym), Ok)?;

    // Set ECP electrons
    elements.entry(element_Z.to_string()).or_default().ecp_electrons = Some(ncore);

    // Parse ECP data blocks
    // The order is: lmax, 0, 1, ..., lmax-1
    let am_order: Vec<i32> = [lmax].into_iter().chain(0..lmax).collect();

    for target_l in &am_order {
        iline += 1;
        if iline >= basis_lines.len() {
            break;
        }

        // Parse the ECP block line(s)
        // Format: nterms rexp1 expn1 coeff1 rexp2 expn2 coeff2 ...
        // Or continuation: rexp1 expn1 coeff1 rexp2 expn2 coeff2 ...

        let mut all_rexps: Vec<i32> = Vec::new();
        let mut all_gexps: Vec<String> = Vec::new();
        let mut all_coeffs: Vec<String> = Vec::new();

        // Read the first line of the block (has nterms at the beginning)
        let line = basis_lines[iline].trim();
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        // First value should be nterms
        let nterms: usize = match parts[0].parse() {
            Ok(n) => n,
            Err(_) => continue, // Line doesn't start with nterms, skip
        };

        // Parse remaining as triples
        let triples = &parts[1..];
        for chunk in triples.chunks(3) {
            if chunk.len() == 3 {
                if let Ok(rexp) = chunk[0].parse::<i32>() {
                    all_rexps.push(rexp);
                }
                all_gexps.push(helpers::replace_d(chunk[1]));
                all_coeffs.push(helpers::replace_d(chunk[2]));
            }
        }

        // Read continuation lines if needed
        while all_rexps.len() < nterms {
            iline += 1;
            if iline >= basis_lines.len() {
                break;
            }

            let cont_line = basis_lines[iline].trim();

            // Check if this is a continuation line or a new block
            // A continuation line starts with an integer (rexp), a new block starts with
            // nterms But both start with an integer... The difference is that
            // continuation lines are for the same block, so we just check if
            // we've read enough terms

            // Skip empty lines
            if cont_line.is_empty() {
                continue;
            }

            // Check if this line looks like a new block (starts with nterms followed by
            // valid triple)
            let cont_parts: Vec<&str> = cont_line.split_whitespace().collect();
            if cont_parts.len() >= 4 {
                // Could be a new block if first is integer and second is integer (rexp)
                // Or continuation if first is integer (rexp) and second is float (expn)
                if let (Ok(_), Ok(_)) = (cont_parts[0].parse::<i32>(), cont_parts[1].parse::<f64>()) {
                    // If second is an integer, this might be a new block
                    // Check if third is a float - if so, this is continuation
                    if cont_parts.len() >= 3 && helpers::is_floating(cont_parts[2]) {
                        // This is a continuation line: rexp expn coeff ...
                        for chunk in cont_parts.chunks(3) {
                            if chunk.len() == 3 {
                                if let Ok(rexp) = chunk[0].parse::<i32>() {
                                    all_rexps.push(rexp);
                                }
                                all_gexps.push(helpers::replace_d(chunk[1]));
                                all_coeffs.push(helpers::replace_d(chunk[2]));
                            }
                        }
                    } else {
                        // This looks like a new block, stop reading continuation
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Create and store the ECP potential
        let ecp_pot = BseEcpPotential {
            angular_momentum: vec![*target_l],
            coefficients: vec![all_coeffs],
            ecp_type: "scalar_ecp".to_string(),
            r_exponents: all_rexps,
            gaussian_exponents: all_gexps,
        };

        elements
            .entry(element_Z.to_string())
            .or_default()
            .ecp_potentials
            .get_or_insert_with(Default::default)
            .push(ecp_pot);
    }

    if lmaxso != 0 {
        eprintln!("Warning: Spin-orbit ECPs not yet supported in libmol reader");
    }

    Ok(iline)
}

pub fn read_libmol(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    let basis_lines: Vec<String> = basis_str.lines().map(|s| s.trim().to_string()).collect();

    // Remove comments
    let basis_lines = helpers::prune_lines(&basis_lines, "!", true, true);

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

    // Parse line by line
    let mut iline = 0;
    while iline < basis_lines.len() {
        if ELEMENT_SHELL_RE.is_match(&basis_lines[iline]) {
            iline = read_shell(&basis_lines, &mut basis_dict.elements, iline)?;
        } else if ECP_RE.is_match(&basis_lines[iline]) {
            iline = read_ecp(&basis_lines, &mut basis_dict.elements, iline)?;
        }
        iline += 1;
    }

    let function_types = compose::whole_basis_types(&basis_dict.elements);
    basis_dict.function_types = function_types;

    Ok(basis_dict)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_libmol() {
        let args = BseGetBasisArgsBuilder::default().elements("H, C-O".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("cc-pVDZ", "libmol", args);
        let basis = read_libmol(&basis_str).unwrap();
        println!("{basis:#?}");
    }

    #[test]
    fn test_read_libmol_ecp() {
        let args = BseGetBasisArgsBuilder::default().elements("49-51".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("def2-ECP", "libmol", args);
        let basis = read_libmol(&basis_str).unwrap();
        println!("{basis:#?}");
    }
}
