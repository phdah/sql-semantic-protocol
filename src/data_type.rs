//! Parser-independent datatype normalization.
//!
//! Dialect-specific SQL datatype syntax is parsed only at this boundary. Consumers depend on the
//! canonical protocol model rather than sqlparser AST types or vendor-specific type spellings.

use std::error::Error;
use std::fmt;

use sqlparser::ast::{
    ArrayElemTypeDef, BinaryLength, CharacterLength, ColumnDef, DataType as SqlDataType,
    EnumMember, ExactNumberInfo, StructField, UnionField,
};
use sqlparser::dialect::dialect_from_str;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

/// Parser-independent canonical SQL datatype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    /// Boolean value.
    Boolean,
    /// Signed integer value with an optional storage width in bits.
    SignedInteger {
        /// Canonical bit width when the dialect type defines one.
        bits: Option<u16>,
    },
    /// Unsigned integer value with an optional storage width in bits.
    UnsignedInteger {
        /// Canonical bit width when the dialect type defines one.
        bits: Option<u16>,
    },
    /// Exact decimal value.
    Decimal {
        /// Maximum decimal precision when declared.
        precision: Option<u64>,
        /// Decimal scale when declared.
        scale: Option<u64>,
    },
    /// Approximate floating-point value.
    FloatingPoint {
        /// Canonical storage width when the dialect type defines one.
        bits: Option<u16>,
    },
    /// Character string value.
    String {
        /// Maximum character length when declared and bounded.
        length: Option<u64>,
        /// Whether the SQL type is fixed-width.
        fixed: bool,
    },
    /// Binary string or blob value.
    Binary {
        /// Maximum byte length when declared and bounded.
        length: Option<u64>,
        /// Whether the SQL type is fixed-width.
        fixed: bool,
    },
    /// Calendar date.
    Date,
    /// Time-of-day value.
    Time {
        /// Fractional-second precision when declared.
        precision: Option<u64>,
    },
    /// Instant/date-time value.
    ///
    /// Dialect-specific timezone spellings normalize to this representation. Consumers may use a
    /// single UTC-based storage model while retaining fractional-second precision.
    Timestamp {
        /// Fractional-second precision when declared.
        precision: Option<u64>,
    },
    /// SQL interval value.
    Interval,
    /// UUID value.
    Uuid,
    /// JSON or semi-structured document value.
    ///
    /// JSON, JSONB, VARIANT, OBJECT, and SUPER normalize here because consumers should reason
    /// about their logical document value rather than their vendor storage encoding.
    Json,
    /// Bit string.
    BitString {
        /// Maximum bit length when declared.
        length: Option<u64>,
    },
    /// Homogeneous array/list value.
    Array {
        /// Element type when the dialect declaration carries one.
        element: Option<Box<DataType>>,
        /// Fixed array length when declared.
        length: Option<u64>,
    },
    /// Key/value map.
    Map {
        /// Key datatype.
        key: Box<DataType>,
        /// Value datatype.
        value: Box<DataType>,
    },
    /// Struct, tuple, row, or nested-record value.
    Struct {
        /// Fields in declaration order. Field names may be absent for tuple-like types.
        fields: Vec<DataTypeField>,
    },
    /// Tagged union value.
    Union {
        /// Union alternatives in declaration order.
        fields: Vec<DataTypeField>,
    },
    /// Enumerated value.
    Enum {
        /// Declared enum members in declaration order.
        values: Vec<EnumValue>,
    },
    /// SQL SET value.
    Set {
        /// Declared allowed string members.
        values: Vec<String>,
    },
    /// Table-valued type.
    Table {
        /// Optional named table type identity.
        name: Option<String>,
        /// Table columns.
        fields: Vec<DataTypeField>,
    },
    /// Geometric or geographic value.
    Geometry {
        /// Canonical geometry kind or vendor type name.
        kind: String,
    },
    /// PostgreSQL regclass/OID relation reference.
    Regclass,
    /// PostgreSQL text-search vector.
    TextSearchVector,
    /// PostgreSQL text-search query.
    TextSearchQuery,
    /// Nullable wrapper whose nullability is semantically significant.
    Nullable(Box<DataType>),
    /// Dialect accepts any value type.
    Any,
    /// SQLite-style column without a declared datatype.
    Unspecified,
    /// PostgreSQL trigger pseudo-type.
    Trigger,
    /// Vendor extension or user-defined type not reducible to a safer common representation.
    Custom {
        /// Canonical textual type identity.
        name: String,
        /// Vendor-specific type modifiers.
        modifiers: Vec<String>,
    },
}

