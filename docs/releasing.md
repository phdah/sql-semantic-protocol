# Versioning and releases

## Versioning

SQL Semantic Protocol uses one version for the application and the protocol. The Cargo package version, emitted `protocol_version`, active protocol contract, Git tag, and GitHub release are the same release identity.

SemVer compatibility is defined primarily by the public protocol contract. A breaking protocol change requires a major version bump. Backward-compatible protocol or application features use a minor bump, while compatible fixes and internal application changes use a patch bump. Non-protocol implementation changes therefore do not require a breaking release, but every release still advances the shared application/protocol version.

Release Please manages the shared application/protocol version from Conventional Commits. After the 1.0.0 bootstrap, breaking changes use major releases, backward-compatible features use minor releases, and compatible fixes or internal changes use patch releases.

The release workflow opens or updates a release PR from `main`. The generated release branch is checked with `cargo publish --dry-run --locked`. Merging the release PR creates the matching `vX.Y.Z` tag and GitHub release, then publishes the same package version to crates.io. Publishing uses the repository secret `CARGO_REGISTRY_TOKEN`. The 1.0.0 bootstrap override was retired after the initial stable release.

After publication, the binary can be installed with:

```sh
cargo install sql-semantic-protocol
```

To add the library dependency at the latest published release, run `cargo add sql-semantic-protocol`.

For published versions and migration notes, see [GitHub releases](https://github.com/phdah/sql-semantic-protocol/releases) and the [changelog](../CHANGELOG.md).
