//! Functions related to composing basis sets from individual components.

use crate::prelude::*;

pub fn compose_table_basis(file_relpath: &str, data_dir: &str) -> BseBasis {
    compose_table_basis_f(file_relpath, data_dir).unwrap()
}

#[cached(key = "String", convert = r#"{ format!("{data_dir}/{file_relpath}") }"#)]
pub fn compose_table_basis_f(file_relpath: &str, data_dir: &str) -> Result<BseBasis, BseError> {
    // read skeleton table
    let skel_table = read_skel_table_file_f(file_relpath, data_dir)?;

    let mut basis_elements = HashMap::new();
    for (atomic_number, skl_element_realpath) in &skel_table.elements {
        // read skeleton element, to get the components
        let skel_element = read_skel_element_file_f(skl_element_realpath, data_dir)?;
        let mut basis_element =
            BseBasisElement { references: vec![], electron_shells: None, ecp_potentials: None, ecp_electrons: None };

        let component_realpaths = &skel_element.elements[atomic_number].components;
        for component_relpath in component_realpaths {
            if let Ok(gto_component) = read_skel_component_gto_file_f(component_relpath, data_dir) {
                // read skeleton component (gto) and parse
                let gto_element = &gto_component.elements[atomic_number];
                let gto_reference = BseBasisReference {
                    reference_keys: gto_element.references.clone(),
                    reference_description: gto_component.description.clone(),
                };
                basis_element.references.push(gto_reference);
                if basis_element.electron_shells.is_none() {
                    basis_element.electron_shells = Some(gto_element.electron_shells.clone());
                } else if let Some(shells) = &mut basis_element.electron_shells {
                    shells.extend_from_slice(&gto_element.electron_shells);
                }
            } else if let Ok(ecp_component) = read_skel_component_ecp_file_f(component_relpath, data_dir) {
                // read skeleton component (ecp) and parse
                let ecp_element = &ecp_component.elements[atomic_number];
                let ecp_reference = BseBasisReference {
                    reference_keys: ecp_element.references.clone(),
                    reference_description: ecp_component.description.clone(),
                };
                basis_element.references.push(ecp_reference);
                if basis_element.ecp_potentials.is_none() {
                    basis_element.ecp_potentials = Some(ecp_element.ecp_potentials.clone());
                } else if let Some(potentials) = &mut basis_element.ecp_potentials {
                    potentials.extend_from_slice(&ecp_element.ecp_potentials);
                }
                if basis_element.ecp_electrons.is_none() {
                    basis_element.ecp_electrons = Some(ecp_element.ecp_electrons);
                } else if let Some(electrons) = basis_element.ecp_electrons
                    && electrons != ecp_element.ecp_electrons
                {
                    bse_raise!(
                        DataError,
                        "Internal bug: ECP electron not match ({} != {})",
                        electrons,
                        ecp_element.ecp_electrons
                    )?;
                }
            } else {
                bse_raise!(DataError, "Internal bug: read skeleton element json failed {component_relpath}")?;
            }
        }
        basis_elements.insert(*atomic_number, basis_element);
    }

    /* #region other fields in BseBasis */

    // version is also defined in metadata (field version), but can be inferred from the file name.
    // take version `1` from file `def2-TZVP.1.table.json`, which is the last third value.
    // if version is not found, use -1 as default.
    let version =
        catch_unwind(|| file_relpath.split('.').rev().collect_vec()[2].parse::<i32>().unwrap()).map_err(|_| {
            BseError::DataError(format!(
                "{}BseError::DataError: Version annotation not found in {file_relpath}",
                bse_trace!()
            ))
        })?;

    // function types are obtained by iterating elements.
    let function_types = {
        let mut function_types = HashSet::new();
        for basis_element in basis_elements.values() {
            if let Some(shells) = &basis_element.electron_shells {
                for shell in shells {
                    function_types.insert(shell.function_type.clone());
                }
            }
        }
        function_types.into_iter().sorted().collect_vec()
    };

    // read skeleton metadata
    let name_prefix = file_relpath.split('.').next().unwrap_or("");
    let metadata_relpath = format!("{name_prefix}.metadata.json");
    let skel_metadata = read_skel_metadata_file_f(&metadata_relpath, data_dir)?;

    // temporarily use the first name in metadata.names
    let name = skel_metadata.names.first().cloned().unwrap();

    /* #endregion */

    let basis = BseBasis {
        molssi_bse_schema: BseMolssiBseSchema { schema_type: "complete".into(), schema_version: "0.1".into() },
        revision_description: skel_table.revision_description,
        revision_date: skel_table.revision_date,
        version,
        function_types,
        name,
        names: skel_metadata.names,
        tags: skel_metadata.tags,
        family: skel_metadata.family,
        description: skel_metadata.description,
        role: skel_metadata.role,
        auxiliaries: skel_metadata.auxiliaries,
        elements: basis_elements,
    };
    Ok(basis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compose_table_basis() {
        let data_dir = get_bse_data_dir().unwrap();

        let time = std::time::Instant::now();
        let table_relpath = "def2-TZVP.1.table.json";
        let basis = compose_table_basis(table_relpath, &data_dir);
        let json_str = serde_json::to_string_pretty(&basis.elements[&1]).unwrap();
        println!("{}", &json_str);
        let json_str = serde_json::to_string_pretty(&basis.elements[&54]).unwrap();
        println!("{}", &json_str);
        println!("compose_table_basis took {} ms", time.elapsed().as_millis());

        let time = std::time::Instant::now();
        let table_relpath = "def2-TZVP.1.table.json";
        let _basis = compose_table_basis(table_relpath, &data_dir);
        println!("compose_table_basis took {} ms", time.elapsed().as_millis());
    }
}
