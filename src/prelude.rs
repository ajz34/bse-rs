#![allow(unused_imports)]

// for users

pub use crate::api::*;
pub use crate::api::{
    filter_basis_sets, filter_basis_sets_f, get_all_basis_names, get_all_basis_names_f, get_families, get_families_f,
    get_roles, lookup_basis_by_role, lookup_basis_by_role_f, BseFilterArgs, BseFilterArgsBuilder,
};
pub use crate::fields::{
    BseAuxiliary, BseBasis, BseBasisElement, BseBasisMinimal, BseBasisReference, BseEcpElement, BseEcpPotential,
    BseElectronShell, BseElementComponents, BseGtoElement, BseMolssiBseSchema, BseRootMetadata, BseRootMetadataVer,
    BseSkelComponentEcp, BseSkelComponentGto, BseSkelElement, BseSkelMetadata, BseSkelTable,
};
pub use crate::readers::read::{get_reader_formats, read_formatted_basis_str, read_formatted_basis_str_f};
pub use crate::writers::write::{
    get_format_extension, get_writer_formats, write_formatted_basis_str, write_formatted_basis_str_f,
};

#[cfg(feature = "remote")]
pub use crate::client::{
    get_api_url, get_basis_notes_remote, get_basis_remote, get_family_notes_remote, get_formats_remote,
    get_formatted_basis_remote, get_metadata_remote, get_reference_formats_remote, DEFAULT_API_URL,
};

// for developers

pub(crate) use cached::proc_macro::{cached, once};
pub(crate) use derive_builder::{Builder, UninitializedFieldError};
pub(crate) use duplicate::duplicate_item;
pub(crate) use itertools::*;
pub(crate) use regex::{Regex, RegexBuilder};
pub(crate) use serde::de::{Unexpected, Visitor};
pub(crate) use serde::{Deserialize, Deserializer, Serialize};
pub(crate) use std::collections::{BTreeMap, HashMap, HashSet};
pub(crate) use std::panic::catch_unwind;
pub(crate) use std::sync::Mutex;

pub(crate) use crate::error::BseError;
pub(crate) use crate::*;
pub(crate) use misc::{COMPACT, HIJ, HIK, INCOMPACT, SCIFMT_D, SCIFMT_E};
