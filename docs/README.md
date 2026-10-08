# Documentation

Start with the [project README](../README.md) for an overview and first commands.

| Document | Purpose |
| --- | --- |
| [CLI and workflows](cli.md) | Input options, SQL files/directories, analysis manifests, dialects, and output example |
| [Semantic model](semantics.md) | How the analyzer handles types, domains, constraints, and transformations |
| [Adapters](adapters.md) | dbt, ODCS, OpenLineage, and metadata authority |
| [Protocol contract](protocol.md) | Active JSON model and exact representation guarantees |
| [Protocol JSON Schema](../schema/protocol.schema.json) | Machine-readable active contract |
| [Analysis manifest v1](analysis-manifest-v1.md) | Declarative analysis input format (distinct from protocol version) |
| [Versioning and releases](releasing.md) | Shared application/protocol version and release automation |

## Historical contracts

[Protocol v0.1](protocol-v0.md) and [protocol v0.2](protocol-v0.2.md) with their matching versioned schemas are retained as historical references and are **not** emitted by the current analyzer. For current behavior, use the [unversioned contract](protocol.md).
