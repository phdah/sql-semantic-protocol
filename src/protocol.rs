//! Parser-independent SQL Semantic Protocol domain values.
//!
//! The Rust model contains no sqlparser AST types. Unknown and unsupported
//! semantics stay explicit so consumers distinguish analysis gaps from known facts.
//! Modules partition Rust domain definitions only; the canonical protocol and
//! its JSON representation remain independent of source-file organization.

use std::collections::BTreeSet;

use crate::boolean_witness::BooleanWitness;
use crate::constraints::RelationConstraintSet;
use crate::group_witness::GroupWitness;
use crate::window_witness::WindowWitness;

/// Current protocol version emitted by this crate.
pub const PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION");

// Keep the canonical crate::protocol API stable for all internal and external users.
mod aggregation;
mod conditions;
mod diagnostics;
mod document;
mod finite_table;
mod functions;
mod predicates;
mod query;
mod relations;
mod sets;
mod values;
mod writes;

pub use aggregation::*;
pub use conditions::*;
pub use diagnostics::*;
pub use document::*;
pub use finite_table::*;
pub use functions::*;
pub use predicates::*;
pub use query::*;
pub use relations::*;
pub use sets::*;
pub use values::*;
pub use writes::*;
