//! Main interface to Basis Set Exchange internal basis sets.
//!
//! This module contains the interface for getting basis set data and references
//! from the internal data store of basis sets.

use crate::prelude::*;

/// Obtain the version of the basis set exchange library (as a string).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static MAIN_URL: &str = "https://www.basissetexchange.org";

/* #region data directory */

pub static BSE_DATA_DIR_SPECIFIED: Mutex<Option<String>> = Mutex::new(None);

/// Set the local directory for the basis set exchange data.
///
/// This directory is usually at `basis_set_exchange/data` of the repository
/// root of <https://github.com/MolSSI-BSE/basis_set_exchange>.
pub fn specify_bse_data_dir(dir: String) {
    let mut specified = BSE_DATA_DIR_SPECIFIED.lock().unwrap();
    *specified = Some(dir);
}

fn get_bse_data_dir_specified() -> String {
    let specified = BSE_DATA_DIR_SPECIFIED.lock().unwrap();
    specified.as_ref().unwrap_or(&String::new()).clone()
}

#[once]
fn get_bse_data_dir_env() -> String {
    std::env::var("BSE_DATA_DIR").unwrap_or_default()
}

#[once]
fn get_bse_data_dir_manifest() -> String {
    std::env::var("CARGO_MANIFEST_DIR")
        .map_or(String::new(), |dir| format!("{dir}/basis_set_exchange/basis_set_exchange/data"))
}

/// Try to detect BSE data directory via Python's basis_set_exchange package.
///
/// This function attempts to run Python to query the installed
/// `basis_set_exchange` package for its data directory location. It does not
/// require pyo3 or linking to libpython - it simply spawns a subprocess.
///
/// # Returns
///
/// - `Some(String)` if Python is available and `basis_set_exchange` is
///   installed
/// - `None` if Python is not available, the package is not installed, or the
///   subprocess fails
#[once]
fn get_bse_data_dir_python() -> Option<String> {
    use std::process::Command;

    // Try multiple Python interpreters in order of preference
    let python_commands = ["python3", "python"];

    for python in python_commands {
        let result = Command::new(python)
            .args(["-c", "import basis_set_exchange; print(basis_set_exchange.get_data_dir())"])
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() && std::path::Path::new(&path).exists() {
                    return Some(path);
                }
            }
        }
    }

    None
}

/// Get the available data directory for the basis set exchange library.
///
/// This function checks the following directories in order:
/// 1. The directory specified by `specify_bse_data_dir`.
/// 2. The directory specified by the environment variable `BSE_DATA_DIR`.
/// 3. The directory detected via Python's installed `basis_set_exchange`
///    package.
/// 4. The directory specified by the `CARGO_MANIFEST_DIR` (the directory where
///    your crate built).
pub fn get_bse_data_dir() -> Option<String> {
    let dir_specified = get_bse_data_dir_specified();
    let dir_env = get_bse_data_dir_env();
    let dir_python = get_bse_data_dir_python();
    let dir_manifest = get_bse_data_dir_manifest();

    // Helper to check if a directory contains METADATA.json
    let is_valid_dir = |dir: &str| std::path::Path::new(&format!("{dir}/METADATA.json")).exists();

    // Check in order: specified -> env -> python -> manifest
    if !dir_specified.is_empty() && is_valid_dir(&dir_specified) {
        Some(dir_specified)
    } else if !dir_env.is_empty() && is_valid_dir(&dir_env) {
        Some(dir_env)
    } else if dir_python.as_ref().is_some_and(|d| is_valid_dir(d)) {
        dir_python
    } else if !dir_manifest.is_empty() && is_valid_dir(&dir_manifest) {
        Some(dir_manifest)
    } else {
        None
    }
}

/* #endregion */

/* #region read metadata */