impl DataType {
    /// Return a stable kind name for protocol serialization.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::SignedInteger { .. } => "signed_integer",
            Self::UnsignedInteger { .. } => "unsigned_integer",
            Self::Decimal { .. } => "decimal",
            Self::FloatingPoint { .. } => "floating_point",
            Self::String { .. } => "string",
            Self::Binary { .. } => "binary",
            Self::Date => "date",
            Self::Time { .. } => "time",
            Self::Timestamp { .. } => "timestamp",
            Self::Interval => "interval",
            Self::Uuid => "uuid",
            Self::Json => "json",
            Self::BitString { .. } => "bit_string",
            Self::Array { .. } => "array",
            Self::Map { .. } => "map",
            Self::Struct { .. } => "struct",
            Self::Union { .. } => "union",
            Self::Enum { .. } => "enum",
            Self::Set { .. } => "set",
            Self::Table { .. } => "table",
            Self::Geometry { .. } => "geometry",
            Self::Regclass => "regclass",
            Self::TextSearchVector => "text_search_vector",
            Self::TextSearchQuery => "text_search_query",
            Self::Nullable(_) => "nullable",
            Self::Any => "any",
            Self::Unspecified => "unspecified",
            Self::Trigger => "trigger",
            Self::Custom { .. } => "custom",
        }
    }
}

/// One field of a structured datatype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTypeField {
    name: Option<String>,
    data_type: DataType,
}

impl DataTypeField {
    /// Construct one canonical structured field.
    pub fn new(name: Option<String>, data_type: DataType) -> Self {
        Self { name, data_type }
    }

    /// Return the optional field name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the field datatype.
    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

/// One enumerated datatype member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumValue {
    name: String,
    value: Option<String>,
}

impl EnumValue {
    /// Return the declared member name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return an optional vendor-assigned member value.
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

/// Error returned when dialect-specific type syntax cannot be normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataTypeParseError {
    /// The dialect is not recognized by sqlparser.
    UnsupportedDialect {
        /// Caller-supplied dialect name.
        dialect: String,
    },
    /// The datatype syntax could not be parsed.
    Parse {
        /// Original type syntax.
        data_type: String,
        /// Parser explanation.
        message: String,
    },
    /// Tokens remain after one complete datatype.
    TrailingSyntax {
        /// Original type syntax.
        data_type: String,
    },
}

impl fmt::Display for DataTypeParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDialect { dialect } => {
                write!(formatter, "unsupported SQL dialect '{dialect}'")
            }
            Self::Parse { data_type, message } => {
                write!(
                    formatter,
                    "could not parse datatype '{data_type}': {message}"
                )
            }
            Self::TrailingSyntax { data_type } => {
                write!(
                    formatter,
                    "unexpected trailing syntax after datatype '{data_type}'"
                )
            }
        }
    }
}

impl Error for DataTypeParseError {}

/// Parse dialect-specific SQL datatype syntax into the canonical protocol datatype model.
pub fn parse_data_type(sql: &str, dialect_name: &str) -> Result<DataType, DataTypeParseError> {
    let normalized_dialect = dialect_name.to_ascii_lowercase();
    let dialect = dialect_from_str(&normalized_dialect).ok_or_else(|| {
        DataTypeParseError::UnsupportedDialect {
            dialect: dialect_name.to_owned(),
        }
    })?;
    let mut parser = Parser::new(dialect.as_ref())
        .try_with_sql(sql)
        .map_err(|error| DataTypeParseError::Parse {
            data_type: sql.to_owned(),
            message: error.to_string(),
        })?;
    let parsed = parser
        .parse_data_type()
        .map_err(|error| DataTypeParseError::Parse {
            data_type: sql.to_owned(),
            message: error.to_string(),
        })?;
    if !matches!(parser.peek_token().token, Token::EOF) {
        return Err(DataTypeParseError::TrailingSyntax {
            data_type: sql.to_owned(),
        });
    }

    Ok(normalize_data_type(&parsed, &normalized_dialect))
}

