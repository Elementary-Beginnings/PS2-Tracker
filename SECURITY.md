# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Report them
privately through GitHub's private vulnerability reporting instead:
[Security → Report a vulnerability](https://github.com/Elementary-Beginnings/ps2-tracker/security/advisories/new). Only the
maintainers can read the report.

Include the version (Settings → Advanced → About, or the first line of the log; see [Troubleshooting](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Troubleshooting-and-FAQ)), your
platform, steps to reproduce and – if a file triggers the problem – a minimal
synthetic sample (no third-party sounds or packs).

What to expect: this is a hobby project with a small team, so there is no
guaranteed service level. We aim to confirm a report within 7 days and to give
an assessment (accepted or not, planned fix) within 30 days. Confirmed issues
are fixed in a new release and published as a GitHub security advisory. Please
give us reasonable time to fix the issue before you disclose it (90 days by
default, or earlier once a fixed release is out); we will credit you in the
advisory and the changelog if you wish.

## Supported versions

| Version | Fixes |
|---|---|
| 1.x (newest release) | yes |
| `main` | yes, fixes land here first |
| 0.x and older 1.x releases | no – please update to the newest release |

## Scope

The app runs locally and talks only to the Daybreak Census API
(`census.daybreakgames.com`, `push.planetside2.com`) and – if enabled – serves an
OBS endpoint on `127.0.0.1`. It never touches the game process. What it reads and
writes, with the source locations, is documented in
[Permissions and Privacy](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Permissions-and-Privacy)
in the wiki (source: [`docs/wiki/Permissions-and-Privacy.md`](docs/wiki/Permissions-and-Privacy.md)).

In scope, for example: code execution or file access outside the app's folders
through a crafted voice pack, crosshair, session file or Census/ESS response;
leaking the Census service ID; the OBS endpoint serving more than the overlay;
Tauri capability or CSP bypasses.

Out of scope: the Census API itself, PlanetSide 2, BattlEye, and attacks that
need an attacker who already controls your user account.

## Updates

In-app updates are off by default and only for the AppImage and the Windows
installer. Update files must carry a valid signature of the project's key
(Tauri updater, minisign) and the signed version must match the announced one
(`requireSignedVersion`); the private key exists only as a GitHub Actions
secret of the `release` environment (see `docs/release.md`). The app only
contacts the update endpoint at startup when “Check for updates on start” is enabled, or
when the user clicks “Check now”.

## Files from other sources (voice packs, session files)

Voice packs and session files are untrusted input. The parsers are written for
that:

- **Read as data only.** Recursion's binary formats (SharpSerializer, .NET
  BinaryFormatter, protobuf) are parsed by our own readers into plain values; no
  types are instantiated and nothing is deserialised in the .NET sense. No
  `unsafe` code in the project (`unsafe_code = "deny"` in the workspace lints),
  with one exception: `src-tauri/src/win_console.rs` (Windows only) makes a few
  Win32 calls – attach to or open a console, read the cursor clip rectangle,
  open a link or folder with `ShellExecuteW` – each with a SAFETY note; a test fails if `unsafe` appears anywhere else.
  The parsers above contain no `unsafe`.
- **Limits** (`recursion_pak.rs`, `voicepack.rs`, `nrbf.rs`, `protowire.rs`,
  `sessionfile.rs`): definition file ≤ 256 MB, embedded file ≤ 32 MB, sound
  ≤ 20 MB, pack ≤ 200 MB, nesting depth ≤ 64, ≤ 1 000 000 protobuf fields,
  background image ≤ 2048×512 px / 4 MB; session file ≤ 25 MB (≤ 64 MB of
  embedded data in an .xlsx), ≤ 200 000 entries per list.
- **Paths:** files are copied into the app's data folder; `..`, absolute paths
  and symlinks are refused, absolute paths from the pack author's machine are
  ignored (`resolve_path`, `is_foreign_absolute`). Removing a pack only deletes
  its own folder (`removable_pack_dir`).
- **Session import** only reads the file chosen in the file dialog and only the
  tracker's own part of an .xlsx (no formulas or macros are evaluated); the
  content is validated before anything is stored.
- **Robustness tests:** the binary readers have mutation tests
  (`fuzz_no_panic` in `sharpser.rs`, `nrbf.rs`, `protowire.rs`: 100 000 truncated,
  bit-flipped and random inputs each must not panic); the session-file reader has
  tests for wrong formats, versions and values. There is no continuous
  coverage-guided fuzzing (cargo-fuzz) yet – contributions welcome.
