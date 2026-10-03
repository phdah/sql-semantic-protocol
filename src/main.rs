use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use sql_semantic_protocol::{
    analyze_inputs, select_targets, to_bundle_json, to_openlineage_json, Error as ProtocolError,
    InputAnalysisError, OpenLineageExportError, SqlInput, TargetSelectionError,
};
use sqlparser::dialect::{dialect_from_str, Dialect};

const USAGE: &str = "Usage: sql-semantic-protocol [--dialect <name>] [--format <protocol|openlineage>] [--namespace <name>] [--event-time <RFC3339>] [--target <relation>]... [--sql <SQL>]... [--file <path>]... [--dir <path>]... [SQL ...]\n\nRepeat --sql, --file, --dir, and --target as needed. Directories are searched recursively for .sql files; other files are ignored.\nLegacy positional SQL remains one input. If no input is supplied, SQL is read from stdin.\nThe dialect defaults to generic and may be any built-in dialect recognized by sqlparser.\nTargets are selected only after the full bundle has been analyzed; each target keeps its required in-bundle ancestors.\nThe output format defaults to protocol. OpenLineage output requires --namespace; --event-time is optional and defaults to the current UTC time.\nUse -- to pass positional SQL that starts with a dash.";

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

            let bundle = analyze_inputs(&inputs, &dialect_name, dialect.as_ref())
                .map_err(CliError::InputProtocol)?;
            let bundle =
                select_targets(&bundle, &options.targets).map_err(CliError::TargetSelection)?;
            let output = match options.format {
                OutputFormat::Protocol => to_bundle_json(&bundle),
                OutputFormat::OpenLineage => {
                    let namespace = options.namespace.as_deref().ok_or_else(|| {
                        CliError::Input(
                            "--namespace is required with --format openlineage".to_string(),
                        )
                    })?;
                    let event_time = match options.event_time {
                        Some(event_time) => event_time,
                        None => current_event_time()?,
                    };
                    to_openlineage_json(&bundle, namespace, &event_time)
                        .map_err(CliError::OpenLineageExport)?
                }
            };
            println!("{output}");

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
    format: OutputFormat,
    namespace: Option<String>,
    event_time: Option<String>,
    targets: Vec<String>,
    inputs: Vec<InputArgument>,
    positional_sql: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Protocol,
    OpenLineage,
}

#[derive(Debug)]
enum InputArgument {
    Inline(String),
    File(PathBuf),
    Directory(PathBuf),
}

fn parse_args(mut arguments: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let mut dialect = "generic".to_string();
    let mut format = OutputFormat::Protocol;
    let mut namespace = None;
    let mut event_time = None;
    let mut targets = Vec::new();
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
            "--format" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --format".to_string()))?;
                format = match value.as_str() {
                    "protocol" => OutputFormat::Protocol,
                    "openlineage" => OutputFormat::OpenLineage,
                    _ => {
                        return Err(CliError::Input(format!(
                            "unsupported output format '{value}'; expected protocol or openlineage"
                        )));
                    }
                };
            }
            "--namespace" => {
                namespace =
                    Some(arguments.next().ok_or_else(|| {
                        CliError::Input("missing value for --namespace".to_string())
                    })?);
            }
            "--event-time" => {
                event_time = Some(arguments.next().ok_or_else(|| {
                    CliError::Input("missing value for --event-time".to_string())
                })?);
            }
            "--target" => {
                let target = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --target".to_string()))?;
                if target.trim().is_empty() {
                    return Err(CliError::Input("--target cannot be empty".to_string()));
                }
                targets.push(target);
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
            "--dir" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| CliError::Input("missing value for --dir".to_string()))?;
                inputs.push(InputArgument::Directory(PathBuf::from(path)));
            }
            _ if argument.starts_with('-') => {
                return Err(CliError::Input(format!("unknown option: {argument}")));
            }
            _ => positional_sql.push(argument),
        }
    }

    if !inputs.is_empty() && !positional_sql.is_empty() {
        return Err(CliError::Input(
            "cannot combine positional SQL with --sql, --file, or --dir; use --sql for explicit inputs"
                .to_string(),
        ));
    }

    match format {
        OutputFormat::Protocol => {
            if namespace.is_some() {
                return Err(CliError::Input(
                    "--namespace requires --format openlineage".to_string(),
                ));
            }
            if event_time.is_some() {
                return Err(CliError::Input(
                    "--event-time requires --format openlineage".to_string(),
                ));
            }
        }
        OutputFormat::OpenLineage => {
            if namespace.as_deref().map(str::trim).unwrap_or("").is_empty() {
                return Err(CliError::Input(
                    "--namespace is required with --format openlineage".to_string(),
                ));
            }
            if event_time
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(CliError::Input("--event-time cannot be empty".to_string()));
            }
        }
    }

    Ok(Command::Analyze(Options {
        dialect,
        format,
        namespace,
        event_time,
        targets,
        inputs,
        positional_sql,
    }))
}

