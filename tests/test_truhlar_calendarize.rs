use assert_json_diff::assert_json_include;
use bse::manip;
use bse::prelude::*;
use rstest::rstest;
use std::fs::{read_to_string, write as write_from_string};

#[cfg(test)]
mod test {
    use super::*;

    #[rstest]
    #[case("aug-cc-pVTZ", "jun", "1-6-31-30")] // H, C, Ga, Zn
    #[case("aug-cc-pVTZ", "apr", "1-6-31-30")] // H, C, Ga, Zn
    #[case("aug-cc-pVQZ", "jun", "1-6-31-30")] // H, C, Ga, Zn
    #[case("aug-cc-pVQZ", "apr", "1-6-31-30")] // H, C, Ga, Zn
    fn test_truhlar_calendarize_json(#[case] basis_name: &str, #[case] month: &str, #[case] elements: &str) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let ref_file =
            format!("{manifest_dir}/tests/python_ref/get_truhlar_calendarize/{basis_name}-{month}-{elements}.json");
        let write_file = format!("{manifest_dir}/tmp/truhlar-{basis_name}-{month}-{elements}.json");

        // Parse element string into format for get_basis
        let element_str = elements.replace("-", ",");

        // Get the original augmented basis
        let args = BseGetBasisArgsBuilder::default().elements(element_str).build().unwrap();
        let aug_basis = get_basis(basis_name, args);

        // Apply truhlar_calendarize
        let result = manip::truhlar_calendarize(&aug_basis, month).unwrap();

        // Serialize to JSON
        let result_str = serde_json::to_string_pretty(&result).unwrap();
        let result_json = serde_json::from_str::<serde_json::Value>(&result_str).unwrap();

        // Write output for debugging
        std::fs::create_dir_all(format!("{manifest_dir}/tmp")).ok();
        write_from_string(&write_file, &result_str).unwrap();

        // Load reference
        let ref_str = read_to_string(ref_file).unwrap();
        let ref_json = serde_json::from_str::<serde_json::Value>(&ref_str).unwrap();

        // Compare
        assert_json_include!(actual: result_json, expected: ref_json);
    }

    #[rstest]
    #[case("aug-cc-pVTZ", "jun", "1-6-31-30")] // H, C, Ga, Zn
    #[case("aug-cc-pVTZ", "apr", "1-6-31-30")] // H, C, Ga, Zn
    #[case("aug-cc-pVQZ", "jun", "1-6-31-30")] // H, C, Ga, Zn
    #[case("aug-cc-pVQZ", "apr", "1-6-31-30")] // H, C, Ga, Zn
    fn test_truhlar_calendarize_g94(#[case] basis_name: &str, #[case] month: &str, #[case] elements: &str) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let ref_file =
            format!("{manifest_dir}/tests/python_ref/get_truhlar_calendarize/{basis_name}-{month}-{elements}.g94");
        let write_file = format!("{manifest_dir}/tmp/truhlar-{basis_name}-{month}-{elements}.g94");

        // Parse element string into format for get_basis
        let element_str = elements.replace("-", ",");

        // Get the original augmented basis
        let args = BseGetBasisArgsBuilder::default().elements(element_str).build().unwrap();
        let aug_basis = get_basis(basis_name, args);

        // Apply truhlar_calendarize
        let result = manip::truhlar_calendarize(&aug_basis, month).unwrap();

        // Format as Gaussian94 (no header)
        let result_str = write_formatted_basis_str(&result, "gaussian94", None);

        // Write output for debugging
        std::fs::create_dir_all(format!("{manifest_dir}/tmp")).ok();
        write_from_string(&write_file, &result_str).unwrap();

        // Load reference
        let ref_str = read_to_string(ref_file).unwrap();

        // Show diff for debugging
        use similar::{ChangeTag, TextDiff};
        let diff = TextDiff::from_lines(&result_str, &ref_str);
        let has_diff = diff.iter_all_changes().any(|c| c.tag() != ChangeTag::Equal);
        if has_diff {
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                print!("{sign}{change}");
            }
        }

        assert_eq!(result_str, ref_str);
    }
}
