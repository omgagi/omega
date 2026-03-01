# Release Process

How to cut a new Omega release.

## Prerequisites

- All tests pass: `cd backend && cargo test --workspace`
- No lint warnings: `cargo clippy --workspace -- -D warnings`
- Formatting clean: `cargo fmt --check`

## Steps

### 1. Bump version

Edit `backend/Cargo.toml` — the single source of truth:

```toml
[workspace.package]
version = "X.Y.Z"
```

All 6 crates inherit via `version.workspace = true`. No other files to update.

### 2. Update CHANGELOG.md

Add a new section at the top of `CHANGELOG.md` with the release date and curated changes grouped by: Added, Changed, Fixed.

### 3. Commit

```bash
git add -A
git commit -m "chore(release): prepare vX.Y.Z"
```

### 4. Tag

```bash
git tag vX.Y.Z
```

### 5. Push

```bash
git push origin main --tags
```

### 6. CI takes over

The `release.yml` workflow:

1. **Validates** the tag matches `Cargo.toml` version (fails fast on mismatch)
2. **Builds** release binaries for 3 targets:
   - `x86_64-unknown-linux-gnu` (Linux x86_64)
   - `aarch64-apple-darwin` (macOS ARM / Apple Silicon)
   - `x86_64-apple-darwin` (macOS Intel)
3. **Generates** `SHA256SUMS.txt` for all artifacts
4. **Creates** a GitHub Release with tarballs, checksums, and auto-generated notes

### 7. Verify

- Check the [Releases page](https://github.com/omgagi/omega/releases) for the new release
- Verify all 3 binaries and checksums are attached
- Test the install script: `curl -fsSL https://raw.githubusercontent.com/omgagi/omega/main/install.sh | bash`

## Version scheme

Follows [Semantic Versioning](https://semver.org/):
- **Major** (1.0.0): Breaking changes to config format, channel protocol, or provider API
- **Minor** (0.X.0): New features, providers, channels, or significant improvements
- **Patch** (0.0.X): Bug fixes, security patches, documentation

## Hotfix releases

For urgent fixes on the latest release:

```bash
git checkout vX.Y.Z
git checkout -b hotfix/description
# ... fix ...
git commit -m "fix(scope): description"
# Bump patch version in Cargo.toml
git tag vX.Y.(Z+1)
git push origin hotfix/description --tags
```

Then create a PR to merge the fix back into `main`.
