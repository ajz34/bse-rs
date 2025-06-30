//! Common basis set manipulations
//!
//! This module contains functions for uncontracting and merging basis set data,
//! as well as some other small functions.

use crate::prelude::*;

/// Check if there is only one non-zero coefficient in a vector.
/// This function is used to determine if two basis shares the same exponents
/// but different coefficients.
fn is_single_column(col: &[String]) -> bool {
    col.iter().filter(|s| s.parse::<f64>().unwrap() != 0.0).count() == 1
}

/// Removes any free primitives from a basis set as a way to generate a minimal
/// basis.
///
/// The input basis set is not modified. The returned basis may have functions
/// with coefficients of zero and may have duplicate shells.
pub fn remove_free_primitives(basis: &mut BseBasis) {
    for (_, el) in basis.elements.iter_mut() {
        if el.electron_shells.is_none() {
            continue;
        }

        let mut new_shells: Vec<BseElectronShell> = vec![];
        for sh in el.electron_shells.as_mut().unwrap() {
            // find contractions
            let coefficients = sh.coefficients.iter().filter(|&col| !is_single_column(col)).cloned().collect_vec();
            if !coefficients.is_empty() {
                sh.coefficients = coefficients;
                new_shells.push(sh.clone());
            }
        }
        el.electron_shells = Some(new_shells);
    }

    // TODO: prune_basis
}
