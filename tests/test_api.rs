//! Integration tests for the API functions.
//!
//! These tests compare the output of the Rust implementation against
//! the expected behavior from Python's basis_set_exchange library.

use bse::prelude::*;
use rstest::rstest;

#[cfg(test)]
mod test_api_metadata {
    use super::*;

    #[test]
    fn test_get_all_basis_names() {
        let names = get_all_basis_names(None);
        assert!(!names.is_empty());

        // Should contain some well-known basis sets
        assert!(names.iter().any(|n| n == "STO-3G"));
        assert!(names.iter().any(|n| n == "cc-pVTZ"));
        assert!(names.iter().any(|n| n == "def2-SVP"));

        // Should be sorted
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);

        println!("Total number of basis sets: {}", names.len());
    }

    #[test]
    fn test_get_families() {
        let families = get_families(None);
        assert!(!families.is_empty());

        // Should contain well-known families
        assert!(families.contains(&"dunning".to_string()));
        assert!(families.contains(&"pople".to_string()));
        assert!(families.contains(&"ahlrichs".to_string()));

        // Should be sorted
        let mut sorted_families = families.clone();
        sorted_families.sort();
        assert_eq!(families, sorted_families);

        println!("Families: {:?}", families);
    }

    #[test]
    fn test_get_roles() {
        let roles = get_roles();
        assert!(!roles.is_empty());

        // Should contain all expected roles
        assert!(roles.contains_key("orbital"));
        assert!(roles.contains_key("jfit"));
        assert!(roles.contains_key("jkfit"));
        assert!(roles.contains_key("rifit"));
        assert!(roles.contains_key("optri"));
        assert!(roles.contains_key("admmfit"));
        assert!(roles.contains_key("dftxfit"));
        assert!(roles.contains_key("dftjfit"));
        assert!(roles.contains_key("guess"));

        println!("Roles: {:?}", roles);
    }
}

#[cfg(test)]
mod test_api_lookup {
    use super::*;

    #[rstest]
    #[case("cc-pVTZ", "jkfit")]
    #[case("cc-pVTZ", "rifit")]
    #[case("def2-SVP", "jkfit")]
    #[case("def2-SVP", "jfit")]
    fn test_lookup_basis_by_role(#[case] basis: &str, #[case] role: &str) {
        let aux_names = lookup_basis_by_role(basis, role, None);
        assert!(!aux_names.is_empty());
        println!("{} {} auxiliary: {:?}", basis, role, aux_names);
    }

    #[test]
    fn test_lookup_basis_by_role_case_insensitive() {
        // Both should work
        let aux1 = lookup_basis_by_role("cc-pVTZ", "jkfit", None);
        let aux2 = lookup_basis_by_role("CC-PVTZ", "JKFIT", None);
        assert_eq!(aux1, aux2);
    }

