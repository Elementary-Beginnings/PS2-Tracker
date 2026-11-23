# Contributing

Thanks for your interest! The documentation is in English; the README is also
kept in German (`README.de.md`, update both). Issues, pull requests and code
comments may be in English or German; commit messages in this repository are
German. UI texts are English (`src/lib/i18n/en.ts`).

## Ground rules

- **No game intervention.** The tracker only uses the public Daybreak Census API.
  No memory reading, injection, hooks into the game process or anything that could
  conflict with BattlEye.
- **No third-party content.** Do not add sounds, images or packs from Recursion
  Tracker, PlanetSide 2 or other sources. Test data must be synthetic
  (see `tools/gen_test_ogg.py` and `crates/ps2-tracker-core/tests/`).
- **No secrets.** Never commit a Census service ID, tokens or personal data. A
  gitleaks pre-commit hook is included: `git config core.hooksPath .githooks`.
- **No real players.** Fixtures, screenshots and examples use synthetic names
  and IDs (`DemoSoldier…`, `5428000000000000042`). Real ESS payloads must be
  pseudonymised before they go into `tests/fixtures/`.
- **No hit detection.** The Census event stream has no damage or hit events for
  infantry; features must not pretend otherwise (see the hit feedback section in
  the README).

## Development

Requirements: Rust (stable), Node.js and pnpm, the Tauri 2 Linux dependencies
(webkit2gtk-4.1, libayatana-appindicator, librsvg, alsa-lib).

```bash
pnpm install
pnpm tauri dev                  # run the app
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm check && pnpm test
```

Windows can be checked from Linux with `cargo xwin clippy --target
x86_64-pc-windows-msvc -p ps2-tracker`.

New Tauri commands go into the `invoke_handler` (`src-tauri/src/main.rs`),
`COMMANDS` (`src-tauri/build.rs`) and the capability of the window that uses them
(`src-tauri/capabilities/*.json`); the test `every_command_is_declared_and_allowed`
fails otherwise. Screenshots for the README come from the browser preview with
demo data (`pnpm build`, `vite preview`, URL parameters in `src/lib/demo.ts`).
For end-to-end tests without the game, `tools/fake_ess.py` serves synthetic
events on `ws://127.0.0.1:8765`; a debug build connects to it with
`PS2_TRACKER_ESS_URL=ws://127.0.0.1:8765` (release builds ignore the variable).
`tools/e2e/move_layer.sh` tests the move layer under Xvfb + xfwm4 against a
fullscreen window that holds the pointer (`tools/e2e/grabwin.c`), with Ctrl+F2
taken by another client like KWin does (`tools/e2e/grabkey.c`): every way out of
the move mode is checked. `tools/e2e/squad_live.sh` adds a squad member at
runtime (debug-only CLI `--debug-squad-add <id> <name>`) and checks the live ESS
subscription and the OBS/overlay snapshot. Both need a debug build and
`pnpm build`, and only stop processes on their own display.
After changing dependencies, regenerate `THIRD-PARTY-NOTICES.md` with
`python3 -I tools/third_party_notices.py`. Releases, the update signature and
the AUR package are described in `docs/release.md`.

## Pull requests

- One topic per pull request, with tests for new logic (the core crate
  `ps2-tracker-core` is pure and easy to test).
- The CI workflow must be green (format, clippy, tests, audit, licenses,
  links; Linux and Windows).
- Describe user-visible changes in `CHANGELOG.md`.

## Security

Security problems: please follow [`SECURITY.md`](SECURITY.md) (private
vulnerability report, not a public issue).

## License

By contributing you agree that your contribution is licensed under
`MIT OR Apache-2.0`, like the rest of the project, without additional terms.

---
