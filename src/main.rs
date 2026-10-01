use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use sql_semantic_protocol::{
    analyze_inputs, analyze_sql, to_bundle_json, to_json, Error as ProtocolError,
    InputAnalysisError, SqlInput,
};
use sqlparser::dialect::{dialect_from_str, Dialect};

const USAGE: &str = "Usage: sql-semantic-protocol [--dialect <name>] [--sql <SQL>]... [--file <path>]... [SQL ...]\n\nRepeat --sql and --file to analyze multiple inputs in command-line order.\nLegacy positional SQL remains one input. If no input is supplied, SQL is read from stdin.\nThe dialect defaults to generic and may be any built-in dialect recognized by sqlparser.\nUse -- to pass positional SQL that starts with a dash.";

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
            let inputs = read_inputs(&options)?;
            let (dialect_name, dialect) = select_dialect(&options.dialect)?;

            if inputs.len() == 1 {
                let protocol = analyze_sql(inputs[0].sql(), &dialect_name, dialect.as_ref())
                    .map_err(CliError::Protocol)?;
                println!("{}", to_json(&protocol));
            } else {
                let bundle = analyze_inputs(&inputs, &dialect_name, dialect.as_ref())
                    .map_err(CliError::InputProtocol)?;
                println!("{}", to_bundle_json(&bundle));
            }

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
    inputs: Vec<InputArgument>,
    positional_sql: Vec<String>,
}

#[derive(Debug)]
enum InputArgument {
    Inline(String),
    File(PathBuf),
}

fn parse_args(mut arguments: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let mut dialect = "generic".to_string();
    let mut inputs = Vec::new();
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
            "-s" | "--sql" => {
                let sql = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --sql".to_string()))?;
                inputs.push(InputArgument::Inline(sql));
            }
            "-f" | "--file" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --file".to_string()))?;
                inputs.push(InputArgument::File(PathBuf::from(path)));
            }
            _ if argument.starts_with('-') => {
                return Err(CliError::Input(format!("unknown option: {argument}")));
            }
            _ => positional_sql.push(argument),
        }
    }

    if !inputs.is_empty() && !positional_sql.is_empty() {
        return Err(CliError::Input(
            "cannot combine positional SQL with --sql or --file; use --sql for explicit inputs"
                .to_string(),
        ));
    }

    Ok(Command::Analyze(Options {
        dialect,
        inputs,
        positional_sql,
    }))
}

fn read_inputs(options: &Options) -> Result<Vec<SqlInput>, CliError> {
    if !options.inputs.is_empty() {
        return options
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| read_explicit_input(input, index + 1))
            .collect();
    }

    if !options.positional_sql.is_empty() {
        let sql = options.positional_sql.join(" ");
        ensure_non_empty_sql(&sql, "positional SQL")?;
        return Ok(vec![SqlInput::inline(sql)]);
    }

    let mut sql = String::new();
    io::stdin()
        .read_to_string(&mut sql)
        .map_err(|error| CliError::Input(format!("failed to read stdin: {error}")))?;
    ensure_non_empty_sql(&sql, "stdin")?;

    Ok(vec![SqlInput::inline(sql)])
}

fn read_explicit_input(input: &InputArgument, position: usize) -> Result<SqlInput, CliError> {
    match input {
        InputArgument::Inline(sql) => {
            ensure_non_empty_sql(sql, &format!("input {position} (--sql)"))?;
            Ok(SqlInput::inline(sql.clone()))
        }
        InputArgument::File(path) => {
            let sql = fs::read_to_string(path).map_err(|error| {
                CliError::Input(format!(
                    "input {position} (file '{}'): failed to read: {error}",
                    path.display()
                ))
            })?;
            ensure_non_empty_sql(
                &sql,
                &format!("input {position} (file '{}')", path.display()),
            )?;
            Ok(SqlInput::file(path.display().to_string(), sql))
        }
    }
}

fn ensure_non_empty_sql(sql: &str, source: &str) -> Result<(), CliError> {
    if sql.trim().is_empty() {
        return Err(CliError::Input(if source == "stdin" {
            "SQL input is empty".to_string()
        } else {
            format!("{source}: SQL input is empty")
        }));
    }

    Ok(())
}

fn select_dialect(name: &str) -> Result<(String, Box<dyn Dialect>), CliError> {
    let normalized_name = name.to_ascii_lowercase();
    let dialect = dialect_from_str(&normalized_name).ok_or_else(|| {
        CliError::Input(format!(
            "unsupported dialect '{name}'; use a built-in dialect recognized by sqlparser"
        ))
    })?;

    Ok((normalized_name, dialect))
}

#[derive(Debug)]
enum CliError {
    Input(String),
    Protocol(ProtocolError),
    InputProtocol(InputAnalysisError),
}

impl CliError {
    fn exit_code(&self) -> ExitCode {
        match self {
            Self::Input(_) => ExitCode::from(2),
            Self::Protocol(ProtocolError::Parse(_)) => ExitCode::from(3),
            Self::Protocol(_) => ExitCode::from(4),
            Self::InputProtocol(error) => match error.error() {
                ProtocolError::Parse(_) => ExitCode::from(3),
                _ => ExitCode::from(4),
            },
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(message) => write!(formatter, "input error: {message}"),
            Self::Protocol(error) => match error {
                ProtocolError::Parse(_) => write!(formatter, "{error}"),
                _ => write!(formatter, "analysis error: {error}"),
            },
            Self::InputProtocol(error) => match error.error() {
                ProtocolError::Parse(_) => write!(formatter, "{error}"),
                _ => write!(formatter, "analysis error: {error}"),
            },
        }
    }
}