/// Obtain the metadata for all basis sets.
///
/// The metadata includes information such as the display name of the basis set,
/// its versions, and what elements are included in the basis set.
///
/// The data is read from the METADATA.json file in the `data_dir` directory.
///
/// # Arguments
///
/// - `data_dir`: Data directory with all the basis set information. By default,
///   it is in the 'data' subdirectory of basis_set_exchange project.
pub fn get_metadata(data_dir: &str) -> HashMap<String, BseRootMetadata> {
    get_metadata_f(data_dir).unwrap()
}

#[cached(key = "String", convert = r#"{ data_dir.to_string() }"#)]
pub fn get_metadata_f(data_dir: &str) -> Result<HashMap<String, BseRootMetadata>, BseError> {
    let metadata_path = format!("{data_dir}/METADATA.json");
    let metadata = serde_json::from_str(&std::fs::read_to_string(metadata_path)?)?;
    Ok(metadata)
}

fn get_basis_metadata(name: &str, data_dir: &str) -> Result<BseRootMetadata, BseError> {
    // Transform the name into an internal representation
    let tr_name = misc::transform_basis_name(name);

    // Get the metadata for all basis sets
    let metadata = get_metadata_f(data_dir)?;

    if metadata.contains_key(&tr_name) {
        Ok(metadata[&tr_name].clone())
    } else {
        bse_raise!(
            DataNotFound,
            "Basis set `{name}` not found in metadata. Available basis sets: {:?}",
            metadata.keys().collect_vec()
        )?
    }
}

/* #endregion */

/* #region other auxiliaries */

/// Creates a header with information about a basis set.
///
/// Information includes description, revision, etc, but not references.
pub fn header_string(basis: &BseBasis) -> String {
    use textwrap::{wrap, Options};
    let space_14 = " ".repeat(14);
    let tw = Options::new(70).initial_indent("").subsequent_indent(&space_14);

    let header_list = vec![
        "-".repeat(70),
        " BSE-rs (Basis Set Exchange in Rust)".to_string(),
        format!(" Version {}", version()),
        format!(" Acknowledges: {MAIN_URL}"),
        "-".repeat(70).to_string(),
        format!("   Basis set: {}", basis.name),
        format!(" Description: {}", basis.description),
        format!("        Role: {}", basis.role),
        format!("     Version: {}  ({})", basis.version, basis.revision_description),
        "-".repeat(70).to_string(),
    ];
    header_list.iter().flat_map(|s| wrap(s, &tw)).join("\n")
}

/* #endregion */

/* #region get_basis */

/// Data source for basis set information.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BseDataSource {
    /// Use local data directory (requires `BSE_DATA_DIR` or `data_dir`
    /// parameter).
    Local,
    /// Use remote REST API (requires `remote` feature).
    #[cfg(feature = "remote")]
    Remote,
    /// Try local first, fallback to remote if available.
    /// Without `remote` feature, this is equivalent to `Local`.
    #[default]
    Auto,
}

#[derive(Builder, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[builder(build_fn(error = "BseError"))]
#[serde(default)]
pub struct BseGetBasisArgs {
    #[builder(default, setter(into))]
    pub elements: Option<String>,

    #[builder(default, setter(into))]
    pub version: Option<String>,

    #[builder(default = false)]
    pub uncontract_general: bool,

    #[builder(default = false)]
    pub uncontract_spdf: bool,

    #[builder(default = false)]
    pub uncontract_segmented: bool,

    #[builder(default = false)]
    pub remove_free_primitives: bool,

    #[builder(default = false)]
    pub make_general: bool,

    #[builder(default = false)]
    pub optimize_general: bool,

    #[builder(default = 0)]
    pub augment_diffuse: i32,

    #[builder(default = 0)]
    pub augment_steep: i32,

    #[builder(default = 0)]
    pub get_aux: i32,

    #[builder(default)]
    pub data_dir: Option<String>,

    #[builder(default = true)]
    pub header: bool,

    /// Data source for basis set (local, remote, or auto).
    #[builder(default)]
    pub source: BseDataSource,

    /// Custom API URL for remote fetching (optional, requires `remote`
    /// feature).
    #[cfg(feature = "remote")]
    #[builder(default, setter(into))]
    pub api_url: Option<String>,
}

