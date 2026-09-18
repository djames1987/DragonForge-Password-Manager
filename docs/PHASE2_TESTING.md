# Phase 2 Local Verification

Run these checks after cloning or pulling Phase 2.

## Prerequisites

Install the current stable Rust toolchain with Rustfmt and Clippy.

Verify:

```powershell
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
```

If needed:

```powershell
rustup toolchain install stable
rustup component add rustfmt clippy
```

## Windows PowerShell

From the repository root:

```powershell
cd C:\path\to\DragonForge-Password-Manager

cargo clean
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

All four commands should finish successfully.

## Phase 2-only integration tests

To isolate the post-quantum layer:

```powershell
cargo test -p dragonforge-crypto --test post_quantum -- --nocapture
```

The test binary should report all tests passing.

## Phase 1 regression tests

Run the original foundation suite separately:

```powershell
cargo test -p dragonforge-crypto --test foundation -- --nocapture
```

This confirms Phase 2 did not break the symmetric/password foundation.

## Repeated randomized PQ test run

Because key generation and encapsulation use fresh randomness, it is useful during development to run the PQ suite repeatedly:

```powershell
1..20 | ForEach-Object {
    cargo test -q -p dragonforge-crypto --test post_quantum
    if ($LASTEXITCODE -ne 0) { throw "Phase 2 randomized test failed on run $_" }
}
```

This is not a substitute for formal test vectors or external review, but it can expose nondeterministic integration bugs.

## Release-mode verification

Also verify optimized builds:

```powershell
cargo test --workspace --all-features --release
```

Cryptographic code must not only work in debug mode.

## What success means

A clean local Phase 2 verification requires:

```text
rustfmt        PASS
clippy         PASS with -D warnings
foundation     PASS
post_quantum   PASS
release tests  PASS
```

GitHub Actions runs formatting, Clippy, and the full debug test suite on every push. The release-mode and repeated-randomized runs are recommended local checks before major milestones.

## What these tests do not prove

Passing tests does not prove cryptographic security. Before production use DragonForge still needs, at minimum:

- official/interoperability vector testing where applicable
- fuzzing of protocol parsers and serialized structures
- dependency/advisory scanning
- side-channel review
- protocol review
- independent cryptographic audit
- application-level security testing
