use assert_json_diff::assert_json_include;
use bse::prelude::*;
use rstest::rstest;
use std::fs::{read_to_string, write as write_from_string};

#[cfg(test)]
mod test {
    use super::*;

    #[rstest]
    #[case("cc-pVTZ", "1, 6-O")]
    #[case("def2-TZVPD", "1-3, 49-51")]
    fn test_get_basis_json(#[case] basis: &str, #[case] elements: &str) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let ref_file = format!("{manifest_dir}/tests/python_ref/get_basis_json/{basis}.json");
        let write_file = format!("{manifest_dir}/tests/tmp.json");

        let args = BseGetBasisArgs { elements: elements.to_string().into(), ..Default::default() };
        let basis = get_basis(basis, args);

        let basis_str = serde_json::to_string_pretty(&basis).unwrap();
        let basis_json = serde_json::from_str::<serde_json::Value>(&basis_str).unwrap();
        write_from_string(&write_file, &basis_str).unwrap();

        let ref_str = read_to_string(ref_file).unwrap();
        let ref_json = serde_json::from_str::<serde_json::Value>(&ref_str).unwrap();
        assert_json_include!(actual: basis_json, expected: ref_json);
    }

    #[rstest]
    #[case("cc-pVTZ", "1, 6-O")]
    #[case("def2-TZVPD", "1-3, 49-51")]
    fn test_get_basis_json_remove_free_primitives(#[case] basis: &str, #[case] elements: &str) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let ref_file = format!("{manifest_dir}/tests/python_ref/get_basis_json/{basis}-remove_free_primitives.json");
        let write_file = format!("{manifest_dir}/tests/tmp.json");

        let args = BseGetBasisArgs {
            elements: elements.to_string().into(),
            remove_free_primitives: true,
            ..Default::default()
        };
        let basis = get_basis(basis, args);

        let basis_str = serde_json::to_string_pretty(&basis).unwrap();
        let basis_json = serde_json::from_str::<serde_json::Value>(&basis_str).unwrap();
        write_from_string(&write_file, &basis_str).unwrap();

        let ref_str = read_to_string(ref_file).unwrap();
        let ref_json = serde_json::from_str::<serde_json::Value>(&ref_str).unwrap();
        assert_json_include!(actual: basis_json, expected: ref_json);
    }

    #[rstest]
    #[case("aug-cc-pVTZ", "1, 6-O")]
    fn test_get_basis_json_make_general(#[case] basis: &str, #[case] elements: &str) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let ref_file = format!("{manifest_dir}/tests/python_ref/get_basis_json/{basis}-make_general.json");
        let write_file = format!("{manifest_dir}/tests/tmp.json");

        let args = BseGetBasisArgs { elements: elements.to_string().into(), make_general: true, ..Default::default() };
        let basis = get_basis(basis, args);

        let basis_str = serde_json::to_string_pretty(&basis).unwrap();
        let basis_json = serde_json::from_str::<serde_json::Value>(&basis_str).unwrap();
        write_from_string(&write_file, &basis_str).unwrap();

        let ref_str = read_to_string(ref_file).unwrap();
        let ref_json = serde_json::from_str::<serde_json::Value>(&ref_str).unwrap();
        assert_json_include!(actual: basis_json, expected: ref_json);
    }
}
