//! Aggregate grouping and distinctness semantics.

use super::*;

/// SELECT-level duplicate elimination and grouping semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregation {
    distinct: bool,
    distinct_on: Vec<Expression>,
    group_by: Option<GroupBy>,
}

impl Aggregation {
    pub(crate) fn new(
        distinct: bool,
        distinct_on: Vec<Expression>,
        group_by: Option<GroupBy>,
    ) -> Self {
        Self {
            distinct,
            distinct_on,
            group_by,
        }
    }

    /// Return whether the SELECT eliminates duplicate output rows.
    pub fn distinct(&self) -> bool {
        self.distinct
    }

    /// Return DISTINCT ON expressions in SQL order.
    pub fn distinct_on(&self) -> &[Expression] {
        &self.distinct_on
    }

    /// Return GROUP BY semantics when grouping is present.
    pub fn group_by(&self) -> Option<&GroupBy> {
        self.group_by.as_ref()
    }
}

/// Normalized GROUP BY form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupBy {
    /// GROUP BY ALL.
    All,
    /// Explicit grouping expressions in SQL order.
    Expressions(Vec<GroupingExpression>),
}

/// One parser-independent grouping element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupingExpression {
    /// An ordinary GROUP BY expression.
    Expression(Expression),
    /// GROUPING SETS, with one vector per set.
    GroupingSets(Vec<Vec<Expression>>),
    /// ROLLUP, preserving parser-provided grouping levels.
    Rollup(Vec<Vec<Expression>>),
    /// CUBE, preserving parser-provided grouping levels.
    Cube(Vec<Vec<Expression>>),
}
