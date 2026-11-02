//! Table-level SQL lifecycle statements at the AST-to-domain boundary.
//!
//! Only a single portable table action is represented; vendor/session laws
//! are kept explicitly unsupported by the caller.

use super::*;

pub(super) fn analyze_relation_lifecycle(
    target: String,
    action: WriteLifecycleAction,
) -> ProtocolStatement {
    ProtocolStatement::Query(
        QueryStatement::new(
            Vec::new(),
            vec![target.clone()],
            Vec::new(),
            RowConditions::new(
                Predicates::new(None, None, None),
                Vec::new(),
                ConditionExactness::from_residuals(vec![ResidualCondition::new(
                    ResidualConditionReason::AnalysisDiagnostic,
                    ConditionClause::RowSetOperator,
                    "lifecycle_requires_attested_complete_relation_state",
                )]),
            ),
            Output::new(Vec::new()),
            Vec::new(),
        )
        .with_produced_relation(Some(target.clone()))
        .with_write(Some(WriteOperation::lifecycle(target, action))),
    )
}
