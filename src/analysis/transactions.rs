//! Transaction SQL recognition at the sqlparser boundary.
//!
//! Physical execution consumes parser-independent controls and canonical SQL
//! bundles. AST variants, vendor options and parser errors never leak into
//! the public physical proof contract.

use sqlparser::ast::Statement;

use crate::bundle::{analyze_inputs, AnalysisBundle, SqlInput};
use crate::parser::parse_sql;
use crate::physical_realization::{
    physical_duckdb_transaction_plan, PhysicalScriptCatalog, PhysicalTransactionScriptProof,
    PhysicalTransactionStep,
};

enum TransactionSqlUnit {
    Begin,
    Commit,
    Rollback,
    Savepoint(String),
    RollbackTo(String),
    Release(String),
    Sql(AnalysisBundle),
}

fn residual(index: usize, reason: impl Into<String>) -> PhysicalTransactionScriptProof {
    PhysicalTransactionScriptProof::Residual {
        step_index: index,
        reason: reason.into(),
    }
}

/// Analyze individual SQL statements as a DuckDB transaction script, without
/// treating parser support as proof of an engine/session law.
///
/// Each string must be exactly one statement. BEGIN/COMMIT/ROLLBACK are
/// lowered to typed transaction controls only for the plain syntax accepted
/// by the DuckDB contract. Vendor-specific modifiers, compound BEGIN blocks,
/// ambiguous SQL input and parse errors remain Residual. Other statements
/// pass through the canonical SQL analyzer before physical proof.
///
/// This entry point is for a single connection and complete closed-world
/// initial catalog. It does not execute SQL or infer implicit transactions.
pub fn physical_duckdb_sql_transaction_plan(
    sql_statements: &[&str],
    initial: &PhysicalScriptCatalog,
) -> PhysicalTransactionScriptProof {
    let Some(dialect) = crate::dialect_from_name("duckdb") else {
        return residual(0, "DuckDB parser dialect unavailable");
    };
    let mut units = Vec::with_capacity(sql_statements.len());
    for (index, sql) in sql_statements.iter().enumerate() {
        let parsed = match parse_sql(sql, dialect.as_ref()) {
            Ok(parsed) => parsed,
            Err(error) => return residual(index, format!("DuckDB parser boundary: {error}")),
        };
        let [statement] = parsed.statements.as_slice() else {
            return residual(index, "transaction SQL input must contain exactly one statement");
        };
        let control = match statement {
            Statement::StartTransaction {
                modes,
                begin: true,
                modifier: None,
                statements,
                exception: None,
                has_end_keyword: false,
                ..
            } if modes.is_empty() && statements.is_empty() => TransactionSqlUnit::Begin,
            Statement::Commit {
                chain: false,
                end: false,
                modifier: None,
            } => TransactionSqlUnit::Commit,
            Statement::Rollback {
                chain: false,
                savepoint: None,
            } => TransactionSqlUnit::Rollback,
            Statement::Rollback {
                chain: false,
                savepoint: Some(name),
            } => TransactionSqlUnit::RollbackTo(name.value.clone()),
            Statement::Savepoint { name } => TransactionSqlUnit::Savepoint(name.value.clone()),
            Statement::ReleaseSavepoint { name } => TransactionSqlUnit::Release(name.value.clone()),
            Statement::StartTransaction { .. }
            | Statement::Commit { .. }
            | Statement::Rollback { .. } => {
                return residual(index, "unsupported DuckDB transaction modifier or compound block");
            }
            _ => {
                let bundle = match analyze_inputs(
                    &[SqlInput::inline(*sql)],
                    "duckdb",
                    dialect.as_ref(),
                ) {
                    Ok(bundle) => bundle,
                    Err(error) => return residual(index, format!("canonical SQL boundary: {error}")),
                };
                TransactionSqlUnit::Sql(bundle)
            }
        };
        units.push(control);
    }
    let steps = units
        .iter()
        .map(|unit| match unit {
            TransactionSqlUnit::Begin => PhysicalTransactionStep::Begin,
            TransactionSqlUnit::Commit => PhysicalTransactionStep::Commit,
            TransactionSqlUnit::Rollback => PhysicalTransactionStep::Rollback,
            TransactionSqlUnit::Savepoint(name) => PhysicalTransactionStep::Savepoint(name),
            TransactionSqlUnit::RollbackTo(name) => PhysicalTransactionStep::RollbackTo(name),
            TransactionSqlUnit::Release(name) => PhysicalTransactionStep::Release(name),
            TransactionSqlUnit::Sql(bundle) => PhysicalTransactionStep::Sql(bundle),
        })
        .collect::<Vec<_>>();
    physical_duckdb_transaction_plan(&steps, initial)
}
