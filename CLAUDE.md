# Switcheroo

One Rust binary that switches the logged-in account of developer CLIs (Claude Code, Codex, Vercel, Wrangler, npm, Fly, GitHub CLI, Netlify, …) from the terminal, a loopback web UI, or the system tray.

## Shape

- `src/core/` — `Core` (the only object the surfaces talk to), data model, state/settings files, event bus, cross-process lock, execution context (`Cx`), switch orchestration (`switch.rs`).
- `src/providers/` — the provider contract (`Provider` trait in `mod.rs`), reusable slot adapters (`slots/`), identity resolvers (`identity.rs`), the generic `SlotProvider`, and one module per CLI. Registry = `all()`.
- `src/vault/` — where secrets go: macOS keychain via `/usr/bin/security`, Windows Credential Manager / Linux Secret Service via keyring-core, or an opt-in 0600 file.
- `src/cli/`, `src/server/`, `src/tray/` — the three surfaces. `web/` is the Vite + React + shadcn UI embedded by `rust-embed` from `web/dist`.

## Invariants

- The `Provider` trait is the only way providers are used. CLI, API, tray and web never special-case a provider id.
- Providers never print, log, or return secrets. Secret bytes travel only as `SecretBlob` (zeroized, redacted in Debug). No API route returns a blob.
- Every switch re-captures the live login first (`Core::use_account`), then activates, then verifies, and rolls back on a failed verify. Do not bypass `Core` for anything that touches a credential slot.
- Composite files (`~/.claude.json`, `~/.expo/state.json`, `~/.npmrc`) are patched by key/line through a slot adapter, never replaced wholesale. All writes go through `core::fsutil::write_atomic`.
- State writes are atomic and publish on the bus; tray and web render from state only.
- macOS keychain access shells out to `/usr/bin/security` on purpose (ACL parity with the CLIs); do not switch it to Security.framework.
- Web pages call `/api` only through `web/src/api/client.ts`; platform facts come from `window.__SWITCHEROO__` (`web/src/boot.ts`).
- rclone-ui (`../rclone-ui`) is a reference, not a source of code.

## Adding a provider

1. `src/providers/<id>.rs`: a `ProviderMeta` (id, name, tier, binaries, process names, `env_shadow`, restart hint, notes, login command), the slots (`FileSlot`, `JsonKeysSlot`, `LinesSlot`, `KeychainItemSlot`, `LockedSlot`), an `IdentityResolver`, optional verify command and extra preflight. Most providers are a `SlotProvider` value; CLIs with their own registry implement `Provider` directly (see `github_cli.rs`, `netlify.rs`).
2. Tests in the same file: identity parsing from a redacted fixture written into a temp home (`Cx::test`), a capture → activate round trip that proves unrelated keys/lines survive, preflight warnings.
3. One line in `providers::all()`. Nothing else changes.

## Dev loop

```
cargo test
cd web && npm install && npm run dev            # Vite on :5173, proxies /api to :7777
cargo run -- serve --dev --bind 127.0.0.1:7777  # fixed session token "dev"
cd web && npm run build && cargo build          # embed the UI; then `cargo run -- serve --open` / `tray`
```
