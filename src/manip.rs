//! Common basis set manipulations
//!
//! This module contains functions for uncontracting and merging basis set data,
//! as well as some other small functions.

use crate::prelude::*;

/// Removes exact duplicates of primitives, and condenses duplicate exponents
/// into general contractions.
///
/// Also removes primitives if all coefficients are zero.
pub fn prune_shell(shell: &BseElectronShell) -> BseElectronShell {
    let mut new_exponents = Vec::new();
    let mut new_coefficients = Vec::new();

    let exponents = &shell.exponents;
    let nprim = exponents.len();

    // transpose of the coefficient matrix
    let coeff_t: Vec<Vec<String>> = (0..shell.coefficients[0].len())
        .map(|i| shell.coefficients.iter().map(|row| row[i].clone()).collect())
        .collect();

    // Group by exponents
    let mut ex_groups: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for i in 0..nprim {
        let mut found = false;
        for ex in &mut ex_groups {
            if exponents[i].parse::<f64>().unwrap() == ex.0.parse::<f64>().unwrap() {
                ex.1.push(coeff_t[i].clone());
                found = true;
                break;
            }
        }
        if !found {
            ex_groups.push((exponents[i].clone(), vec![coeff_t[i].clone()]));
        }
    }

    // Now collapse within groups
    for ex in ex_groups {
        if ex.1.len() == 1 {
            // only add if there is a nonzero contraction coefficient
            if !ex.1[0].iter().all(|x| x.parse::<f64>().unwrap() == 0.0) {
                new_exponents.push(ex.0);
                new_coefficients.push(ex.1[0].clone());
            }
            continue;
        }

        // ex.1 contains rows of coefficients. The length of ex.1
        // is the number of times the exponent is duplicated. Columns represent general
        // contractions. We want to find the non-zero coefficient in each
        // column, if it exists The result is a single row with a length
        // representing the number of general contractions

        let mut new_coeff_row = Vec::new();

        // so take yet another transpose.
        let ex_coeff: Vec<Vec<String>> =
            (0..ex.1[0].len()).map(|i| ex.1.iter().map(|row| row[i].clone()).collect()).collect();

        for g in ex_coeff {
            let nonzero: Vec<String> = g.iter().filter(|x| x.parse::<f64>().unwrap() != 0.0).cloned().collect();

            if nonzero.len() > 1 {
                panic!("Exponent {} is duplicated within a contraction", ex.0);
            }

            if nonzero.is_empty() {
                new_coeff_row.push(g[0].clone());
            } else {
                new_coeff_row.push(nonzero[0].clone());
            }
        }

        // only add if there is a nonzero contraction coefficient anywhere for this
        // exponent
        if !new_coeff_row.iter().all(|x| x.parse::<f64>().unwrap() == 0.0) {
            new_exponents.push(ex.0);
            new_coefficients.push(new_coeff_row);
        }
    }

    // take the transpose again, putting the general contraction
    // as the slowest index
    let final_coefficients: Vec<Vec<String>> = if !new_coefficients.is_empty() {
        (0..new_coefficients[0].len()).map(|i| new_coefficients.iter().map(|row| row[i].clone()).collect()).collect()
    } else {
        Vec::new()
    };

    BseElectronShell {
        function_type: shell.function_type.clone(),
        region: shell.region.clone(),
        angular_momentum: shell.angular_momentum.clone(),
        exponents: new_exponents,
        coefficients: final_coefficients,
    }
}

pub fn prune_basis(basis: &mut BseBasis) {
    // Removes primitives that have a zero coefficient, and
    // removes duplicate primitives and shells
    //
    // This only finds EXACT duplicates, and is meant to be used
    // after other manipulations

    for (_, el) in basis.elements.iter_mut() {
        let shells = match &mut el.electron_shells {
            Some(shells) => shells,
            None => continue,
        };

        // Prune each shell
        for shell in shells.iter_mut() {
            prune_shell(shell);
        }

        // Remove duplicates
        let mut unique_shells = Vec::new();
        for shell in shells.drain(..) {
            if !unique_shells.contains(&shell) {
                unique_shells.push(shell);
            }
        }

        *shells = unique_shells;
    }
}