fn read_inputs(options: &Options) -> Result<Vec<SqlInput>, CliError> {
    if !options.inputs.is_empty() {
        let mut inputs = Vec::new();

        for input in &options.inputs {
            match input {
                InputArgument::Inline(sql) => {
                    let position = inputs.len() + 1;
                    ensure_non_empty_sql(sql, &format!("input {position} (--sql)"))?;
                    inputs.push(SqlInput::inline(sql.clone()));
                }
                InputArgument::File(path) => {
                    let position = inputs.len() + 1;
                    inputs.push(read_file_input(path, position)?);
                }
                InputArgument::Directory(path) => {
                    for discovered_path in discover_sql_files(path)? {
                        let position = inputs.len() + 1;
                        inputs.push(read_file_input(&discovered_path, position)?);
                    }
                }
            }
        }

        if inputs.is_empty() {
            return Err(CliError::Input(
                "no SQL inputs found; supplied directories contained no .sql files".to_string(),
            ));
        }

        return Ok(inputs);
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

fn read_file_input(path: &Path, position: usize) -> Result<SqlInput, CliError> {
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

fn discover_sql_files(root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let metadata = fs::metadata(root).map_err(|error| {
        CliError::Input(format!(
            "directory '{}': failed to inspect: {error}",
            root.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(CliError::Input(format!(
            "directory '{}': path is not a directory",
            root.display()
        )));
    }

    let mut paths = Vec::new();
    collect_sql_files(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_sql_files(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), CliError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        CliError::Input(format!(
            "directory '{}': failed to read: {error}",
            directory.display()
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            CliError::Input(format!(
                "directory '{}': failed to read entry: {error}",
                directory.display()
            ))
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            CliError::Input(format!(
                "path '{}': failed to inspect: {error}",
                path.display()
            ))
        })?;

        if file_type.is_dir() {
            collect_sql_files(&path, paths)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("sql"))
        {
            paths.push(path);
        }
    }

    Ok(())
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

fn current_event_time() -> Result<String, CliError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| CliError::Input(format!("system clock is before Unix epoch: {error}")))?;
    format_utc_event_time(elapsed.as_secs())
}

fn format_utc_event_time(seconds_since_epoch: u64) -> Result<String, CliError> {
    let days = i64::try_from(seconds_since_epoch / 86_400)
        .map_err(|_| CliError::Input("current time is outside the supported range".to_string()))?;
    let seconds_of_day = seconds_since_epoch % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);

    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_date_from_unix_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted_days = days_since_epoch + 719_468;
    let era = shifted_days / 146_097;
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    (year, month, day)
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
    InputProtocol(InputAnalysisError),
    TargetSelection(TargetSelectionError),
    OpenLineageExport(OpenLineageExportError),
}

impl CliError {
    fn exit_code(&self) -> ExitCode {
        match self {
            Self::Input(_) => ExitCode::from(2),
            Self::InputProtocol(error) => match error.error() {
                ProtocolError::Parse(_) => ExitCode::from(3),
                _ => ExitCode::from(4),
            },
            Self::TargetSelection(_) => ExitCode::from(2),
            Self::OpenLineageExport(_) => ExitCode::from(4),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(message) => write!(formatter, "input error: {message}"),
            Self::InputProtocol(error) => match error.error() {
                ProtocolError::Parse(_) => write!(formatter, "{error}"),
                _ => write!(formatter, "analysis error: {error}"),
            },
            Self::TargetSelection(error) => write!(formatter, "input error: {error}"),
            Self::OpenLineageExport(error) => {
                write!(formatter, "OpenLineage export error: {error}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openlineage_output_options() {
        let command = parse_args(
            [
                "--format",
                "openlineage",
                "--namespace",
                "postgresql://warehouse",
                "--event-time",
                "2026-10-02T07:00:00Z",
                "--sql",
                "SELECT id FROM raw.orders",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect("OpenLineage CLI options should parse");

        match command {
            Command::Analyze(options) => {
                assert_eq!(options.format, OutputFormat::OpenLineage);
                assert_eq!(options.namespace.as_deref(), Some("postgresql://warehouse"));
                assert_eq!(options.event_time.as_deref(), Some("2026-10-02T07:00:00Z"));
            }
            Command::Help => panic!("expected analyze command"),
        }
    }

    #[test]
    fn openlineage_output_requires_namespace() {
        let error = parse_args(
            [
                "--format",
                "openlineage",
                "--sql",
                "SELECT id FROM raw.orders",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect_err("OpenLineage output without a namespace should fail");

        assert_eq!(
            error.to_string(),
            "input error: --namespace is required with --format openlineage"
        );
    }

    #[test]
    fn protocol_output_rejects_openlineage_only_options() {
        let error = parse_args(
            ["--namespace", "postgresql://warehouse", "--sql", "SELECT 1"]
                .into_iter()
                .map(str::to_string),
        )
        .expect_err("namespace should not be silently ignored for protocol output");

        assert_eq!(
            error.to_string(),
            "input error: --namespace requires --format openlineage"
        );
    }

    #[test]
    fn formats_current_time_as_utc_rfc3339() {
        assert_eq!(
            format_utc_event_time(0).expect("Unix epoch should format"),
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(
            format_utc_event_time(1_790_924_400).expect("representative timestamp should format"),
            "2026-10-02T07:00:00Z"
        );
    }
}