    #[test]
    fn test_lookup_basis_by_role_invalid_role() {
        let result = lookup_basis_by_role_f("cc-pVTZ", "invalid_role", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_lookup_basis_by_role_missing_role() {
        // Some basis sets don't have all auxiliary roles
        let result = lookup_basis_by_role_f("STO-3G", "jkfit", None);
        // This should either return an error or empty list
        // depending on the basis set
        println!("STO-3G jkfit result: {:?}", result);
    }
}

#[cfg(test)]
mod test_api_filter {
    use super::*;

    #[test]
    fn test_filter_by_family() {
        let args = BseFilterArgsBuilder::default().family("dunning".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);

        assert!(!filtered.is_empty());
        assert!(filtered.values().all(|v| v.family == "dunning"));

        println!("Dunning family basis sets: {}", filtered.len());
    }

    #[test]
    fn test_filter_by_role() {
        let args = BseFilterArgsBuilder::default().role("jkfit".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);

        assert!(!filtered.is_empty());
        assert!(filtered.values().all(|v| v.role == "jkfit"));

        println!("JK-fitting basis sets: {}", filtered.len());
    }

    #[test]
    fn test_filter_by_substr() {
        let args = BseFilterArgsBuilder::default().substr("aug-cc-pV".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);

        assert!(!filtered.is_empty());

        // All results should contain "aug-cc-pV" in either name or display name
        for (key, meta) in &filtered {
            let contains =
                key.to_lowercase().contains("aug-cc-pv") || meta.display_name.to_lowercase().contains("aug-cc-pv");
            assert!(contains, "Expected {} to contain 'aug-cc-pV'", key);
        }

        println!("Basis sets matching 'aug-cc-pV': {}", filtered.len());
    }

    #[test]
    fn test_filter_by_elements() {
        // Find basis sets that cover H through C (1-6)
        let args = BseFilterArgsBuilder::default().elements("H-C".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);

        assert!(!filtered.is_empty());

        println!("Basis sets covering H-C: {}", filtered.len());
    }

    #[test]
    fn test_filter_combined() {
        // Filter by family AND role
        // Note: jkfit fitting basis sets for dunning are in "dunning_fit" family
        let args = BseFilterArgsBuilder::default()
            .family("dunning_fit".to_string())
            .role("jkfit".to_string())
            .build()
            .unwrap();
        let filtered = filter_basis_sets(args);

        assert!(!filtered.is_empty());
        assert!(filtered.values().all(|v| v.family == "dunning_fit" && v.role == "jkfit"));

        println!("Dunning_fit JK-fitting basis sets: {}", filtered.len());
    }

    #[test]
    fn test_filter_case_insensitive() {
        let args1 = BseFilterArgsBuilder::default().family("DUNNING".to_string()).build().unwrap();
        let args2 = BseFilterArgsBuilder::default().family("dunning".to_string()).build().unwrap();

        let filtered1 = filter_basis_sets(args1);
        let filtered2 = filter_basis_sets(args2);

        assert_eq!(filtered1.len(), filtered2.len());
    }
}

#[cfg(test)]
mod test_api_formats {
    use super::*;

    #[test]
    fn test_get_writer_formats() {
        let formats = get_writer_formats(None);
        assert!(!formats.is_empty());

        // Should contain well-known formats
        assert!(formats.contains_key("nwchem"));
        assert!(formats.contains_key("gaussian94"));
        assert!(formats.contains_key("psi4"));
        assert!(formats.contains_key("turbomole"));
        assert!(formats.contains_key("molpro"));

        println!("Writer formats: {:?}", formats.keys().collect::<Vec<_>>());
    }

    #[test]
    fn test_get_writer_formats_filter() {
        // Filter by function types
        let formats_all = get_writer_formats(None);
        let formats_gto = get_writer_formats(Some(vec!["gto".to_string(), "gto_spherical".to_string()]));

        // VeloxChem only supports gto and gto_spherical, so it should be included
        assert!(formats_gto.contains_key("veloxchem"));

        // All filtered formats should be in the full list
        for key in formats_gto.keys() {
            assert!(formats_all.contains_key(key));
        }

        println!("All formats: {}", formats_all.len());
        println!("GTO spherical formats: {}", formats_gto.len());
    }

    #[test]
    fn test_get_reader_formats() {
        let formats = get_reader_formats();
        assert!(!formats.is_empty());

        // Should contain well-known formats
        assert!(formats.contains_key("nwchem"));
        assert!(formats.contains_key("gaussian94"));
        assert!(formats.contains_key("turbomole"));
        assert!(formats.contains_key("dalton"));

        println!("Reader formats: {:?}", formats);
    }

    #[test]
    fn test_get_format_extension() {
        assert_eq!(get_format_extension("nwchem").unwrap(), ".nw");
        assert_eq!(get_format_extension("gaussian94").unwrap(), ".gbs");
        assert_eq!(get_format_extension("turbomole").unwrap(), ".tm");
        assert_eq!(get_format_extension("qcschema").unwrap(), ".json");

        // Case insensitive
        assert_eq!(get_format_extension("NWCHEM").unwrap(), ".nw");
        assert_eq!(get_format_extension("Gaussian94").unwrap(), ".gbs");
    }

    #[test]
    fn test_get_format_extension_invalid() {
        let result = get_format_extension("invalid_format");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_formats() {
        // get_formats is an alias for get_writer_formats
        let formats1 = get_formats(None);
        let formats2 = get_writer_formats(None);

        assert_eq!(formats1, formats2);
    }
}
