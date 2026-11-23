# Licences of the project's own assets

All assets in this repository were made by the project. It contains **no**
files, sounds, images or packs from Recursion Tracker, PlanetSide 2 or third
parties.

| Asset | Origin | Licence |
|---|---|---|
| Standard tones (`standard-tones`, sine tones) | generated in code (`crates/ps2-tracker-core/src/tones.rs`) | CC0-1.0 |
| Standard crosshairs (PNG) | generated in code (`crates/ps2-tracker-core/src/crosshair.rs`, `standard_crosshairs`) | CC0-1.0 |
| App icon (`assets/icon.svg`, `src-tauri/icons/*`) | drawn by the project, rendered from it | CC0-1.0 |
| Test fixtures audio (`tests/fixtures/audio/*.ogg`) | generated synthetically (`tools/gen_test_ogg.py`) | CC0-1.0 |
| Test fixtures Recursion (`tests/fixtures/recursion/`, binary variants in `tests/recursion_binary.rs`) | synthetic, same structure as the formats, own content | CC0-1.0 |
| ESS recording (`tests/fixtures/ess_real_2026-11-20.jsonl`) | recorded public Census events, character IDs pseudonymised | CC0-1.0 |
| Screenshots (`docs/screenshots/`) | browser preview of the app with demo data (fictional names) | CC0-1.0 |

The program code is licensed under `MIT OR Apache-2.0` (see `LICENSE-MIT`,
`LICENSE-APACHE`).

## Build-time file from NSIS

`packaging/windows/nsis-include/Win/RestartManager.nsh` is copied unchanged from
NSIS 3.12 (zlib/libpng license, <https://nsis.sourceforge.io/License>) because the
NSIS in the CI image is older; it only ends up inside the Windows installer script.
