//! Construction of typed local operator witnesses and producer obligations.

use super::*;

/// Return typed intermediate-source obligations that cannot be satisfied by
/// inserting rows directly into a derived relation.
///
/// Consumers must discharge these against the producer graph, with the physical
/// leaf dependencies in `ResolvedComposedSemantics::dependencies`. A pending
/// producer is not a proof that physical realization is possible (TASK-68).
pub fn local_pending_producers(semantics: &ResolvedComposedSemantics) -> Vec<WitnessObligation> {
    let mut boundaries = Vec::new();
    let mut add = |relation: Option<&str>, kind: GroupBoundaryKind, layer_id: &str| {
        if kind != GroupBoundaryKind::Intermediate {
            return;
        }
        if let Some(boundary) = relation.and_then(|r| WitnessBoundary::new(r, kind, layer_id)) {
            if !boundaries.contains(&boundary) {
                boundaries.push(boundary);
            }
        }
    };
    for witness in semantics.boolean_witnesses() {
        add(
            Some(witness.witness().source_relation()),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for witness in semantics.group_witnesses() {
        add(
            witness.witness().boundary(),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for witness in semantics.window_witnesses() {
        add(
            witness.witness().boundary(),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for witness in semantics.subquery_witnesses() {
        add(
            witness.witness().inner_relation(),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for item in semantics.set_operations() {
        let operation = item.operation();
        for branch in operation.branches() {
            if let Some(boundary) = branch.witness_boundary() {
                if boundary.is_intermediate() {
                    add(
                        Some(boundary.relation()),
                        GroupBoundaryKind::Intermediate,
                        item.origin_layer_id(),
                    );
                }
            }
        }
    }
    boundaries
        .sort_by(|a, b| (&a.origin_layer_id, &a.relation).cmp(&(&b.origin_layer_id, &b.relation)));
    boundaries
        .into_iter()
        .map(|boundary| {
            // A missing producer mapping is unknown, never the terminal dependency set.
            let physical_sources = semantics
                .producer_physical_sources(boundary.relation())
                .map_or_else(Vec::new, |sources| sources.to_vec());
            WitnessObligation::Producer {
                boundary,
                physical_sources,
            }
        })
        .collect()
}

/// Translate existing operator-local proofs into one deterministic typed API.
///
/// No physical-source DAG realization is inferred. Operator families not yet
/// translated remain visibly residual, even when their legacy local witness
/// is exact, rather than being silently upgraded to full constructive proof.
pub fn local_constructive_witnesses(
    semantics: &ResolvedComposedSemantics,
) -> Vec<ConstructiveWitness> {
    let mut proofs = Vec::new();
    for source in semantics.boolean_witnesses() {
        let witness = source.witness();
        let row = RowVariable::new(
            witness.source_relation(),
            witness.source_relation(),
            "candidate",
        );
        let map = |direction: &BooleanWitnessDirection| match (
            row.as_ref(),
            direction,
            source.boundary_kind(),
        ) {
            (Some(row), BooleanWitnessDirection::Exact(truth), GroupBoundaryKind::Physical) => {
                let case = WitnessCase::new(
                    vec![WitnessObligation::Predicate(WitnessFormula::RowTruth {
                        row: row.clone(),
                        predicate: witness.condition().clone(),
                        truth: *truth,
                    })],
                    ProofStrength::Sufficient,
                );
                case.and_then(|c| WitnessDirection::feasible(vec![c]))
                    .unwrap_or_else(|| WitnessDirection::residual("invalid_boolean_case"))
            }
            (_, BooleanWitnessDirection::Residual { reason }, _) => {
                WitnessDirection::residual(reason)
            }
            (_, _, _) => WitnessDirection::residual("requires_physical_source_realization"),
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Boolean,
            origin_layer_id: source.origin_layer_id().to_string(),
            qualifying: map(witness.qualifying()),
            rejected: map(witness.rejected()),
        });
    }
    for source in semantics.join_witnesses() {
        let translate = |direction: &JoinWitnessDirection| match direction {
            JoinWitnessDirection::Impossible => WitnessDirection::Impossible,
            JoinWitnessDirection::Residual { reason } => WitnessDirection::residual(reason),
            JoinWitnessDirection::Exact(cases) => {
                let (Some(left), Some(right), Some(op)) =
                    (source.left(), source.right(), source.comparison())
                else {
                    return WitnessDirection::residual("missing_join_endpoint");
                };
                let (Some(left_row), Some(right_row)) = (
                    RowVariable::new(left.relation(), left.relation_instance(), "left"),
                    RowVariable::new(right.relation(), right.relation_instance(), "right"),
                ) else {
                    return WitnessDirection::residual("invalid_join_relation_instance");
                };
                let mut results = Vec::new();
                for join_case in cases {
                    let obligation = match join_case.shape() {
                        JoinWitnessShape::Matched => WitnessObligation::JoinPair {
                            left_row: left_row.clone(),
                            right_row: right_row.clone(),
                            left: ColumnRef::new(
                                Some(left.relation().to_string()),
                                left.column().to_string(),
                            ),
                            right: ColumnRef::new(
                                Some(right.relation().to_string()),
                                right.column().to_string(),
                            ),
                            comparison: op,
                            null_extended: join_case.null_extended_side(),
                        },
                        JoinWitnessShape::LeftUnmatched => WitnessObligation::NoMatchingPartner {
                            candidate: left_row.clone(),
                            partner: right_row.clone(),
                            comparison: op,
                            left: ColumnRef::new(
                                Some(left.relation().to_string()),
                                left.column().to_string(),
                            ),
                            right: ColumnRef::new(
                                Some(right.relation().to_string()),
                                right.column().to_string(),
                            ),
                            null_extended: join_case.null_extended_side(),
                            closed_world: true,
                        },
                        JoinWitnessShape::RightUnmatched => WitnessObligation::NoMatchingPartner {
                            candidate: right_row.clone(),
                            partner: left_row.clone(),
                            comparison: op.reversed(),
                            left: ColumnRef::new(
                                Some(right.relation().to_string()),
                                right.column().to_string(),
                            ),
                            right: ColumnRef::new(
                                Some(left.relation().to_string()),
                                left.column().to_string(),
                            ),
                            null_extended: join_case.null_extended_side(),
                            closed_world: true,
                        },
                    };
                    let mut obligations = vec![obligation];
                    if let Some(WitnessObligation::NoMatchingPartner { partner, .. }) =
                        obligations.first()
                    {
                        if !semantics
                            .dependencies()
                            .iter()
                            .any(|relation| relation == partner.relation())
                        {
                            return WitnessDirection::residual(
                                "unproved_physical_partner_relation",
                            );
                        }
                        let Some(boundary) = WitnessBoundary::new(
                            partner.relation(),
                            GroupBoundaryKind::Physical,
                            source.origin_layer_id(),
                        ) else {
                            return WitnessDirection::residual("invalid_partner_boundary");
                        };
                        obligations.push(WitnessObligation::ClosedWorld {
                            boundary,
                            coverage: ClosedWorldCoverage::EntireRelation,
                        });
                    }
                    let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient)
                    else {
                        return WitnessDirection::residual("invalid_join_case");
                    };
                    results.push(case);
                }
                WitnessDirection::feasible(results)
                    .unwrap_or_else(|| WitnessDirection::residual("empty_join_case"))
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Join,
            origin_layer_id: source.origin_layer_id().to_string(),
            qualifying: translate(source.qualifying()),
            rejected: translate(source.rejected()),
        });
    }
    for item in semantics.group_witnesses() {
        let witness = item.witness();
        let translate = |direction: &GroupWitnessDirection| -> WitnessDirection {
            match direction {
                GroupWitnessDirection::Residual { reason } => WitnessDirection::residual(reason),
                GroupWitnessDirection::Exact(cases) if cases.is_empty() => {
                    WitnessDirection::Impossible
                }
                GroupWitnessDirection::Exact(cases) => {
                    if item.boundary_kind() != GroupBoundaryKind::Physical {
                        return WitnessDirection::residual("requires_physical_source_realization");
                    }
                    let Some(boundary_name) = witness.boundary() else {
                        return WitnessDirection::residual("missing_group_boundary");
                    };
                    let Some(boundary) = WitnessBoundary::new(
                        boundary_name,
                        item.boundary_kind(),
                        item.origin_layer_id(),
                    ) else {
                        return WitnessDirection::residual("invalid_group_boundary");
                    };
                    let Some(aggregate) = witness.aggregate() else {
                        return WitnessDirection::residual("missing_group_aggregate");
                    };
                    let mut translated = Vec::new();
                    for case in cases {
                        let (Some(rows), Some(non_null)) = (
                            CountBounds::new(case.min_rows(), case.max_rows()),
                            CountBounds::new(case.min_non_null(), case.max_non_null()),
                        ) else {
                            return WitnessDirection::residual("invalid_group_cardinality");
                        };
                        if non_null.minimum() > rows.maximum().unwrap_or(u64::MAX) {
                            return WitnessDirection::residual("unsatisfiable_group_cardinality");
                        }
                        let Some(translated_case) = WitnessCase::new(
                            vec![WitnessObligation::Group {
                                boundary: boundary.clone(),
                                key: witness.group_keys().to_vec(),
                                aggregate,
                                distinct: witness.distinct(),
                                argument: witness.argument().cloned(),
                                rows,
                                non_null,
                                tests: case.tests().to_vec(),
                            }],
                            ProofStrength::Sufficient,
                        ) else {
                            return WitnessDirection::residual("invalid_group_case");
                        };
                        translated.push(translated_case);
                    }
                    WitnessDirection::feasible(translated)
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_group_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Group,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(witness.qualifying()),
            rejected: translate(witness.rejected()),
        });
    }
    for item in semantics.window_witnesses() {
        let witness = item.witness();
        let translate = |direction: &WindowWitnessDirection| -> WitnessDirection {
            match direction {
                WindowWitnessDirection::Impossible => WitnessDirection::Impossible,
                WindowWitnessDirection::Residual { reason } => WitnessDirection::residual(reason),
                WindowWitnessDirection::Exact(case) => {
                    if item.boundary_kind() != GroupBoundaryKind::Physical {
                        return WitnessDirection::residual("requires_physical_source_realization");
                    }
                    let Some(boundary_name) = witness.boundary() else {
                        return WitnessDirection::residual("missing_window_boundary");
                    };
                    let (Some(boundary), Some(candidate), Some(preceding)) = (
                        WitnessBoundary::new(
                            boundary_name,
                            item.boundary_kind(),
                            item.origin_layer_id(),
                        ),
                        RowVariable::new(boundary_name, boundary_name, "candidate"),
                        CountBounds::new(case.min_preceding(), case.max_preceding()),
                    ) else {
                        return WitnessDirection::residual("invalid_window_case");
                    };
                    let Some(translated) = WitnessCase::new(
                        vec![WitnessObligation::Ranked {
                            boundary,
                            candidate,
                            partition_by: witness.partition_by().to_vec(),
                            order_by: witness.order_by().to_vec(),
                            preceding,
                            strict_unique: true,
                            closed_world: true,
                        }],
                        ProofStrength::Sufficient,
                    ) else {
                        return WitnessDirection::residual("invalid_window_case");
                    };
                    WitnessDirection::feasible(vec![translated])
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_window_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Window,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(witness.qualifying()),
            rejected: translate(witness.rejected()),
        });
    }
    for item in semantics.subquery_witnesses() {
        let witness = item.witness();
        let translate = |direction: &SubqueryMembershipDirection| -> WitnessDirection {
            match direction {
                SubqueryMembershipDirection::Residual { reason } => {
                    WitnessDirection::residual(reason)
                }
                SubqueryMembershipDirection::Exact(cases) if cases.is_empty() => {
                    WitnessDirection::Impossible
                }
                SubqueryMembershipDirection::Exact(cases) => {
                    if item.boundary_kind() != GroupBoundaryKind::Physical {
                        return WitnessDirection::residual("requires_physical_source_realization");
                    }
                    let Some(inner_name) = witness.inner_relation() else {
                        return WitnessDirection::residual("missing_inner_source_identity");
                    };
                    // Later transformations may introduce unrelated physical sources.
                    // The original witness retains its outer physical relation.
                    let Some(outer_name) = witness.outer_source_relation() else {
                        return WitnessDirection::residual("unresolved_outer_source_identity");
                    };
                    let (Some(outer), Some(inner)) = (
                        RowVariable::new(outer_name, witness.outer_relation(), "candidate"),
                        WitnessBoundary::new(
                            inner_name,
                            item.boundary_kind(),
                            item.origin_layer_id(),
                        ),
                    ) else {
                        return WitnessDirection::residual("invalid_subquery_identity");
                    };
                    let mut translated = Vec::new();
                    for case in cases {
                        let Some(translated_case) = WitnessCase::new(
                            vec![
                                WitnessObligation::Membership {
                                    outer: outer.clone(),
                                    inner: inner.clone(),
                                    case: *case,
                                    correlations: witness.correlations().to_vec(),
                                    membership_key: witness.membership_key().cloned(),
                                    inner_domains: witness.inner_column_domains().to_vec(),
                                    closed_world: true,
                                },
                                WitnessObligation::ClosedWorld {
                                    boundary: inner.clone(),
                                    coverage: ClosedWorldCoverage::EntireRelation,
                                },
                            ],
                            ProofStrength::Sufficient,
                        ) else {
                            return WitnessDirection::residual("invalid_membership_case");
                        };
                        translated.push(translated_case);
                    }
                    WitnessDirection::feasible(translated)
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_membership_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Subquery,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(witness.qualifying()),
            rejected: translate(witness.rejected()),
        });
    }
    for item in semantics.set_operations() {
        let operation = item.operation();
        let (qualifying, rejected) = operation.witness_directions();
        let translate = |direction: SetWitnessDirection| -> WitnessDirection {
            match direction {
                SetWitnessDirection::Residual { reason, .. } => WitnessDirection::residual(reason),
                SetWitnessDirection::Exact(cases) if cases.is_empty() => {
                    WitnessDirection::Impossible
                }
                SetWitnessDirection::Exact(cases) => {
                    let mut translated = Vec::new();
                    for case in cases {
                        let mut obligations = Vec::new();
                        for original in case.obligations() {
                            let source = original.boundary();
                            let kind = if source.is_intermediate() {
                                GroupBoundaryKind::Intermediate
                            } else {
                                GroupBoundaryKind::Physical
                            };
                            // Producer realization is a separate task. A local tuple
                            // count cannot be turned into a direct write at a CTE.
                            if kind != GroupBoundaryKind::Physical {
                                return WitnessDirection::residual(
                                    "requires_physical_source_realization",
                                );
                            }
                            let Some(boundary) = WitnessBoundary::new(
                                source.relation(),
                                kind,
                                item.origin_layer_id(),
                            ) else {
                                return WitnessDirection::residual("invalid_set_boundary");
                            };
                            let Some(branch) = operation
                                .branches()
                                .iter()
                                .find(|branch| branch.identity() == original.branch_identity())
                            else {
                                return WitnessDirection::residual("missing_set_branch_evidence");
                            };
                            obligations.push(WitnessObligation::SetTuple {
                                branch_identity: original.branch_identity().to_string(),
                                boundary: boundary.clone(),
                                tuple_columns: source.tuple_columns().to_vec(),
                                matching_rows: original.matching_tuple_count(),
                                column_domains: branch.column_domains().to_vec(),
                                closed_world: true,
                            });
                            if original.matching_tuple_count() == 0 {
                                obligations.push(WitnessObligation::ClosedWorld {
                                    boundary,
                                    coverage: ClosedWorldCoverage::CandidateTuple {
                                        branch_identity: original.branch_identity().to_string(),
                                        columns: source.tuple_columns().to_vec(),
                                    },
                                });
                            }
                        }
                        obligations.push(WitnessObligation::SetResultTuple {
                            matching_rows: case.output_tuple_count(),
                            nulls_equal: true,
                        });
                        let Some(translated_case) =
                            WitnessCase::new(obligations, ProofStrength::Sufficient)
                        else {
                            return WitnessDirection::residual("invalid_set_case");
                        };
                        translated.push(translated_case);
                    }
                    WitnessDirection::feasible(translated)
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_set_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Set,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(qualifying),
            rejected: translate(rejected),
        });
    }
    proofs
}

