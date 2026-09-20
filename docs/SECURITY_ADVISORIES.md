# Security Advisory Disposition

DragonForge treats dependency advisories as security findings that must be
remediated or explicitly dispositioned. An advisory is not ignored solely to
make CI or local verification pass.

## RUSTSEC-2023-0071 — `rsa` Marvin Attack

**Status:** Accepted as non-applicable to the current DragonForge dependency
graph, with an automated applicability check.

**Affected lockfile package:** `rsa 0.9.10`

**Why it appears:** `sqlx-mysql 0.8.6` declares a normal dependency on
`rsa 0.9`. Cargo includes that optional SQLx backend and its dependencies in
the generated lockfile even though DragonForge does not enable the MySQL
backend.

DragonForge's sync server declares SQLx as:

```toml
sqlx = { version = "0.8", optional = true, default-features = false, features = ["runtime-tokio-rustls", "postgres", "uuid", "migrate", "macros"] }
```

The project therefore enables PostgreSQL and does not enable SQLx's MySQL
feature.

### Evidence

The following checks were performed on Windows:

```powershell
cargo tree -i rsa --workspace --all-features
cargo tree -i rsa@0.9.10 --workspace --all-features --target all
```

Both produced no active inverse dependency tree for `rsa`. A direct inspection
of `Cargo.lock` identified `sqlx-mysql 0.8.6` as the package that references
`rsa`.

### Automated guard

The Phase 11 verification runner executes an all-target/all-feature inverse
dependency check before `cargo audit`. If `rsa 0.9.10` ever becomes active
in the workspace graph, verification fails before the RustSec exception can
mask it.

The project-level `.cargo/audit.toml` contains the narrow exception:

```toml
[advisories]
ignore = ["RUSTSEC-2023-0071"]
```

This exception must be removed if SQLx MySQL support is ever enabled, if RSA is
introduced directly, or when the dependency chain is changed such that the
advisory becomes reachable.

## Informational advisories

`cargo audit` may also report informational warnings such as unmaintained or
unsound transitive crates. These remain visible in audit output. They are not
silently converted into vulnerability exceptions and should be reviewed during
dependency maintenance.

## Policy

For every ignored RustSec advisory DragonForge should retain:

1. the advisory identifier;
2. the dependency path that introduced it;
3. the reason it is not applicable;
4. a machine-checkable guard when practical;
5. the conditions that require the exception to be removed.