fn normalize_data_type(data_type: &SqlDataType, dialect_name: &str) -> DataType {
    match data_type {
        SqlDataType::Table(columns) => DataType::Table {
            name: None,
            fields: columns
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|column| column_field(column, dialect_name))
                .collect(),
        },
        SqlDataType::NamedTable { name, columns } => DataType::Table {
            name: Some(name.to_string()),
            fields: columns
                .iter()
                .map(|column| column_field(column, dialect_name))
                .collect(),
        },
        SqlDataType::Character(length) | SqlDataType::Char(length) => DataType::String {
            length: character_length(*length),
            fixed: true,
        },
        SqlDataType::CharacterVarying(length)
        | SqlDataType::CharVarying(length)
        | SqlDataType::Varchar(length)
        | SqlDataType::Nvarchar(length) => DataType::String {
            length: character_length(*length),
            fixed: false,
        },
        SqlDataType::Uuid => DataType::Uuid,
        SqlDataType::CharacterLargeObject(length)
        | SqlDataType::CharLargeObject(length)
        | SqlDataType::Clob(length) => DataType::String {
            length: *length,
            fixed: false,
        },
        SqlDataType::Binary(length) => DataType::Binary {
            length: *length,
            fixed: true,
        },
        SqlDataType::Varbinary(length) => DataType::Binary {
            length: binary_length(*length),
            fixed: false,
        },
        SqlDataType::Blob(length) => DataType::Binary {
            length: *length,
            fixed: false,
        },
        SqlDataType::TinyBlob => DataType::Binary {
            length: Some(255),
            fixed: false,
        },
        SqlDataType::MediumBlob => DataType::Binary {
            length: Some(16_777_215),
            fixed: false,
        },
        SqlDataType::LongBlob => DataType::Binary {
            length: Some(4_294_967_295),
            fixed: false,
        },
        SqlDataType::Bytes(length) => DataType::Binary {
            length: *length,
            fixed: false,
        },
        SqlDataType::Numeric(info)
        | SqlDataType::Decimal(info)
        | SqlDataType::BigNumeric(info)
        | SqlDataType::BigDecimal(info)
        | SqlDataType::Dec(info) => decimal_type(*info),
        SqlDataType::Float(_) => DataType::FloatingPoint { bits: None },
        SqlDataType::TinyInt(_) => signed_integer(8),
        SqlDataType::TinyIntUnsigned(_) | SqlDataType::UTinyInt | SqlDataType::UInt8 => {
            unsigned_integer(8)
        }
        SqlDataType::Int2(_) | SqlDataType::SmallInt(_) | SqlDataType::Int16 => signed_integer(16),
        SqlDataType::Int2Unsigned(_)
        | SqlDataType::SmallIntUnsigned(_)
        | SqlDataType::USmallInt
        | SqlDataType::UInt16 => unsigned_integer(16),
        SqlDataType::MediumInt(_) => signed_integer(24),
        SqlDataType::MediumIntUnsigned(_) => unsigned_integer(24),
        SqlDataType::Int(_)
        | SqlDataType::Int4(_)
        | SqlDataType::Int32
        | SqlDataType::Integer(_) => signed_integer(32),
        SqlDataType::IntUnsigned(_)
        | SqlDataType::Int4Unsigned(_)
        | SqlDataType::IntegerUnsigned(_)
        | SqlDataType::UInt32 => unsigned_integer(32),
        SqlDataType::Int8(_) => {
            if dialect_name == "clickhouse" {
                signed_integer(8)
            } else {
                signed_integer(64)
            }
        }
        SqlDataType::Int64
        | SqlDataType::BigInt(_)
        | SqlDataType::Signed
        | SqlDataType::SignedInteger => signed_integer(64),
        SqlDataType::Int8Unsigned(_) => {
            if dialect_name == "clickhouse" {
                unsigned_integer(8)
            } else {
                unsigned_integer(64)
            }
        }
        SqlDataType::BigIntUnsigned(_)
        | SqlDataType::UBigInt
        | SqlDataType::UInt64
        | SqlDataType::Unsigned
        | SqlDataType::UnsignedInteger => unsigned_integer(64),
        SqlDataType::Int128 | SqlDataType::HugeInt => signed_integer(128),
        SqlDataType::UInt128 | SqlDataType::UHugeInt => unsigned_integer(128),
        SqlDataType::Int256 => signed_integer(256),
        SqlDataType::UInt256 => unsigned_integer(256),
        SqlDataType::Float4 | SqlDataType::Float32 | SqlDataType::Real => {
            DataType::FloatingPoint { bits: Some(32) }
        }
        SqlDataType::Float8
        | SqlDataType::Float64
        | SqlDataType::Double(_)
        | SqlDataType::DoublePrecision => DataType::FloatingPoint { bits: Some(64) },
        SqlDataType::Bool | SqlDataType::Boolean => DataType::Boolean,
        SqlDataType::Date | SqlDataType::Date32 => DataType::Date,
        SqlDataType::Time(precision, _) => DataType::Time {
            precision: *precision,
        },
        SqlDataType::Datetime(precision) => DataType::Timestamp {
            precision: *precision,
        },
        SqlDataType::Datetime64(precision, _) => DataType::Timestamp {
            precision: Some(*precision),
        },
        SqlDataType::Timestamp(precision, _) => DataType::Timestamp {
            precision: *precision,
        },
        SqlDataType::TimestampNtz => DataType::Timestamp { precision: None },
        SqlDataType::Interval => DataType::Interval,
        SqlDataType::JSON | SqlDataType::JSONB => DataType::Json,
        SqlDataType::Regclass => DataType::Regclass,
        SqlDataType::Text
        | SqlDataType::TinyText
        | SqlDataType::MediumText
        | SqlDataType::LongText
        | SqlDataType::String(_) => DataType::String {
            length: string_length(data_type),
            fixed: false,
        },
        SqlDataType::FixedString(length) => DataType::String {
            length: Some(*length),
            fixed: true,
        },
        SqlDataType::Bytea => DataType::Binary {
            length: None,
            fixed: false,
        },
        SqlDataType::Bit(length)
        | SqlDataType::BitVarying(length)
        | SqlDataType::VarBit(length) => DataType::BitString { length: *length },
        SqlDataType::Custom(name, modifiers) => {
            normalize_custom_type(&name.to_string(), modifiers, dialect_name)
        }
        SqlDataType::Array(element) => match element {
            ArrayElemTypeDef::None => DataType::Array {
                element: None,
                length: None,
            },
            ArrayElemTypeDef::AngleBracket(element) | ArrayElemTypeDef::Parenthesis(element) => {
                DataType::Array {
                    element: Some(Box::new(normalize_data_type(element, dialect_name))),
                    length: None,
                }
            }
            ArrayElemTypeDef::SquareBracket(element, length) => DataType::Array {
                element: Some(Box::new(normalize_data_type(element, dialect_name))),
                length: *length,
            },
        },
        SqlDataType::Map(key, value) => DataType::Map {
            key: Box::new(normalize_data_type(key, dialect_name)),
            value: Box::new(normalize_data_type(value, dialect_name)),
        },
        SqlDataType::Tuple(fields) | SqlDataType::Struct(fields, _) => DataType::Struct {
            fields: fields
                .iter()
                .map(|field| struct_field(field, dialect_name))
                .collect(),
        },
        SqlDataType::Nested(columns) => DataType::Array {
            element: Some(Box::new(DataType::Struct {
                fields: columns
                    .iter()
                    .map(|column| column_field(column, dialect_name))
                    .collect(),
            })),
            length: None,
        },
        SqlDataType::Enum(values, _) => DataType::Enum {
            values: values
                .iter()
                .map(|value| match value {
                    EnumMember::Name(name) => EnumValue {
                        name: name.clone(),
                        value: None,
                    },
                    EnumMember::NamedValue(name, value) => EnumValue {
                        name: name.clone(),
                        value: Some(value.to_string()),
                    },
                })
                .collect(),
        },
        SqlDataType::Set(values) => DataType::Set {
            values: values.clone(),
        },
        SqlDataType::Union(fields) => DataType::Union {
            fields: fields
                .iter()
                .map(|field| union_field(field, dialect_name))
                .collect(),
        },
        SqlDataType::Nullable(inner) => {
            DataType::Nullable(Box::new(normalize_data_type(inner, dialect_name)))
        }
        SqlDataType::LowCardinality(inner) => normalize_data_type(inner, dialect_name),
        SqlDataType::Unspecified => DataType::Unspecified,
        SqlDataType::Trigger => DataType::Trigger,
        SqlDataType::AnyType => DataType::Any,
        SqlDataType::GeometricType(kind) => DataType::Geometry {
            kind: kind.to_string(),
        },
        SqlDataType::TsVector => DataType::TextSearchVector,
        SqlDataType::TsQuery => DataType::TextSearchQuery,
    }
}

