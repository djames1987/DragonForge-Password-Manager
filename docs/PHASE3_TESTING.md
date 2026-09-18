# Phase 3 Local Verification

Run these checks after pulling Phase 3.

## Windows PowerShell

From the repository root:

```powershell
cd C:\DragonForge-Password-Manager

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Then isolate the new Phase 3 suite:

```powershell
cargo test -p dragonforge-vault --test local_vault -- --nocapture
```

Run the earlier regression suites separately:

```powershell
cargo test -p dragonforge-crypto --test foundation -- --nocapture
cargo test -p dragonforge-crypto --test post_quantum -- --nocapture
```

Finally verify optimized behavior:

```powershell
cargo test --workspace --all-features --release
```

## Expected Phase 3 behaviors

The `local_vault` integration suite verifies that:

- a vault can be created and persisted;
- the returned Account Secret is required to reopen it;
- a wrong master password cannot unlock it;
- a wrong Account Secret cannot unlock it;
- logins and secure notes survive lock/unlock;
- plaintext titles, usernames, passwords, URLs, notes, and tags do not appear in the vault file;
- local search works after decryption;
- updates and deletes persist;
- changing the master password rewraps the VMK without losing items;
- encrypted backups can be exported/imported;
- modified item ciphertext is detected;
- generated passwords satisfy enabled character classes.

## Optional manual inspection

After the tests, you can inspect a temporary/sample `.dfvault` file with a text editor.

You should see JSON structure, UUIDs, KDF parameters, nonces, and integer ciphertext arrays.

You should **not** see the plaintext item title, username, password, URL, notes, or tags that were inserted before saving.

## Release criteria

A clean Phase 3 local check is:

```text
rustfmt                 PASS
clippy -D warnings      PASS
foundation tests        PASS
post-quantum tests      PASS
local vault tests       PASS
release-mode tests      PASS
```

Passing these tests does not constitute a security audit. Production use still requires fuzzing, advisory/dependency scanning, side-channel review, protocol review, penetration testing, and an independent cryptographic/application audit.