impl Default for BseGetBasisArgs {
    fn default() -> Self {
        BseGetBasisArgsBuilder::default().build().unwrap()
    }
}

/// Obtain a basis set.
///
/// This is the main function for getting basis set information.
/// This function reads in all the basis data and returns it either
/// as a string or as a serde Value (JSON-like structure).
///
/// If you are looking for a specific format (like output nwchem, gaussian94,
/// etc.), use [`get_formatted_basis`] instead.
///
/// # Arguments
/// * `name` - Name of the basis set (case insensitive)
/// * `elements` - List of elements to get the basis set for. Elements can be
///   specified by:
///   - Atomic number (as integer or string)
///   - Element symbol (case insensitive)
///   - Can be a range string like "1-3,7-10" which will be expanded
///   - If empty/None, returns all elements the basis set is defined for
/// * `version` - Optional specific version of the basis set (defaults to
///   latest)
/// * `uncontract_general` - Remove general contractions by duplicating
///   primitives
/// * `uncontract_spdf` - Remove combined angular momentum contractions (sp, spd
///   etc)
/// * `uncontract_segmented` - Remove segmented contractions (each primitive
///   becomes new shell)
/// * `make_general` - Make basis set as generally-contracted as possible (one
///   shell per am)
/// * `optimize_general` - Optimize by removing general contractions with
///   uncontracted functions
/// * `augment_diffuse` - Add n diffuse functions via even-tempered
///   extrapolation
/// * `augment_steep` - Add n steep functions via even-tempered extrapolation
/// * `get_aux` - Which auxiliary basis to return:
///   - 0: Orbital basis (default)
///   - 1: AutoAux basis
///   - 2: Auto-ABS Coulomb fitting basis
/// * `data_dir` - Alternative data directory (defaults to project's "data"
///   subdirectory)
///
/// # Examples
/// ```
/// use bse::prelude::*;
/// let args = BseGetBasisArgsBuilder::default().elements("H, 6-O".to_string()).build().unwrap();
/// let basis: BseBasis = get_basis_f("sto-3g", args).expect("Failed to get basis set");
/// println!("Basis set: {basis:#?}");
/// ```
///
/// Arguments can also be parsed by `toml`:
/// ```
/// use bse::prelude::*;
/// let args = r#"
///     elements = "H, 6-O"
///     augment_diffuse = 1
///     get_aux = 1
/// "#;
/// let args: BseGetBasisArgs = toml::from_str(args).unwrap();
/// let basis: BseBasis = get_basis_f("sto-3g", args).unwrap();
/// println!("Basis set: {basis:#?}");
/// ```
pub fn get_basis(name: &str, args: BseGetBasisArgs) -> BseBasis {
    get_basis_f(name, args).unwrap()
}

