//! Typed constructive obligations, cases, rows, and proof boundaries.

use super::*;

pub(super) fn constructive_witness_to_value(witness: &crate::ConstructiveWitness) -> Value {
    json!({
        "operator": witness.operator().as_str(),
        "origin_layer_id": witness.origin_layer_id(),
        "qualifying": constructive_direction_to_value(witness.qualifying()),
        "rejected": constructive_direction_to_value(witness.rejected())
    })
}

pub(super) fn constructive_direction_to_value(direction: &crate::WitnessDirection) -> Value {
    match direction {
        crate::WitnessDirection::Feasible(cases) => json!({
            "status": "feasible",
            "cases": cases.iter().map(constructive_case_to_value).collect::<Vec<_>>()
        }),
        crate::WitnessDirection::Impossible => json!({ "status": "impossible" }),
        crate::WitnessDirection::Residual { reason } => json!({
            "status": "residual", "reason": reason
        }),
    }
}

pub(super) fn constructive_case_to_value(case: &crate::WitnessCase) -> Value {
    let strength = match case.strength() {
        crate::ProofStrength::Sufficient => "sufficient",
        crate::ProofStrength::Necessary => "necessary",
        crate::ProofStrength::Equivalent => "equivalent",
    };
    json!({
        "strength": strength,
        "obligations": case.obligations().iter().map(constructive_obligation_to_value).collect::<Vec<_>>()
    })
}

pub(super) fn constructive_bounds_to_value(bounds: crate::CountBounds) -> Value {
    json!({ "minimum": bounds.minimum(), "maximum": bounds.maximum() })
}

pub(super) fn constructive_row_to_value(row: &crate::RowVariable) -> Value {
    json!({
        "relation": row.relation(),
        "instance": row.instance(),
        "name": row.name()
    })
}

pub(super) fn constructive_boundary_to_value(boundary: &crate::WitnessBoundary) -> Value {
    json!({
        "relation": boundary.relation(),
        "kind": boundary.kind().as_str(),
        "origin_layer_id": boundary.origin_layer_id()
    })
}

pub(super) fn constructive_term_to_value(term: &crate::WitnessTerm) -> Value {
    match term {
        crate::WitnessTerm::Column { row, column } => json!({
            "kind": "column", "row": constructive_row_to_value(row),
            "column": column_ref_to_value(column)
        }),
        crate::WitnessTerm::Integer(value) => json!({ "kind": "integer", "value": value }),
        crate::WitnessTerm::Boolean(value) => json!({ "kind": "boolean", "value": value }),
        crate::WitnessTerm::Null => json!({ "kind": "null" }),
    }
}

pub(super) fn constructive_formula_to_value(formula: &crate::WitnessFormula) -> Value {
    match formula {
        crate::WitnessFormula::All(operands) => json!({
            "kind": "all",
            "operands": operands.iter().map(constructive_formula_to_value).collect::<Vec<_>>()
        }),
        crate::WitnessFormula::Any(operands) => json!({
            "kind": "any",
            "operands": operands.iter().map(constructive_formula_to_value).collect::<Vec<_>>()
        }),
        crate::WitnessFormula::Not(operand) => json!({
            "kind": "not", "operand": constructive_formula_to_value(operand)
        }),
        crate::WitnessFormula::RowTruth {
            row,
            predicate,
            truth,
        } => json!({
            "kind": "row_truth",
            "row": constructive_row_to_value(row),
            "predicate": boolean_constraint_to_value(predicate),
            "truth": truth.as_str()
        }),
        crate::WitnessFormula::Comparison {
            left,
            operator,
            right,
        } => json!({
            "kind": "comparison",
            "left": constructive_term_to_value(left),
            "operator": operator.as_str(),
            "right": constructive_term_to_value(right)
        }),
        crate::WitnessFormula::IsNull { term, negated } => json!({
            "kind": "is_null",
            "term": constructive_term_to_value(term),
            "negated": negated
        }),
        crate::WitnessFormula::TupleComparison { left, equal, right } => json!({
            "kind": "tuple_comparison",
            "left": left.iter().map(constructive_term_to_value).collect::<Vec<_>>(),
            "equal": equal,
            "right": right.iter().map(constructive_term_to_value).collect::<Vec<_>>()
        }),
        crate::WitnessFormula::StringPrefix {
            term,
            prefix,
            negated,
        } => json!({
            "kind": "string_prefix",
            "term": constructive_term_to_value(term),
            "prefix": prefix,
            "negated": negated
        }),
    }
}

