# Security

Short overview. The binding policy is
[`SECURITY.md`](https://github.com/Elementary-Beginnings/ps2-tracker/blob/main/SECURITY.md)
in the repository.

## Reporting a vulnerability

Please do **not** open a public issue. Report privately through
[Security → Report a vulnerability](https://github.com/Elementary-Beginnings/ps2-tracker/security/advisories/new)
(GitHub private vulnerability reporting). Supported is the newest 1.x release;
we aim to confirm a report within 7 days.

## What the app does and does not do

- Talks only to the Daybreak Census API (`census.daybreakgames.com`,
  `push.planetside2.com`), after opt-in to the update endpoint on GitHub, and
  locally to Discord and (if enabled) an OBS endpoint on `127.0.0.1`.
- Never touches the game process; no telemetry.
- Voice packs and session files are untrusted input: parsed by own readers as
  data only, with size and depth limits, no paths outside the app's folders,
  mutation tests against panics. No `unsafe` code except three documented Win32
  calls.
- Tauri capabilities minimal per window, CSP without remote content.

Everything with source code locations:
[Permissions and Privacy](Permissions-and-Privacy).

## Verifying downloads

- `SHA256SUMS` in every release: `sha256sum -c SHA256SUMS --ignore-missing`.
- Build provenance attestations:
  `gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`.
- In-app updates are signed (minisign key ID `9D64F76BBF11AD27`), see
  [Auto-Update](Auto-Update).
- The Windows installer is not code-signed (SmartScreen warns).

## Supply chain

- CI on GitHub-hosted runners: `cargo audit`, `cargo deny` (advisories,
  licences, sources), `pnpm audit`, gitleaks.
- CodeQL (Rust, JavaScript/TypeScript, workflows), OpenSSF Scorecard,
  dependency review on pull requests, Dependabot with a 7-day cooldown.
- Workflows with minimal permissions per job, actions pinned to commit SHAs,
  checked with zizmor; release secrets only in a protected environment.

## Accepted advisories

No known vulnerability affects the app. These advisories (warnings, no CVE)
concern transitive dependencies without a fix reachable from the current
stable parent crates; they are accepted on purpose (checked 2026-11-23, listed
in [`.cargo/audit.toml`](https://github.com/Elementary-Beginnings/ps2-tracker/blob/main/.cargo/audit.toml))
and re-checked with every Tauri update. OpenSSF Scorecard still counts them.

| Advisory | Crate | Kind | Why it is there |
|---|---|---|---|
| RUSTSEC-2024-0429 | glib 0.18.5 | unsound | Tauri 2's GTK 3 stack (gtk-rs 0.18); the affected `VariantStrIter` is not used; fixed only in gtk-rs ≥ 0.20 |
| RUSTSEC-2024-0370 | proc-macro-error 1.0.4 | unmaintained | build-time macro of glib-macros 0.18; not in the binary |
| RUSTSEC-2026-0370 | zvariant 5.15.1 | unmaintained | zbus 5 of tauri-plugin-single-instance 2.5.5 (newest stable); merged into zbus 6 |
| RUSTSEC-2026-0371 | zbus_names 4.3.5 | unmaintained | as above |
| RUSTSEC-2026-0372 | zvariant_derive 5.15.1 | unmaintained | as above |
| RUSTSEC-2026-0543 | chrono 0.4.45 | unmaintained | only in `Cargo.lock` via kdotool (Linux part of the Windows-only active-win-pos-rs); never compiled |
