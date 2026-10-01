use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use sql_semantic_protocol::{
    analyze_sql, to_json, AnalysisError, Error as ProtocolError, ParseError,
};
use sqlparser::dialect::{Dialect, GenericDialect, SnowflakeDialect};

const USAGE: &str = "Usage: sql-semantic-protocol [--dialect <generic|snowflake>] [--file <path>] [SQL ...]\n\nIf neither --file nor SQL is supplied, SQL is read from stdin.\nUse -- to pass positional SQL that starts with a dash.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn run() -> Result<(), CliError> {
    match parse_args(env::args().skip(1))? {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Analyze(options) => {
            let sql = read_sql(&options)?;
            let (dialect_name, dialect) = select_dialect(&options.dialect)?;
            let protocol =
                analyze_sql(&sql, dialect_name, dialect.as_ref()).map_err(CliError::from)?;
            println!("{}", to_json(&protocol));
            Ok(())
        }
    }
}

#[derive(Debug)]
enum Command {
    Analyze(Options),
    Help,
}

#[derive(Debug)]
struct Options {
    dialect: String,
    file: Option<PathBuf>,
    positional_sql: Vec<String>,
}

fn parse_args(arguments: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let mut arguments = arguments.peekable();
    let mut dialect = "generic".to_string();
    let mut file = None;
    let mut positional_sql = Vec::new();
    let mut positional_only = false;

    while let Some(argument) = arguments.next() {
        if positional_only {
            positional_sql.push(argument);
            continue;
        }

        match argument.as_str() {
            "--" => positional_only = true,
            "-h" | "--help" => return Ok(Command::Help),
            "-d" | "--dialect" => {
                dialect = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --dialect".to_string()))?;
            }
            "-f" | "--file" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --file".to_string()))?;
                if file.replace(PathBuf::from(path)).is_some() {
                    return Err(CliError::Input(
                        "--file may only be specified once".to_string(),
                    ));
                }
            }
            _ if argument.starts_with('-') => {
                return Err(CliError::Input(format!("unknown option: {argument}")));
            }
            _ => positional_sql.push(argument),
        }
    }

    if file.is_some() && !positional_sql.is_empty() {
        return Err(CliError::Input(
            "cannot combine --file with positional SQL".to_string(),
        ));
    }

    Ok(Command::Analyze(Options {
        dialect,
        file,
        positional_sql,
    }))
}

fn read_sql(options: &Options) -> Result<String, CliError> {
    let sql = if let Some(path) = &options.file {
        fs::read_to_string(path).map_err(|error| {
            CliError::Input(format!("failed to read {}: {error}", path.display()))
        })?
    } else if !options.positional_sql.is_empty() {
        options.positional_sql.join(" ")
    } else {
        let mut sql = String::new();
        io::stdin()
            .read_to_string(&mut sql)
            .map_err(|error| CliError::Input(format!("failed to read stdin: {error}")))?;
        sql
    };

    if sql.trim().is_empty() {
        return Err(CliError::Input("SQL input is empty".to_string()));
    }

    Ok(sql)
}

fn select_dialect(name: &str) -> Result<(&'static str, Box<dyn Dialect>), CliError> {
    match name.to_ascii_lowercase().as_str() {
        "generic" => Ok(("generic", Box::new(GenericDialect {}))),
        "snowflake" => Ok(("snowflake", Box::new(SnowflakeDialect {}))),
        _ => Err(CliError::Input(format!(
            "unsupported dialect '{name}'; supported dialects: generic, snowflake"
        ))),
    }
}

#[derive(Debug)]
enum CliError {
    Input(String),
    Parse(ParseError),
    Analysis(AnalysisError),
}

impl CliError {
    fn exit_code(&self) -> ExitCode {
        match self {
            Self::Input(_) => ExitCode::from(2),
            Self::Parse(_) => ExitCode::from(3),
            Self::Analysis(_) => ExitCode::from(4),
        }
    }
}

impl From<ProtocolError> for CliError {
    fn from(error: ProtocolError) -> Self {
        match error {
            ProtocolError::Parse(error) => Self::Parse(error),
            ProtocolError::Analysis(error) => Self::Analysis(error),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(message) => write!(formatter, "input error: {message}"),
            Self::Parse(error) => write!(formatter, "{error}"),
            Self::Analysis(error) => write!(formatter, "analysis error: {error}"),
        }
    }
}