/// Internal function for local basis set fetching.
fn get_basis_local_f(name: &str, args: &BseGetBasisArgs) -> Result<BseBasis, BseError> {
    let data_dir = args.data_dir.clone().or(get_bse_data_dir());
    if data_dir.is_none() {
        return bse_raise!(
            DataNotFound,
            "No data directory specified. Please set `BSE_DATA_DIR` environment variable."
        )?;
    }
    let data_dir = data_dir.unwrap();

    let bs_data = get_basis_metadata(name, &data_dir)?;

    // If version is not specified, use the latest
    let ver = args.version.clone().unwrap_or(bs_data.latest_version);
    if !bs_data.versions.contains_key(&ver) {
        bse_raise!(DataNotFound, "Version {ver} not found in metadata.")?;
    }

    // Compose the entire basis set (all elements)
    let table_relpath = &bs_data.versions[&ver].file_relpath;
    let mut basis_dict = compose::compose_table_basis_f(table_relpath, &data_dir)?;

    // Set the name (from the global metadata)
    // Only the list of all names will be returned from compose_table_basis
    basis_dict.name = bs_data.display_name.clone();

    // Handle optional arguments
    if let Some(elements) = &args.elements {
        // Convert to purely a list of strings that represent integers
        let elements = misc::expand_elements_f(elements)?;

        // Did the user pass an empty string or empty list?
        // If so, include all elements
        if !elements.is_empty() {
            let bs_elements = &basis_dict.elements;

            // Are elements part of this basis set?
            let bs_elements_keys = bs_elements.keys().map(|s| s.parse::<i32>().unwrap()).collect::<HashSet<_>>();
            let bs_elements_keys_vec = bs_elements_keys.iter().sorted().collect_vec();
            let elements_set: HashSet<i32> = HashSet::from_iter(elements.clone());
            if !elements_set.is_subset(&bs_elements_keys) {
                bse_raise!(
                    DataNotFound,
                    "Elements {:?} not found in basis set `{name}`. Available elements: {:?}",
                    elements,
                    bs_elements_keys_vec
                )?;
            }

            // Set to only the elements we want
            basis_dict.elements.retain(|el, _| elements_set.contains(&el.parse::<i32>().unwrap()));

            // Since we only grab some of the elements, we need to update the
            // function types used, too
            basis_dict.function_types = compose::whole_basis_types(&basis_dict.elements);
        }
    }

    // Note that from now on, the pipeline is going to modify basis_dict.
    // That is ok, since we are returned a unique instance from compose_table_basis.

    apply_basis_manipulations(&mut basis_dict, args)?;

    Ok(basis_dict)
}

/// Apply manipulations to a basis set based on args.
/// This is used after fetching from either local or remote source.
fn apply_basis_manipulations(basis_dict: &mut BseBasis, args: &BseGetBasisArgs) -> Result<(), BseError> {
    let mut needs_pruning = false;

    if args.remove_free_primitives {
        manip::remove_free_primitives(basis_dict);
        needs_pruning = true;
    }

    if args.optimize_general {
        manip::optimize_general(basis_dict);
        needs_pruning = true;
    }

    // uncontract_segmented implies uncontract_general
    if args.uncontract_segmented {
        manip::uncontract_segmented(basis_dict);
        needs_pruning = true;
    }

    if args.uncontract_general {
        manip::uncontract_general(basis_dict);
        needs_pruning = true;
    }

    if args.uncontract_spdf {
        manip::uncontract_spdf(basis_dict, 0);
        needs_pruning = true;
    }

    if args.make_general {
        manip::make_general(basis_dict, false);
        needs_pruning = true;
    }

    if needs_pruning {
        manip::prune_basis(basis_dict);
    }

    if args.augment_diffuse > 0 {
        manip::geometric_augmentation(basis_dict, args.augment_diffuse, false);
    }

    if args.augment_steep > 0 {
        manip::geometric_augmentation(basis_dict, args.augment_steep, true);
        sort::sort_basis(basis_dict);
    }

    // Re-make general
    if (args.augment_diffuse > 0 || args.augment_steep > 0) && args.make_general {
        manip::make_general(basis_dict, false);
    }

    match args.get_aux {
        0 => (),
        1 => *basis_dict = manip::autoaux_basis(basis_dict),
        2 => *basis_dict = manip::autoabs_basis(basis_dict, 1, 1.5),
        _ => bse_raise!(KeyError, "Invalid value for `get_aux`: {}", args.get_aux)?,
    }

    Ok(())
}

pub fn get_basis_f(name: &str, args: BseGetBasisArgs) -> Result<BseBasis, BseError> {
    // Handle data source selection
    match args.source {
        BseDataSource::Local => get_basis_local_f(name, &args),
        #[cfg(feature = "remote")]
        BseDataSource::Remote => {
            // Fetch from remote API
            let mut basis = client::get_basis_remote(name, &args)?;
            // Apply local manipulations that the REST API doesn't support
            apply_basis_manipulations(&mut basis, &args)?;
            Ok(basis)
        },
        #[cfg(feature = "remote")]
        BseDataSource::Auto => {
            // Try local first, fallback to remote
            get_basis_local_f(name, &args).or_else(|_| {
                let mut basis = client::get_basis_remote(name, &args)?;
                apply_basis_manipulations(&mut basis, &args)?;
                Ok(basis)
            })
        },
        #[cfg(not(feature = "remote"))]
        BseDataSource::Auto => get_basis_local_f(name, &args),
    }
}

