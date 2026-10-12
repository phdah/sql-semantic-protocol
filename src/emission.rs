//! Protocol serialization boundary.
//!
//! Serialization depends only on parser-independent protocol values. It deliberately contains no
//! sqlparser AST handling.

use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::physical_realization::{
    physical_joint_source_plan, physical_source_plan, PhysicalPlanNode, PhysicalPlanRef,
};

use crate::bundle::{
    AnalysisBundle, AnalysisGraph, ComposedSemantics, CompositionDiagnostic, DatasetRef,
    GraphComponent, GraphEdge, LayerWriteStateEffect, ResolvedComposedSemantics, SqlInputSource,
    TransformationLayer, UnresolvedComposedSemantics,
};
use crate::constraints::{
    ConstraintDiagnostic, ConstraintValue, RelationConstraint, RelationConstraintSet,
};
use crate::data_type::DataType;
use crate::protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, Bound, CaseExpression, CaseSourceDomains, ColumnDomain, ColumnExpression,
    ColumnRef, ComparisonPredicate, ConditionExactness, Diagnostic, ExistsPredicate, Expression,
    FunctionExpression, GroupBy, GroupingExpression, InPredicate, InSubqueryPredicate,
    IsNullPredicate, Join, LineageSource, LiteralExpression, LiteralValue, LogicalPredicate,
    MergeAction, MergeClause, NotPredicate, Output, OutputColumn, Predicate, Predicates, Protocol,
    ProtocolStatement, QueryStatement, RelationRef, ScalarSubqueryExpression, SetOperand,
    SetOperation, SourceRelation, SubquerySemantics, UnaryExpression, UnknownSemantic,
    UnsupportedSemantic, UnsupportedStatement, ValueDomain, ValueRange, WindowFrame,
    WindowFrameBound, WindowFunctionExpression, WindowOrderExpression, WindowSpecification,
    WriteEffectAction, WriteIdempotence, WriteOperation, WritePostState, WriteValue,
};

// Serialization remains parser-independent; each format family has a single owner.
mod aggregation_sets;
mod composition;
mod constraints;
mod constructive;
mod document;
mod expressions;
mod graph;
mod operator_witnesses;
mod outcomes;
mod predicates;
mod relations_domains;
mod scalars;
mod statements;

pub use document::{to_bundle_json, to_json};

use aggregation_sets::*;
use composition::*;
use constraints::*;
use constructive::*;
use expressions::*;
use graph::*;
use operator_witnesses::*;
use outcomes::*;
use predicates::*;
use relations_domains::*;
use scalars::*;
use statements::*;
