//! Reader for the Dalton format

use crate::prelude::*;
use crate::readers::helpers;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref SHELL_BEGIN_RE: Regex = Regex::new(r"^(?:[hH]\s+)?(\d+)\s+(\d+)(?: +0)?$").unwrap();
    static ref ELEMENT_BEGIN_RE: Regex = Regex::new(r"^!\s+([a-z]+)\s+\(.*\)\s*->\s*\[.*\]$").unwrap();
}

/// List of all element names (lowercase) including "wolffram" for tungsten
fn all_element_names() -> Vec<String> {
    let mut names = lut::all_element_names();
    names.push("wolffram".to_string());
    names
}

/// Determines if a line begins an element block
fn line_begins_element(line: &str) -> Result<bool, BseError> {
    if line.is_empty() {
        return Ok(false);
    }

    let line_lower = line.to_lowercase();

    if line_lower.starts_with("a ") {
        return Ok(true);
    }

    if ELEMENT_BEGIN_RE.is_match(line) {
        let element_name = ELEMENT_BEGIN_RE.captures(line).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");

        if all_element_names().contains(&element_name.to_string()) {
            Ok(true)
        } else {
            bse_raise!(ValueError, "Line looks to start an element, but element name is unknown to me. Line: {}", line)
        }
    } else {
        Ok(false)
    }
}

/// Parses lines representing all the electron shells for all elements
fn parse_electron_lines(
    elements: &mut HashMap<String, BseBasisElement>,
    basis_lines: &[String],
) -> Result<(), BseError> {
    // Fix common spelling mistakes
    let basis_lines: Vec<String> = basis_lines.iter().map(|line| line.replace("PHOSPHOROUS", "PHOSPHORUS")).collect();

    // A little bit of a hack here
    // If we find the start of an element, remove all the following comment lines
    let mut new_basis_lines = Vec::new();
    let mut i = 0;
    while i < basis_lines.len() {
        if line_begins_element(&basis_lines[i])? {
            new_basis_lines.push(basis_lines[i].clone());
            i += 1;
            while i < basis_lines.len() && basis_lines[i].starts_with('!') {
                i += 1;
            }
        } else {
            new_basis_lines.push(basis_lines[i].clone());
            i += 1;
        }
    }

    let basis_lines = helpers::prune_lines(&new_basis_lines, "$", true, true);

    // Now split out all the element blocks
    let element_blocks = helpers::partition_lines(
        &basis_lines,
        |x| line_begins_element(x).unwrap_or(false),
        0,
        None,
        None,
        None,
        3,
        true,
    )?;

    // For each block, split out all the shells
    let parse_rex = Regex::new(r"a +(\d+) *$").unwrap();
    for el_lines in element_blocks {
        // Figure out which type of block this is (does it start with 'a ' or a comment)
        let header = el_lines[0].to_lowercase();
        let element_Z = if header.starts_with("a ") {
            let parsed = helpers::parse_line_regex(&parse_rex, &header, "a {element_z}")?;
            parsed[0].clone()
        } else if header.starts_with('!') {
            let parsed = helpers::parse_line_regex(&ELEMENT_BEGIN_RE, &header, "! {element_name}")?;
            lut::element_Z_from_name(&parsed[0])
                .map_or(bse_raise!(ValueError, "Unknown element name: {}", parsed[0]), Ok)?
                .to_string()
        } else {
            return bse_raise!(ValueError, "Unable to parse block in dalton: header line is \"{}\"", header);
        };

        let mut el_lines = el_lines[1..].to_vec();

        // Remove all the rest of the comment lines
        el_lines = helpers::prune_lines(&el_lines, "!", true, true);

        // Now partition again into blocks of shells for this element
        let shell_blocks =
            helpers::partition_lines(&el_lines, |x| SHELL_BEGIN_RE.is_match(x), 0, None, None, None, 1, true)?;

        // Shells are written in increasing angular momentum

        for (shell_am, sh_lines) in shell_blocks.into_iter().enumerate() {
            let shell_am = shell_am as i32; // Convert to i32 for consistency with BseElectronShell
            let parsed = helpers::parse_line_regex(&SHELL_BEGIN_RE, &sh_lines[0], "nprim, ngen")?;
            let nprim: usize = parsed[0].parse().map_or(bse_raise!(ValueError, "Invalid nprim: {}", parsed[0]), Ok)?;
            let ngen: usize = parsed[1].parse().map_or(bse_raise!(ValueError, "Invalid ngen: {}", parsed[1]), Ok)?;

            let mut bas_lines = sh_lines[1..].to_vec();

            // fix for split over newline
            if nprim > 0 && !bas_lines.is_empty() {
                let num_line_splits = bas_lines.len() / nprim;
                if num_line_splits * nprim == bas_lines.len() {
                    bas_lines = (0..nprim)
                        .map(|i| {
                            (0..num_line_splits)
                                .map(|offset| bas_lines[num_line_splits * i + offset].as_str())
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect();
                }
            }

            let (exponents, coefficients) = helpers::parse_primitive_matrix(&bas_lines, Some(nprim), Some(ngen), None)?;

            let function_type = lut::function_type_from_am(&[shell_am], "gto", "spherical");

            let shell = BseElectronShell {
                function_type,
                region: "".to_string(),
                angular_momentum: vec![shell_am],
                exponents,
                coefficients,
            };

            elements.entry(element_Z.clone()).or_default().electron_shells.get_or_insert_default().push(shell);
        }
    }

    Ok(())
}

pub fn read_dalton(basis_str: &str) -> Result<BseBasisMinimal, BseError> {
    // We need to leave in comments until later, since they can be significant
    // (one format allows "! {ELEMENT}" to start an element block)
    // But we still prune blank lines
    let mut basis_lines =
        helpers::prune_lines(&basis_str.lines().map(|s| s.trim().to_string()).collect_vec(), "", true, true);

    let mut basis_dict = BseBasisMinimal {
        molssi_bse_schema: BseMolssiBseSchema { schema_type: "minimal".to_string(), schema_version: "0.1".to_string() },
        elements: HashMap::new(),
        function_types: Vec::new(),
        name: "unknown_basis".to_string(),
        description: "no_description".to_string(),
    };

    // Skip forward until either:
    // 1. Line begins with 'a'
    // 2. Line begins with 'ecp'
    // 3. Lines begins with '!', with an element name following
    while !basis_lines.is_empty()
        && !line_begins_element(&basis_lines[0])?
        && !basis_lines[0].eq_ignore_ascii_case("ecp")
    {
        basis_lines.remove(0);
    }

    // Empty file?
    if basis_lines.is_empty() {
        return Ok(basis_dict);
    }

    // Partition into ECP and electron blocks
    // I don't think Dalton supports ECPs, but the original BSE
    // Used the NWChem output format for the ECP part
    let basis_sections =
        helpers::partition_lines(&basis_lines, |x| x.eq_ignore_ascii_case("ecp"), 0, None, Some(1), Some(2), 1, true)?;

    for s in basis_sections {
        if s[0].eq_ignore_ascii_case("ecp") {
            bse_raise!(NotImplementedError, "ECPs are not supported in Dalton format. Section: {s:?}")?;
        } else {
            parse_electron_lines(&mut basis_dict.elements, &s)?;
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
    fn test_read_dalton() {
        let args = BseGetBasisArgsBuilder::default().elements("H, O".to_string()).build().unwrap();
        let basis_str = get_formatted_basis("cc-pVDZ", "dalton", args);
        let basis = read_dalton(&basis_str).unwrap();
        println!("{basis:#?}");
    }
}
