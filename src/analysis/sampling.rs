//! Normalize SQL sampling syntax into source-independent, conservatively residual facts.

use super::*;
use sqlparser::ast::{
    TableSampleKind, TableSampleMethod, TableSampleModifier, TableSampleSeedModifier,
    TableSampleUnit,
};

pub(super) fn analyze_query_sampling(query: &SqlQuery) -> Vec<TableSampling> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut source_instance = 0usize;
    for from in &select.from {
        if let Some(sample) = sample_from_factor(&from.relation, source_instance) {
            found.push(sample);
        }
        source_instance += 1;
        for join in &from.joins {
            if let Some(sample) = sample_from_factor(&join.relation, source_instance) {
                found.push(sample);
            }
            source_instance += 1;
        }
    }
    found
}

fn sample_from_factor(factor: &TableFactor, source_instance: usize) -> Option<TableSampling> {
    let TableFactor::Table {
        name,
        sample: Some(kind),
        ..
    } = factor
    else {
        return None;
    };
    let sample = match kind {
        TableSampleKind::BeforeTableAlias(sample) | TableSampleKind::AfterTableAlias(sample) => {
            sample
        }
    };
    let syntax = match &sample.modifier {
        TableSampleModifier::Sample => SamplingSyntax::Sample,
        TableSampleModifier::TableSample => SamplingSyntax::TableSample,
    };
    let method = sample.name.as_ref().map(|method| match method {
        TableSampleMethod::Row => SamplingMethod::Row,
        TableSampleMethod::Bernoulli => SamplingMethod::Bernoulli,
        TableSampleMethod::System => SamplingMethod::System,
        TableSampleMethod::Block => SamplingMethod::Block,
    });
    let quantity = sample.quantity.as_ref().map(|quantity| {
        let value = match &quantity.value {
            Expr::Value(value) => sampling_literal(&value.value),
            _ => None,
        };
        let unit = quantity.unit.as_ref().map(|unit| match unit {
            TableSampleUnit::Rows => SamplingUnit::Rows,
            TableSampleUnit::Percent => SamplingUnit::Percent,
        });
        SamplingQuantity::new(value, unit)
    });
    let seed = sample.seed.as_ref().map(|seed| {
        let syntax = match &seed.modifier {
            TableSampleSeedModifier::Seed => SamplingSeedSyntax::Seed,
            TableSampleSeedModifier::Repeatable => SamplingSeedSyntax::Repeatable,
        };
        SamplingSeed::new(syntax, sampling_literal(&seed.value))
    });
    Some(TableSampling::new(
        name.to_string(),
        source_instance,
        syntax,
        method,
        quantity,
        seed,
        sample.bucket.is_some() || sample.offset.is_some(),
    ))
}

fn sampling_literal(value: &Value) -> Option<LiteralValue> {
    match value {
        Value::Number(number, _) => Some(LiteralValue::Number(number.clone())),
        Value::SingleQuotedString(text) => Some(LiteralValue::Text(text.clone())),
        Value::Boolean(boolean) => Some(LiteralValue::Boolean(*boolean)),
        Value::Null => Some(LiteralValue::Null),
        _ => None,
    }
}
