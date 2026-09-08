# Switcheroo

Switch the logged-in account of your developer CLIs in one command, one click, or one tray menu.

```
switcheroo save claude-code            # remember the current login
switcheroo login claude-code           # log in as another account, remember it too
switcheroo use claude-code work@x.io   # switch; the previous login is re-saved first
switcheroo tray                        # menu-bar quick switcher + web UI
```

Switcheroo does not create profiles or config directories. It swaps the CLI's **live login**: the credential is captured into your OS credential store (macOS Keychain, Windows Credential Manager, Linux Secret Service) and restored on demand. CLIs that already keep several accounts (GitHub CLI, Netlify) are driven through their own switch commands and nothing is stored.

## Providers

| Provider | How | Notes |
|---|---|---|
| Claude Code | slot swap | keychain item `Claude Code-credentials` (macOS) or `~/.claude/.credentials.json`, plus `oauthAccount` in `~/.claude.json`; hot-reloads |
| Codex CLI | slot swap | `~/.codex/auth.json` (file credential mode only); restart running sessions |
| GitHub CLI | native | `gh auth status --json hosts` / `gh auth switch --user` |
| Vercel | slot swap | global `auth.json` token + `currentTeam`; `vercel whoami` names the account |
| Cloudflare Wrangler | slot swap | OAuth `default.toml`; encrypted keyring mode unsupported |
| npm (pnpm, yarn v1, bun) | slot swap | only the `//registry.npmjs.org/:_authToken` line of `~/.npmrc` |
| Fly.io | slot swap | `~/.fly/config.yml`, respecting flyctl's lock |
| Netlify | native | `netlify switch --email` |
| Gemini CLI, Turso, Expo/EAS, Railway | slot swap | **experimental** until confirmed on real logins |

`switcheroo providers` prints the full list with what each one touches. `switcheroo doctor` shows detection, vault health, and environment variables that would shadow a switch (`GH_TOKEN`, `CLOUDFLARE_API_TOKEN`, `VERCEL_TOKEN`, …).

## Install

Download the binary for your platform from the Releases page, put it on your PATH, and run `switcheroo`. Build from source with `cd web && npm ci && npm run build && cd .. && cargo build --release`.

Linux needs GTK 3 and an AppIndicator library at runtime for the tray (`libayatana-appindicator3`); without them `switcheroo serve` still provides the web UI.

## How a switch works

1. Take a lock so the CLI and the tray never interleave.
2. Read the live slot and re-save it under the account that is currently logged in (tokens rotate; the saved copy must be fresh).
3. Write the target account's credential into the slot.
4. Ask the CLI who is logged in. On a mismatch the previous login is restored.

Every terminal using that CLI is affected: the login is global by design.

## Security

- Secrets live only in the OS credential store (or, if you opt in with `--vault file`, a 0600 JSON file). `state.json` holds emails, labels and timestamps.
- On macOS the keychain is accessed through `/usr/bin/security`, the same tool Claude Code uses, so no access-control prompts appear and nothing breaks when the binary is updated.
- The web UI listens on loopback only. Mutations require a per-launch session token in a custom header, which blocks cross-site requests.
- Nothing leaves your machine; there is no telemetry.

## Web UI and tray

`switcheroo tray` shows a menu with one submenu per detected CLI: click an account to switch, "Save current login" to capture a login you made with the CLI itself, "Add account…" to open a terminal running the CLI's login. `switcheroo open` (or the tray's "Open Switcheroo…") opens the full web UI for renaming, removing, settings and diagnostics.

## Development

See `CLAUDE.md` for the architecture and the provider checklist.
