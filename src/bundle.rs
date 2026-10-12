//! Multi-input analysis orchestration.
//!
//! This module owns parser-independent input identities and bundles. It deliberately reuses the
//! existing single-input analyzer for each unit and builds deterministic relation dependency graphs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sqlparser::dialect::Dialect;

use crate::constraints::{
    merge_relation_constraint_sets, ConstraintDiagnostic, RelationConstraintSet,
};
use crate::protocol::{
    ColumnDomain, ComparisonAssumption, ConditionExactness, DiagnosticSeverity, JoinKind, Output,
    Protocol, ProtocolStatement, SetOperation, WriteKind, WriteStateEffect, PROTOCOL_VERSION,
};
use crate::relation::{
    RelationCatalog, RelationContext, RelationResolutionError, RelationResolver, RelationSchema,
};
use crate::{analyze_sql, analyze_sql_with_catalog, Error};

// Domain types and orchestration stay separate from deterministic graph assembly.
mod composed;
mod graph;
mod graph_build;
mod input;
mod layers;
mod orchestration;
mod state;
mod targets;

pub use composed::*;
pub use graph::*;
pub use input::*;
pub use layers::*;
pub use orchestration::*;
pub use state::*;
pub use targets::*;

use graph_build::*;