/// Removes sp, spd, spdf, etc, contractions from a basis set.
///
/// The general contractions are replaced by uncontracted versions
///
/// Contractions up to max_am will be left in place. For example,
/// if max_am = 1, spd will be split into sp and d
///
/// The input basis set is modified directly.
/// The returned basis may have functions with coefficients of zero and may
/// have duplicate shells.
pub fn uncontract_spdf(basis: &mut BseBasis, max_am: i32) {
    for (_, el) in basis.elements.iter_mut() {
        let Some(electron_shells) = &mut el.electron_shells else {
            continue;
        };

        let mut newshells = Vec::new();

        for sh in electron_shells.iter() {
            // am will be a list
            let am = &sh.angular_momentum;
            let coeff = &sh.coefficients;

            // if this is an sp, spd,... orbital
            if am.len() > 1 {
                let mut newsh = sh.clone();
                newsh.angular_momentum = Vec::new();
                newsh.coefficients = Vec::new();

                let ngen = sh.coefficients.len();
                for g in 0..ngen {
                    if am[g] > max_am {
                        let mut newsh2 = sh.clone();
                        newsh2.angular_momentum = vec![am[g]];
                        newsh2.coefficients = vec![coeff[g].clone()];
                        newshells.push(newsh2);
                    } else {
                        newsh.angular_momentum.push(am[g]);
                        newsh.coefficients.push(coeff[g].clone());
                    }
                }

                newshells.insert(0, newsh);
            } else {
                newshells.push(sh.clone());
            }
        }

        *electron_shells = newshells;
    }
}

/// Removes the general contractions from a basis set
///
/// The input basis set is modified in place. The resulting basis
/// may have functions with coefficients of zero and may have duplicate
/// shells.
pub fn uncontract_general(basis: &mut BseBasis) {
    for (_, el) in basis.elements.iter_mut() {
        let Some(ref mut electron_shells) = el.electron_shells else {
            continue;
        };

        let mut new_shells = Vec::new();

        for shell in electron_shells.iter() {
            // See if we actually have to uncontract
            // Also, don't uncontract sp, spd,.... orbitals
            //      (leave that to uncontract_spdf)
            if shell.coefficients.len() == 1 || shell.angular_momentum.len() > 1 {
                new_shells.push(shell.clone());
            } else if shell.angular_momentum.len() == 1 {
                for coeff in &shell.coefficients {
                    // clone, then replace 'coefficients'
                    let mut new_shell = shell.clone();
                    new_shell.coefficients = vec![coeff.clone()];
                    new_shells.push(new_shell);
                }
            }
        }

        *electron_shells = new_shells;
    }
}

/// Makes one large general contraction for each angular momentum
///
/// The output of this function is not pretty. If you want to make it nicer,
/// use sort_basis afterwards.
pub fn make_general(basis: &mut BseBasis, skip_spdf: bool) {
    let zero = "0.00000000";

    if !skip_spdf {
        uncontract_spdf(basis, 0);
    }

    for (_, el) in basis.elements.iter_mut() {
        let electron_shells = match &mut el.electron_shells {
            Some(shells) => shells,
            None => continue,
        };

        let mut newshells = Vec::new();

        // See what we have
        let mut all_am = Vec::new();
        for sh in electron_shells.iter() {
            let am = &sh.angular_momentum;

            // Skip sp shells
            if am.len() > 1 {
                newshells.push(sh.clone());
                continue;
            }

            if !all_am.contains(am) {
                all_am.push(am.clone());
            }
        }

        all_am.sort();

        for am in all_am {
            let mut newsh = BseElectronShell {
                angular_momentum: am.clone(),
                exponents: Vec::new(),
                coefficients: Vec::new(),
                region: String::new(),
                function_type: String::new(),
            };

            // Do exponents first
            for sh in electron_shells.iter() {
                if sh.angular_momentum == am {
                    newsh.exponents.extend(sh.exponents.clone());
                }
            }

            // Number of primitives in the new shell
            let nprim = newsh.exponents.len();

            let mut cur_prim = 0;
            for sh in electron_shells.iter() {
                if sh.angular_momentum != am {
                    continue;
                }

                if newsh.function_type.is_empty() {
                    newsh.function_type = sh.function_type.clone();
                }

                // Make sure the shells we are merging have the same function types
                let ft1 = &newsh.function_type;
                let ft2 = &sh.function_type;

                // Check if one function type is the subset of another
                // (should handle gto/gto_spherical, etc)
                if !ft1.contains(ft2) && !ft2.contains(ft1) {
                    panic!("Cannot make general contraction of different function types");
                }

                let ngen = sh.coefficients.len();

                for g in 0..ngen {
                    let mut coef = vec![zero.to_string(); cur_prim];
                    coef.extend(sh.coefficients[g].clone());
                    coef.extend(vec![zero.to_string(); nprim - coef.len()]);
                    newsh.coefficients.push(coef);
                }

                cur_prim += sh.exponents.len();
            }

            newshells.push(newsh);
        }

        el.electron_shells = Some(newshells);
    }

    // If the basis was read in from a segmented format, it will have
    // duplicate primitives, and so a pruning is necessary
    prune_basis(basis);
}

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