pub(super) fn constructive_obligation_to_value(obligation: &crate::WitnessObligation) -> Value {
    match obligation {
        crate::WitnessObligation::Predicate(formula) => json!({
            "kind": "predicate",
            "formula": constructive_formula_to_value(formula)
        }),
        crate::WitnessObligation::Rows {
            boundary,
            quantifier,
            bounds,
            predicate,
            closed_world,
        } => json!({
            "kind": "rows",
            "boundary": constructive_boundary_to_value(boundary),
            "quantifier": match quantifier {
                crate::RowQuantifier::Exists => "exists",
                crate::RowQuantifier::ForAll => "for_all"
            },
            "bounds": constructive_bounds_to_value(*bounds),
            "predicate": constructive_formula_to_value(predicate),
            "closed_world": closed_world
        }),
        crate::WitnessObligation::NoMatchingPartner {
            candidate,
            partner,
            comparison,
            left,
            right,
            null_extended,
            closed_world,
        } => json!({
            "kind": "no_matching_partner",
            "candidate": constructive_row_to_value(candidate),
            "partner": constructive_row_to_value(partner),
            "comparison": comparison.as_str(),
            "left": column_ref_to_value(left),
            "right": column_ref_to_value(right),
            "null_extended": null_extended.map(|side| side.as_str()),
            "closed_world": closed_world
        }),
        crate::WitnessObligation::JoinPair {
            left_row,
            right_row,
            left,
            right,
            comparison,
            null_extended,
        } => json!({
            "kind": "join_pair",
            "left_row": constructive_row_to_value(left_row),
            "right_row": constructive_row_to_value(right_row),
            "left": column_ref_to_value(left),
            "right": column_ref_to_value(right),
            "comparison": comparison.as_str(),
            "null_extended": null_extended.map(|side| side.as_str())
        }),
        crate::WitnessObligation::JoinPopulation {
            left_row,
            right_row,
            left_key,
            right_key,
            additional_keys,
            join_kind,
            pattern,
            left_rows,
            right_rows,
            output_rows,
            closed_world,
        } => {
            let mut value = json!({
                "kind": "join_population",
                "left_row": constructive_row_to_value(left_row),
                "right_row": constructive_row_to_value(right_row),
                "left_key": column_ref_to_value(left_key),
                "right_key": column_ref_to_value(right_key),
                "join_kind": join_kind.as_str(),
                "pattern": pattern.as_str(),
                "left_rows": left_rows,
                "right_rows": right_rows,
                "output_rows": output_rows,
                "closed_world": closed_world
            });
            if !additional_keys.is_empty() {
                value["additional_keys"] = json!(additional_keys
                    .iter()
                    .map(|key| json!({
                        "left": column_ref_to_value(key.left()),
                        "right": column_ref_to_value(key.right())
                    }))
                    .collect::<Vec<_>>());
            }
            if let Some(groups) = pattern.group_count() {
                value["pattern_groups"] = json!(groups);
            }
            value
        }
        crate::WitnessObligation::UniformJoinPopulation {
            left_row,
            right_row,
            left_key,
            right_key,
            law,
            left_value,
            right_value,
            left_rows,
            right_rows,
            output_rows,
            closed_world,
        } => json!({
            "kind": "uniform_join_population",
            "left_row": constructive_row_to_value(left_row),
            "right_row": constructive_row_to_value(right_row),
            "left_key": left_key.as_ref().map(column_ref_to_value),
            "right_key": right_key.as_ref().map(column_ref_to_value),
            "law": law.as_str(),
            "left_value": left_value,
            "right_value": right_value,
            "left_rows": left_rows,
            "right_rows": right_rows,
            "output_rows": output_rows,
            "closed_world": closed_world
        }),
        crate::WitnessObligation::JoinChainPopulation {
            left_row,
            middle_row,
            right_row,
            left_key,
            middle_left_key,
            middle_right_key,
            right_key,
            matched_value,
            left_rows,
            middle_rows,
            right_rows,
            output_rows,
            closed_world,
        } => json!({
            "kind": "join_chain_population",
            "left_row": constructive_row_to_value(left_row),
            "middle_row": constructive_row_to_value(middle_row),
            "right_row": constructive_row_to_value(right_row),
            "left_key": column_ref_to_value(left_key),
            "middle_left_key": column_ref_to_value(middle_left_key),
            "middle_right_key": column_ref_to_value(middle_right_key),
            "right_key": column_ref_to_value(right_key),
            "matched_value": matched_value,
            "left_rows": left_rows,
            "middle_rows": middle_rows,
            "right_rows": right_rows,
            "output_rows": output_rows,
            "closed_world": closed_world,
        }),
        crate::WitnessObligation::OperatorPopulation { witness } => json!({
            "kind": "operator_population",
            "witness": outcome_witness_to_value(witness)
        }),
        crate::WitnessObligation::Group {
            boundary,
            key,
            aggregate,
            distinct,
            argument,
            rows,
            non_null,
            tests,
        } => json!({
            "kind": "group",
            "boundary": constructive_boundary_to_value(boundary),
            "key": key.iter().map(column_ref_to_value).collect::<Vec<_>>(),
            "aggregate": aggregate.as_str(),
            "distinct": distinct,
            "argument": argument.as_ref().map_or(Value::Null, column_ref_to_value),
            "rows": constructive_bounds_to_value(*rows),
            "non_null": constructive_bounds_to_value(*non_null),
            "tests": tests.iter().map(|test| json!({
                "kind": test.kind(),
                "operator": test.operator().as_str(),
                "bound": literal_expression_to_value(test.bound())
            })).collect::<Vec<_>>()
        }),
        crate::WitnessObligation::Ranked {
            boundary,
            candidate,
            partition_by,
            order_by,
            preceding,
            strict_unique,
            closed_world,
        } => json!({
            "kind": "ranked",
            "boundary": constructive_boundary_to_value(boundary),
            "candidate": constructive_row_to_value(candidate),
            "partition_by": partition_by.iter().map(column_ref_to_value).collect::<Vec<_>>(),
            "order_by": order_by.iter().map(|key| json!({
                "column": column_ref_to_value(key.column()),
                "ascending": key.ascending(),
                "nulls_first": key.nulls_first()
            })).collect::<Vec<_>>(),
            "preceding": constructive_bounds_to_value(*preceding),
            "strict_unique": strict_unique,
            "closed_world": closed_world
        }),
        crate::WitnessObligation::RankPopulation {
            boundary,
            partition_key,
            order,
            partitions,
            rows_per_partition,
            rank_limit,
            closed_world,
        } => json!({
            "kind": "rank_population",
            "boundary": constructive_boundary_to_value(boundary),
            "partition_key": column_ref_to_value(partition_key),
            "order": {
                "column": column_ref_to_value(order.column()),
                "ascending": order.ascending(),
                "nulls_first": order.nulls_first()
            },
            "partitions": partitions,
            "rows_per_partition": rows_per_partition,
            "rank_limit": rank_limit,
            "closed_world": closed_world
        }),
        crate::WitnessObligation::Membership {
            outer,
            inner,
            case,
            correlations,
            membership_key,
            inner_domains,
            closed_world,
        } => json!({
            "kind": "membership",
            "outer": constructive_row_to_value(outer),
            "inner": constructive_boundary_to_value(inner),
            "case": case.as_str(),
            "correlations": correlations.iter().map(|pair| json!({
                "outer": column_ref_to_value(pair.outer()),
                "inner": column_ref_to_value(pair.inner())
            })).collect::<Vec<_>>(),
            "membership_key": membership_key.as_ref().map(|pair| json!({
                "outer": column_ref_to_value(pair.outer()),
                "inner": column_ref_to_value(pair.inner())
            })),
            "inner_domains": inner_domains.iter().map(column_domain_to_value).collect::<Vec<_>>(),
            "closed_world": closed_world
        }),
        crate::WitnessObligation::SetTuple {
            branch_identity,
            boundary,
            tuple_columns,
            matching_rows,
            column_domains,
            closed_world,
        } => json!({
            "kind": "set_tuple",
            "branch_identity": branch_identity,
            "boundary": constructive_boundary_to_value(boundary),
            "tuple_columns": tuple_columns,
            "matching_rows": matching_rows,
            "column_domains": column_domains.iter().map(column_domain_to_value).collect::<Vec<_>>(),
            "closed_world": closed_world
        }),
        crate::WitnessObligation::SetResultTuple {
            matching_rows,
            nulls_equal,
        } => json!({
            "kind": "set_result_tuple", "matching_rows": matching_rows, "nulls_equal": nulls_equal
        }),
        crate::WitnessObligation::OutputRows { layer_id, bounds } => json!({
            "kind": "output_rows",
            "layer_id": layer_id,
            "bounds": constructive_bounds_to_value(*bounds)
        }),
        crate::WitnessObligation::StateRows {
            relation,
            before,
            after,
        } => json!({
            "kind": "state_rows",
            "relation": relation,
            "before": constructive_bounds_to_value(*before),
            "after": constructive_bounds_to_value(*after)
        }),
        crate::WitnessObligation::ClosedWorld { boundary, coverage } => json!({
            "kind": "closed_world",
            "boundary": constructive_boundary_to_value(boundary),
            "coverage": match coverage {
                crate::ClosedWorldCoverage::EntireRelation => json!({
                    "kind": "entire_relation"
                }),
                crate::ClosedWorldCoverage::CandidateTuple {
                    branch_identity,
                    columns,
                } => json!({
                    "kind": "candidate_tuple",
                    "branch_identity": branch_identity,
                    "columns": columns
                })
            }
        }),
        crate::WitnessObligation::Producer {
            boundary,
            physical_sources,
        } => json!({
            "kind": "producer",
            "boundary": constructive_boundary_to_value(boundary),
            "physical_sources": physical_sources
        }),
    }
}