/// Obtain a formatted basis set.
///
/// If you are looking for dumping dictionary-like structure, use
/// [`get_basis`] instead.
///
/// Usage is similar to [`get_basis`], but with an additional `fmt` argument.
///
/// # Arguments
///
/// * `name` - Name of the basis set (case insensitive)
/// * `fmt` - Desired output format (case insensitive). None returns a serde
///   Value. Example formats: nwchem, gaussian94, turbomole, etc.
/// * `args` - Arguments for the basis set, see [`BseGetBasisArgs`]. Additional
///   to [`get_basis`],
///   - `header` - Whether to include a header with information about the basis
///     set.
pub fn get_formatted_basis(name: &str, fmt: &str, args: BseGetBasisArgs) -> String {
    get_formatted_basis_f(name, fmt, args).unwrap()
}

pub fn get_formatted_basis_f(name: &str, fmt: &str, args: BseGetBasisArgs) -> Result<String, BseError> {
    let basis = get_basis_f(name, args.clone())?;
    let header = if args.header { Some(header_string(&basis)) } else { None };
    writers::write::write_formatted_basis_str_f(&basis, fmt, header.as_deref())
}

/* #endregion */

/// Return information about the basis set formats available for output.
///
/// This is a convenience wrapper that calls [`get_writer_formats`].
///
/// The returned data is a map of format name to display name. The format
/// can be passed as the `fmt` argument to [`get_formatted_basis`].
///
/// # Arguments
///
/// * `function_types` - Optional list of function types to filter by. If
///   provided, only formats supporting those types are returned.
///
/// # Example
///
/// ```
/// use bse::prelude::*;
/// let formats = get_formats(None);
/// assert!(!formats.is_empty());
/// assert!(formats.contains_key("nwchem"));
/// ```
pub fn get_formats(function_types: Option<Vec<String>>) -> HashMap<String, String> {
    writers::write::get_writer_formats(function_types)
}

/// Return information about the available basis set roles.
///
/// The returned data is a map of role to display name. Roles represent
/// different types of basis sets such as orbital, fitting, etc.
///
/// Available roles are:
/// - `orbital`: Orbital basis
/// - `jfit`: J-fitting
/// - `jkfit`: JK-fitting
/// - `rifit`: RI-fitting
/// - `optri`: Optimized RI-fitting
/// - `admmfit`: Auxiliary-Density Matrix Method Fitting
/// - `dftxfit`: DFT Exchange Fitting
/// - `dftjfit`: DFT Correlation Fitting
/// - `guess`: Initial guess
pub fn get_roles() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("orbital", "Orbital basis"),
        ("jfit", "J-fitting"),
        ("jkfit", "JK-fitting"),
        ("rifit", "RI-fitting"),
        ("optri", "Optimized RI-fitting"),
        ("admmfit", "Auxiliary-Density Matrix Method Fitting"),
        ("dftxfit", "DFT Exchange Fitting"),
        ("dftjfit", "DFT Correlation Fitting"),
        ("guess", "Initial guess"),
    ])
}

/* #endregion */

/* #region get_basis_names_and_families */

/// Obtain a list of all basis set names (display names).
///
/// The returned list contains the display names of all basis sets,
/// sorted alphabetically.
///
/// # Arguments
///
/// * `data_dir` - Optional data directory. If None, uses the default (from
///   `BSE_DATA_DIR` environment variable or auto-detected).
///
/// # Example
///
/// ```
/// use bse::prelude::*;
/// let names = get_all_basis_names(None);
/// assert!(!names.is_empty());
/// assert!(names.contains(&"STO-3G".to_string()));
/// ```
pub fn get_all_basis_names(data_dir: Option<String>) -> Vec<String> {
    get_all_basis_names_f(data_dir).unwrap()
}