fn signed_integer(bits: u16) -> DataType {
    DataType::SignedInteger { bits: Some(bits) }
}

fn unsigned_integer(bits: u16) -> DataType {
    DataType::UnsignedInteger { bits: Some(bits) }
}

fn decimal_type(info: ExactNumberInfo) -> DataType {
    let (precision, scale) = match info {
        ExactNumberInfo::None => (None, None),
        ExactNumberInfo::Precision(precision) => (Some(precision), None),
        ExactNumberInfo::PrecisionAndScale(precision, scale) => (Some(precision), Some(scale)),
    };
    DataType::Decimal { precision, scale }
}

fn character_length(length: Option<CharacterLength>) -> Option<u64> {
    match length {
        None | Some(CharacterLength::Max) => None,
        Some(CharacterLength::IntegerLength { length, .. }) => Some(length),
    }
}

fn binary_length(length: Option<BinaryLength>) -> Option<u64> {
    match length {
        None | Some(BinaryLength::Max) => None,
        Some(BinaryLength::IntegerLength { length }) => Some(length),
    }
}

fn string_length(data_type: &SqlDataType) -> Option<u64> {
    match data_type {
        SqlDataType::TinyText => Some(255),
        SqlDataType::MediumText => Some(16_777_215),
        SqlDataType::LongText => Some(4_294_967_295),
        SqlDataType::String(length) => *length,
        SqlDataType::Text => None,
        _ => None,
    }
}

