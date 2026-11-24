# Fuzzing

Fuzz targets for the parsers that read files and messages from outside:

| Target | Input |
|---|---|
| `recursion_pak` | binary Recursion pack definition (`.rtst_vpk`) |
| `nrbf` | .NET BinaryFormatter stream inside Recursion packs |
| `voicepack_manifest` | `voicepack.toml` and the path check of its file list |
| `session_file` | session file (JSON) of "Import session" |
| `ess_message` | message of the Census event stream |

Needs nightly Rust and [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz):

```bash
cargo install cargo-fuzz --locked
cargo +nightly fuzz run --fuzz-dir fuzz recursion_pak -- -max_total_time=60
```

The workflow `.github/workflows/fuzz.yml` runs every target for two minutes on
changes to the core crate and for ten minutes once a week. Crash inputs are
uploaded as artifact; add a regression test for each one in the core crate.
`fuzz/` is its own workspace (own `Cargo.lock`) and not part of the app build.