pub fn get_all_basis_names_f(data_dir: Option<String>) -> Result<Vec<String>, BseError> {
    let data_dir = data_dir.or(get_bse_data_dir());
    if data_dir.is_none() {
        return bse_raise!(
            DataNotFound,
            "No data directory specified. Please set `BSE_DATA_DIR` environment variable."
        );
    }
    let data_dir = data_dir.unwrap();

    let metadata = get_metadata_f(&data_dir)?;
    let names: Vec<String> = metadata.values().map(|v| v.display_name.clone()).sorted().collect();
    Ok(names)
}

/// Return a list of all basis set families.
///
/// Families group related basis sets together (e.g., "pople", "dunning",
/// "ahlrichs", etc.).
///
/// # Arguments
///
/// * `data_dir` - Optional data directory. If None, uses the default.
///
/// # Example
///
/// ```
/// use bse::prelude::*;
/// let families = get_families(None);
/// assert!(!families.is_empty());
/// assert!(families.contains(&"dunning".to_string()));
/// ```
pub fn get_families(data_dir: Option<String>) -> Vec<String> {
    get_families_f(data_dir).unwrap()
}

pub fn get_families_f(data_dir: Option<String>) -> Result<Vec<String>, BseError> {
    let data_dir = data_dir.or(get_bse_data_dir());
    if data_dir.is_none() {
        return bse_raise!(
            DataNotFound,
            "No data directory specified. Please set `BSE_DATA_DIR` environment variable."
        );
    }
    let data_dir = data_dir.unwrap();

    let metadata = get_metadata_f(&data_dir)?;
    let families: HashSet<String> = metadata.values().map(|v| v.family.clone()).collect();
    let families: Vec<String> = families.into_iter().sorted().collect();
    Ok(families)
}

/// Lookup the name of an auxiliary basis set given a primary basis set and
/// role.
///
/// This is useful for finding fitting basis sets that correspond to
/// a given orbital basis set.
///
/// # Arguments
///
/// * `primary_basis` - The primary (orbital) basis set name (case insensitive)
/// * `role` - Desired role/type of auxiliary basis set (case insensitive). Use
///   [`get_roles`] to see available roles.
/// * `data_dir` - Optional data directory.
///
/// # Returns
///
/// A list of auxiliary basis set names for the given primary basis and role.
///
/// # Example
///
/// ```
/// use bse::prelude::*;
/// let aux_names = lookup_basis_by_role("cc-pVTZ", "jkfit", None);
/// assert!(!aux_names.is_empty());
/// ```
pub fn lookup_basis_by_role(primary_basis: &str, role: &str, data_dir: Option<String>) -> Vec<String> {
    lookup_basis_by_role_f(primary_basis, role, data_dir).unwrap()
}

pub fn lookup_basis_by_role_f(
    primary_basis: &str,
    role: &str,
    data_dir: Option<String>,
) -> Result<Vec<String>, BseError> {
    let data_dir = data_dir.or(get_bse_data_dir());
    if data_dir.is_none() {
        return bse_raise!(
            DataNotFound,
            "No data directory specified. Please set `BSE_DATA_DIR` environment variable."
        );
    }
    let data_dir = data_dir.unwrap();

    let role = role.to_lowercase();
    let roles = get_roles();
    if !roles.contains_key(role.as_str()) {
        bse_raise!(ValueError, "Role '{role}' is not a valid role. Available roles: {:?}", roles.keys().collect_vec())?;
    }

    let bs_data = get_basis_metadata(primary_basis, &data_dir)?;
    let auxdata = &bs_data.auxiliaries;

    if !auxdata.contains_key(&role) {
        bse_raise!(ValueError, "Role '{role}' doesn't exist for basis set '{primary_basis}'")?;
    }

    match &auxdata[&role] {
        BseAuxiliary::Str(s) => Ok(vec![s.clone()]),
        BseAuxiliary::Vec(v) => Ok(v.clone()),
    }
}