fn struct_field(field: &StructField, dialect_name: &str) -> DataTypeField {
    DataTypeField::new(
        field.field_name.as_ref().map(ToString::to_string),
        normalize_data_type(&field.field_type, dialect_name),
    )
}

fn union_field(field: &UnionField, dialect_name: &str) -> DataTypeField {
    DataTypeField::new(
        Some(field.field_name.to_string()),
        normalize_data_type(&field.field_type, dialect_name),
    )
}

fn column_field(column: &ColumnDef, dialect_name: &str) -> DataTypeField {
    DataTypeField::new(
        Some(column.name.to_string()),
        normalize_data_type(&column.data_type, dialect_name),
    )
}

fn normalize_custom_type(name: &str, modifiers: &[String], _dialect_name: &str) -> DataType {
    let normalized = name
        .trim_matches(|character| {
            character == '"' || character == '`' || character == '[' || character == ']'
        })
        .to_ascii_uppercase();

    match normalized.as_str() {
        "VARIANT" | "OBJECT" | "SUPER" => DataType::Json,
        "GEOGRAPHY" | "GEOMETRY" => DataType::Geometry {
            kind: normalized.to_ascii_lowercase(),
        },
        "BYTE" => signed_integer(8),
        "SHORT" => signed_integer(16),
        "LONG" => signed_integer(64),
        "NUMBER" => decimal_from_modifiers(modifiers),
        "TIMESTAMP_LTZ" | "TIMESTAMP_TZ" | "TIMESTAMPTZ" => DataType::Timestamp {
            precision: numeric_modifier(modifiers.first()),
        },
        "VARCHAR2" | "NVARCHAR2" => DataType::String {
            length: numeric_modifier(modifiers.first()),
            fixed: false,
        },
        "VARBYTE" => DataType::Binary {
            length: numeric_modifier(modifiers.first()),
            fixed: false,
        },
        _ => DataType::Custom {
            name: name.to_owned(),
            modifiers: modifiers.to_vec(),
        },
    }
}

fn decimal_from_modifiers(modifiers: &[String]) -> DataType {
    let precision = numeric_modifier(modifiers.first());
    let scale = numeric_modifier(modifiers.get(1));
    DataType::Decimal { precision, scale }
}

fn numeric_modifier(value: Option<&String>) -> Option<u64> {
    value.and_then(|value| value.trim().parse::<u64>().ok())
}
