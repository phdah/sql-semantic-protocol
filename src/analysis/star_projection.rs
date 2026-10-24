//! Shared AST boundary for named wildcard column modification laws.
//!
//! Both projected output domains and exact finite physical rows use this
//! positional plan. No SQL AST escapes into the canonical protocol.

use sqlparser::ast::{ExcludeSelectItem, Expr, RenameSelectItem, WildcardAdditionalOptions};
use std::collections::BTreeSet;

/// Validated position-based wildcard change independent of downstream consumers.
pub(super) enum StarModification {
    /// No modification, just ordinary schema-attested wildcard expansion.
    Identity,
    /// Remove named unique positions from the output.
    Exclude(Vec<usize>),
    /// Rename an existing position without changing its value.
    Rename(Vec<(usize, String)>),
    /// Replace expressions at named positions, preserving column order.
    Replace(Vec<(usize, Expr)>),
}

/// Return one unambiguous SQL modifier plan for the *complete ordered schema*.
/// Missing, duplicate, ambiguous or mixed modifiers are explicitly rejected.
pub(super) fn plan_star_modification(
    options: &WildcardAdditionalOptions,
    columns: &[String],
) -> Result<StarModification, &'static str> {
    let count = usize::from(options.opt_except.is_some())
        + usize::from(options.opt_exclude.is_some())
        + usize::from(options.opt_rename.is_some())
        + usize::from(options.opt_replace.is_some())
        + usize::from(options.opt_ilike.is_some());
    if count == 0 {
        return Ok(StarModification::Identity);
    }
    if count != 1 || options.opt_ilike.is_some() {
        return Err("mixed wildcard modifiers or ILIKE require unproved ordering/collation");
    }
    let resolve = |name: &str| -> Result<usize, &'static str> {
        let mut matching = columns
            .iter()
            .enumerate()
            .filter(|(_, value)| value.as_str() == name);
        let pos = matching
            .next()
            .map(|(pos, _)| pos)
            .ok_or("wildcard modifier target not in the complete schema")?;
        if matching.next().is_some() {
            return Err("wildcard modifier target has ambiguous source columns");
        }
        Ok(pos)
    };
    if let Some(except) = &options.opt_except {
        let names = std::iter::once(&except.first_element)
            .chain(except.additional_elements.iter())
            .map(|column| column.value.as_str());
        let mut positions = BTreeSet::new();
        for name in names {
            if !positions.insert(resolve(name)?) {
                return Err("repeated wildcard exclusion target");
            }
        }
        if positions.len() == columns.len() {
            return Err("wildcard exclusion removes all output columns");
        }
        return Ok(StarModification::Exclude(positions.into_iter().collect()));
    }
    if let Some(exclude) = &options.opt_exclude {
        let names = match exclude {
            ExcludeSelectItem::Single(column) => vec![column],
            ExcludeSelectItem::Multiple(columns) => columns.iter().collect(),
        };
        let mut positions = BTreeSet::new();
        for name in names {
            if !positions.insert(resolve(&name.value)?) {
                return Err("repeated wildcard exclusion target");
            }
        }
        if positions.len() == columns.len() {
            return Err("wildcard exclusion removes all output columns");
        }
        return Ok(StarModification::Exclude(positions.into_iter().collect()));
    }
    if let Some(rename) = &options.opt_rename {
        let pairs = match rename {
            RenameSelectItem::Single(pair) => vec![pair],
            RenameSelectItem::Multiple(pairs) => pairs.iter().collect(),
        };
        let mut positions = BTreeSet::new();
        let mut final_names = columns.to_vec();
        let mut edits = Vec::new();
        for pair in pairs {
            let position = resolve(&pair.ident.value)?;
            if !positions.insert(position) {
                return Err("repeated wildcard rename target");
            }
            final_names[position] = pair.alias.value.clone();
            edits.push((position, pair.alias.value.clone()));
        }
        if final_names.iter().collect::<BTreeSet<_>>().len() != final_names.len() {
            return Err("wildcard rename creates ambiguous or duplicate output columns");
        }
        return Ok(StarModification::Rename(edits));
    }
    if let Some(replace) = &options.opt_replace {
        let mut positions = BTreeSet::new();
        let mut edits = Vec::new();
        for item in &replace.items {
            let position = resolve(&item.column_name.value)?;
            if !positions.insert(position) {
                return Err("repeated wildcard replacement target");
            }
            edits.push((position, item.expr.clone()));
        }
        return Ok(StarModification::Replace(edits));
    }
    Err("wildcard modifier has no implemented semantic plan")
}