/* #endregion */

/* #region filter_basis_sets */

/// Arguments for filtering basis sets.
#[derive(Builder, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[builder(build_fn(error = "BseError"), default)]
pub struct BseFilterArgs {
    /// Substring to search for in the basis set name
    #[builder(default, setter(into))]
    pub substr: Option<String>,

    /// Family the basis set belongs to
    #[builder(default, setter(into))]
    pub family: Option<String>,

    /// Role of the basis set
    #[builder(default, setter(into))]
    pub role: Option<String>,

    /// List of elements that the basis set must include
    #[builder(default, setter(into))]
    pub elements: Option<String>,

    /// Data directory
    #[builder(default)]
    pub data_dir: Option<String>,
}

/// Filter basis sets by various criteria.
///
/// All parameters are ANDed together (all must match for a basis set
/// to be included in the result).
///
/// # Arguments
///
/// * `substr` - Substring to search for in the basis set name (case
///   insensitive)
/// * `family` - Family the basis set belongs to (case insensitive)
/// * `role` - Role of the basis set (case insensitive)
/// * `elements` - List of elements that the basis set must include. Elements
///   can be specified by atomic number, symbol, or ranges (e.g., "1-3,6-8" or
///   "H-Li,C-O").
/// * `data_dir` - Optional data directory
///
/// # Returns
///
/// A HashMap of internal basis set names to their metadata.
///
/// # Example
///
/// ```
/// use bse::prelude::*;
/// let args = BseFilterArgsBuilder::default()
///     .family("dunning")
///     .substr("aug")
///     .build()
///     .unwrap();
/// let filtered = filter_basis_sets(args);
/// assert!(!filtered.is_empty());
/// ```
pub fn filter_basis_sets(args: BseFilterArgs) -> HashMap<String, BseRootMetadata> {
    filter_basis_sets_f(args).unwrap()
}

pub fn filter_basis_sets_f(args: BseFilterArgs) -> Result<HashMap<String, BseRootMetadata>, BseError> {
    let data_dir = args.data_dir.clone().or(get_bse_data_dir());
    if data_dir.is_none() {
        return bse_raise!(
            DataNotFound,
            "No data directory specified. Please set `BSE_DATA_DIR` environment variable."
        );
    }
    let data_dir = data_dir.unwrap();

    let mut metadata = get_metadata_f(&data_dir)?;

    // Filter by family
    if let Some(family) = &args.family {
        let family = family.to_lowercase();
        let families = get_families_f(Some(data_dir.clone()))?;
        if !families.contains(&family) {
            bse_raise!(ValueError, "Family '{family}' is not a valid family. Available families: {:?}", families)?;
        }
        metadata.retain(|_, v| v.family == family);
    }

    // Filter by role
    if let Some(role) = &args.role {
        let role = role.to_lowercase();
        let roles = get_roles();
        if !roles.contains_key(role.as_str()) {
            bse_raise!(
                ValueError,
                "Role '{role}' is not a valid role. Available roles: {:?}",
                roles.keys().collect_vec()
            )?;
        }
        metadata.retain(|_, v| v.role == role);
    }

    // Filter by elements
    if let Some(elements) = &args.elements {
        let elements = misc::expand_elements_f(elements)?;
        let elements_set: HashSet<i32> = HashSet::from_iter(elements.clone());

        for (_, basis_data) in metadata.iter_mut() {
            // Filter versions to those that contain all required elements
            basis_data.versions.retain(|_, ver| {
                let ver_elements: HashSet<i32> = ver.elements.iter().filter_map(|e| e.parse::<i32>().ok()).collect();
                elements_set.is_subset(&ver_elements)
            });
        }

        // Remove basis sets with no matching versions
        metadata.retain(|_, v| !v.versions.is_empty());
    }

    // Filter by substring
    if let Some(substr) = &args.substr {
        let substr = substr.to_lowercase();
        metadata.retain(|k, v| k.contains(&substr) || v.display_name.to_lowercase().contains(&substr));
    }

    Ok(metadata)
}

