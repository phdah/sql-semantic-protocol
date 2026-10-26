//! Source-independent evidence for SQL table sampling and its unknown membership law.

use super::LiteralValue;

/// SQL keyword used to request sample rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingSyntax {
    /// SAMPLE.
    Sample,
    /// TABLESAMPLE.
    TableSample,
}
impl SamplingSyntax {
    /// Stable protocol identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::TableSample => "tablesample",
        }
    }
}

/// The parsed sampling algorithm, not proof of its native implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingMethod {
    /// ROW.
    Row,
    /// BERNOULLI.
    Bernoulli,
    /// SYSTEM.
    System,
    /// BLOCK.
    Block,
}
impl SamplingMethod {
    /// Stable protocol identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Bernoulli => "bernoulli",
            Self::System => "system",
            Self::Block => "block",
        }
    }
}

/// Unit of a table sampling quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingUnit {
    /// Number of rows.
    Rows,
    /// Percentage.
    Percent,
}
impl SamplingUnit {
    /// Stable protocol identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Percent => "percent",
        }
    }
}

/// Quantity requested by a sample clause; unknown expressions have no literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplingQuantity {
    value: Option<LiteralValue>,
    unit: Option<SamplingUnit>,
}
impl SamplingQuantity {
    pub(crate) fn new(value: Option<LiteralValue>, unit: Option<SamplingUnit>) -> Self {
        Self { value, unit }
    }
    /// Typed constant, absent for unresolved quantity expressions.
    pub fn value(&self) -> Option<&LiteralValue> {
        self.value.as_ref()
    }
    /// Optional explicit unit, distinct from vendor-default semantics.
    pub fn unit(&self) -> Option<SamplingUnit> {
        self.unit
    }
}

/// Syntax of a seed declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingSeedSyntax {
    /// SEED.
    Seed,
    /// REPEATABLE.
    Repeatable,
}
impl SamplingSeedSyntax {
    /// Stable protocol identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::Repeatable => "repeatable",
        }
    }
}

/// Requested seed; a literal alone cannot prove native deterministic membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplingSeed {
    syntax: SamplingSeedSyntax,
    value: Option<LiteralValue>,
}
impl SamplingSeed {
    pub(crate) fn new(syntax: SamplingSeedSyntax, value: Option<LiteralValue>) -> Self {
        Self { syntax, value }
    }
    /// How the seed was declared.
    pub fn syntax(&self) -> SamplingSeedSyntax {
        self.syntax
    }
    /// Parsed literal, absent when unavailable or opaque.
    pub fn value(&self) -> Option<&LiteralValue> {
        self.value.as_ref()
    }
}

/// One sampled source occurrence in a query's FROM/JOIN sequence.
///
/// Sampling alters membership, so the protocol never certifies this as a closed
/// deterministic bag merely from an algorithm name or an explicit seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampling {
    source: String,
    source_instance: usize,
    syntax: SamplingSyntax,
    method: Option<SamplingMethod>,
    quantity: Option<SamplingQuantity>,
    seed: Option<SamplingSeed>,
    has_bucket_or_offset: bool,
}
impl TableSampling {
    pub(crate) fn new(
        source: String,
        source_instance: usize,
        syntax: SamplingSyntax,
        method: Option<SamplingMethod>,
        quantity: Option<SamplingQuantity>,
        seed: Option<SamplingSeed>,
        has_bucket_or_offset: bool,
    ) -> Self {
        Self {
            source,
            source_instance,
            syntax,
            method,
            quantity,
            seed,
            has_bucket_or_offset,
        }
    }
    /// Physical source name from the resolved table reference.
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Stable source occurrence index in FROM/JOIN order.
    pub fn source_instance(&self) -> usize {
        self.source_instance
    }
    /// SQL keyword variant.
    pub fn syntax(&self) -> SamplingSyntax {
        self.syntax
    }
    /// Optional explicit algorithm, otherwise vendor-dependent.
    pub fn method(&self) -> Option<SamplingMethod> {
        self.method
    }
    /// Optional requested quantity.
    pub fn quantity(&self) -> Option<&SamplingQuantity> {
        self.quantity.as_ref()
    }
    /// Optional seed metadata, not a membership certificate.
    pub fn seed(&self) -> Option<&SamplingSeed> {
        self.seed.as_ref()
    }
    /// Whether vendor bucket/offset options need additional engine evidence.
    pub fn has_bucket_or_offset(&self) -> bool {
        self.has_bucket_or_offset
    }
}
