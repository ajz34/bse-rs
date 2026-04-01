//! Writer for BSE debug/dump format.

use crate::prelude::*;

/// Format an electron shell for debug output.
fn electron_shell_str(shell: &BseElectronShell, shellidx: Option<usize>) -> String {
    let am = &shell.angular_momentum;
    let amchar = lut::amint_to_char(am, HIK).to_uppercase();

    let shellidx_str = if let Some(idx) = shellidx { format!("Index {}  ", idx) } else { String::new() };

    let exponents = &shell.exponents;
    let coefficients = &shell.coefficients;
    let ncol = coefficients.len() + 1;

    let region = if shell.region.is_empty() { "(none)" } else { &shell.region };

    let point_places: Vec<usize> = (1..=ncol).map(|i| 8 * i + 15 * (i - 1)).collect();

    let mut s = format!("Shell: {}Region: {} AM: {}\n", shellidx_str, region, amchar);
    s.push_str(&format!("Function: {}\n", shell.function_type));

    // Combine exponents and coefficients into a matrix
    let mut mat: Vec<Vec<String>> = vec![exponents.clone()];
    mat.extend(coefficients.clone());
    s.push_str(&printing::write_matrix(&mat, &point_places, false));
    s.push('\n');

    s
}

/// Format an ECP potential for debug output.
fn ecp_pot_str(pot: &BseEcpPotential) -> String {
    let am = &pot.angular_momentum;
    let amchar = lut::amint_to_char(am, HIK);

    let rexponents = &pot.r_exponents;
    let gexponents = &pot.gaussian_exponents;
    let coefficients = &pot.coefficients;

    // Format r_exponents as strings
    let rexp_str: Vec<String> = rexponents.iter().map(|r| r.to_string()).collect();

    let point_places = vec![0, 10, 33];

    let mut s = format!("Potential: {} potential\n", amchar);
    s.push_str(&format!("Type: {}\n", pot.ecp_type));

    // Build matrix: [r_exponents, gaussian_exponents, coefficients]
    let mut mat: Vec<Vec<String>> = vec![rexp_str, gexponents.clone()];
    mat.extend(coefficients.clone());
    s.push_str(&printing::write_matrix(&mat, &point_places, false));
    s.push_str("\n\n");

    s
}

/// Format element data for debug output.
fn element_data_str(z: &str, eldata: &BseBasisElement) -> String {
    let sym = lut::element_sym_from_Z_with_normalize(z.parse().unwrap()).unwrap();

    let cont_str = if let Some(shells) = &eldata.electron_shells {
        misc::contraction_string(shells, HIK, INCOMPACT)
    } else {
        "(no electron shells)".to_string()
    };

    let mut s = format!("\nElement: {} : {}\n", sym, cont_str);

    if let Some(shells) = &eldata.electron_shells {
        for (shellidx, shell) in shells.iter().enumerate() {
            s.push_str(&electron_shell_str(shell, Some(shellidx)));
            s.push('\n');
        }
    }

    if let Some(ecp_potentials) = &eldata.ecp_potentials {
        s.push_str(&format!("ECP: Element: {}   Number of electrons: {}\n", sym, eldata.ecp_electrons.unwrap()));

        for pot in ecp_potentials {
            s.push_str(&ecp_pot_str(pot));
        }
    }

    s
}

/// Converts a basis set to BSE debug format.
pub fn write_bsedebug(basis: &BseBasis) -> String {
    let mut s = String::new();

    for (el, eldata) in basis.elements.iter().sorted_by_key(|(z, _)| z.parse::<i32>().unwrap_or(0)) {
        s.push_str(&element_data_str(el, eldata));
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_bsedebug() {
        let args = BseGetBasisArgsBuilder::default().elements("1, 49".to_string()).build().unwrap();
        let basis = get_basis("def2-TZVP", args);
        let output = write_bsedebug(&basis);
        println!("{output}");
    }

    #[test]
    fn test_write_bsedebug_no_ecp() {
        let args = BseGetBasisArgsBuilder::default().elements("1-10".to_string()).build().unwrap();
        let basis = get_basis("cc-pVTZ", args);
        let output = write_bsedebug(&basis);
        println!("{output}");
    }
}