/* #endregion */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_basis() {
        let args = BseGetBasisArgsBuilder::default().elements("H, 6-O".to_string()).build().unwrap();
        let basis: BseBasis = get_basis_f("sto-3g", args).unwrap();
        println!("Basis set: {basis:#?}");
        assert_eq!(basis.name, "STO-3G");

        let header = header_string(&basis);
        println!("Header:\n{header}");
    }

    #[test]
    fn test_get_basis_toml() {
        let args = r#"
            elements = "H, 6-O"
            augment_diffuse = 1
            get_aux = 1
            header = true
        "#;
        let args: BseGetBasisArgs = toml::from_str(args).unwrap();
        let basis: BseBasis = get_basis_f("sto-3g", args).unwrap();
        println!("Basis set: {basis:#?}");
    }

    #[test]
    fn test_get_header() {
        let data_dir = get_bse_data_dir().expect("Data directory not found");
        let args = BseGetBasisArgsBuilder::default().data_dir(Some(data_dir.clone())).build().unwrap();
        let basis = get_basis_f("2ZaPa-NR-CV", args).expect("Failed to get basis set");
        let header = header_string(&basis);
        println!("Header:\n{header}");
    }

    #[test]
    fn test_python_detection() {
        // Test that Python detection works if basis_set_exchange is installed
        let dir_python = get_bse_data_dir_python();
        if let Some(dir) = dir_python {
            println!("Python detected data directory: {dir}");
            assert!(std::path::Path::new(&format!("{dir}/METADATA.json")).exists());
        } else {
            println!("Python basis_set_exchange package not available, skipping");
        }
    }

    #[test]
    fn test_get_all_basis_names() {
        let names = get_all_basis_names(None);
        assert!(!names.is_empty());
        // Should contain some well-known basis sets
        assert!(names.iter().any(|n| n.contains("STO-3G")));
        assert!(names.iter().any(|n| n.contains("cc-pVTZ")));
        println!("Number of basis sets: {}", names.len());
    }

    #[test]
    fn test_get_families() {
        let families = get_families(None);
        assert!(!families.is_empty());
        assert!(families.contains(&"dunning".to_string()));
        assert!(families.contains(&"pople".to_string()));
        println!("Families: {:?}", families);
    }

    #[test]
    fn test_get_roles() {
        let roles = get_roles();
        assert!(!roles.is_empty());
        assert!(roles.contains_key("orbital"));
        assert!(roles.contains_key("jkfit"));
        println!("Roles: {:?}", roles);
    }

    #[test]
    fn test_lookup_basis_by_role() {
        // cc-pVTZ should have jkfit auxiliary
        let aux_names = lookup_basis_by_role("cc-pVTZ", "jkfit", None);
        assert!(!aux_names.is_empty());
        println!("cc-pVTZ jkfit auxiliaries: {:?}", aux_names);
    }

    #[test]
    fn test_filter_basis_sets() {
        // Filter by family
        let args = BseFilterArgsBuilder::default().family("dunning".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);
        assert!(!filtered.is_empty());
        assert!(filtered.values().all(|v| v.family == "dunning"));
        println!("Dunning family basis sets: {}", filtered.len());

        // Filter by substring
        let args = BseFilterArgsBuilder::default().substr("aug-cc-pV".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);
        assert!(!filtered.is_empty());
        println!("Basis sets matching 'aug-cc-pV': {}", filtered.len());

        // Filter by role
        let args = BseFilterArgsBuilder::default().role("jkfit".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);
        assert!(!filtered.is_empty());
        assert!(filtered.values().all(|v| v.role == "jkfit"));
        println!("JK-fitting basis sets: {}", filtered.len());

        // Filter by elements
        let args = BseFilterArgsBuilder::default().elements("H-C".to_string()).build().unwrap();
        let filtered = filter_basis_sets(args);
        assert!(!filtered.is_empty());
        println!("Basis sets covering H-C: {}", filtered.len());
    }
}
