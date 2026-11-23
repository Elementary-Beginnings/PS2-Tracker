# Release, update signature and AUR

What happens in a release and what only the maintainer can do. The repository
is <https://github.com/Elementary-Beginnings/ps2-tracker>; CI and releases run
there on GitHub Actions with GitHub-hosted runners.

## Principle

Every new version is a release with downloads: the tag `vX.Y.Z` starts
`.github/workflows/release.yml`, which builds all packages and creates the
GitHub release.

## Version numbers (SemVer)

`MAJOR.MINOR.PATCH` (since 1.0.0):

- **PATCH** (`1.0.0` → `1.0.1`): bug fixes only, no new settings, session data
  version and SQLite schema unchanged.
- **MINOR** (`1.0.x` → `1.1.0`): new features, new settings, a new session data
  version or a new schema (old data stays readable).
- **MAJOR** (`1.x` → `2.0.0`): changes after which old data, configurations or
  packs can no longer be read, or a removed feature.

Changes to CI or documentation alone need no new version.

One source of truth: `version` in `[workspace.package]` of `Cargo.toml`.
`tools/check_versions.sh` (run in CI and in the release workflow) fails when
`package.json`, `src-tauri/tauri.conf.json` (also the Windows installer
version), both PKGBUILDs, the newest `<release>` of the Flatpak metainfo or
`Cargo.lock` differ, and in the release workflow when the tag is not
`v<version>`. The app shows the version in the sidebar and under *Settings →
Advanced → About* together with the commit and the build date
(`src-tauri/build.rs`: `git rev-parse`, otherwise “unknown”; a click copies
the line for bug reports); the log start and `--version` print the same.

## Steps

1. Everything for the version is in `CHANGELOG.md` under `## [Unreleased]`,
   `main` is clean and CI on `main` is green.
2. `tools/release.sh patch|minor|major` (or an explicit `X.Y.Z`) sets the
   version in `Cargo.toml` (`[workspace.package]`, plus `Cargo.lock`),
   `package.json`, `src-tauri/tauri.conf.json`, `packaging/arch/PKGBUILD`,
   `packaging/aur/PKGBUILD` (pkgrel 1) and the Flatpak metainfo (`<release>`),
   turns `## [Unreleased]` into the section `## [X.Y.Z] – date`, commits
   (“Release X.Y.Z”) and creates the annotated tag `vX.Y.Z`; the tag message is
   the short form of the CHANGELOG section. It runs `tools/check_versions.sh`
   and finally asks before pushing `main` and the tag to `origin` (GitHub);
   run non-interactively it only prints the commands.
3. Check (`git show vX.Y.Z`), then push. The tag starts the release workflow.

### Release workflow

| Job | Runner | Result |
|---|---|---|
| `verify` | ubuntu-22.04 | tag = version (`check_versions.sh`), CHANGELOG section exists |
| `linux` | ubuntu-22.04 | `ps2-tracker-x86_64-linux.tar.gz`, `ps2-tracker-x86_64.AppImage` (+ `.sig`) |
| `arch` | `archlinux:base-devel` container (digest pinned) | `ps2-tracker-x86_64.pkg.tar.zst` from the tarball, AUR `ps2-tracker-bin.PKGBUILD` / `.SRCINFO` |
| `windows` | windows-latest | `ps2-tracker-x64-setup.exe` (NSIS, built natively, + `.sig`) |
| `flatpak` | ubuntu-22.04 | `ps2-tracker.flatpak` (offline build from source; a failure does not stop the release) |
| `release` | ubuntu-22.04 | `SHA256SUMS`, `latest.json` (only with signatures), provenance attestations, GitHub release with the CHANGELOG section as text |

Hardening: `permissions: {}` at the top and minimal permissions per job (only
`release` writes: `contents`, `id-token`, `attestations`), every action pinned
to a commit SHA (Dependabot updates them, with a cooldown), checkout without
persisted credentials, no caches in release builds, no `pull_request_target`;
checked with `zizmor`. Downloads can be verified with
`gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`.

### Repository settings (maintainer)

- **Environment `release`** (*Settings → Environments*): deployment rule “tags
  `v*` only”. Only the build and release jobs use it, so pull request
  workflows never see its secrets. Every value is optional; the release
  succeeds without them:
  - secret `CENSUS_DEFAULT_SERVICE_ID` – built-in Census service ID (without
    it the builds use `example`). The log only says “set” / “not set”;
    `linux` and `windows` check that the value is in the binary without
    printing it.
  - secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` –
    updater signatures (without them there are no `.sig` files and no
    `latest.json`).
- Actions variable `DISCORD_APP_ID` – built-in Discord application (public
  value, 17–20 digits).
- *Settings → Security*: private vulnerability reporting on (see
  `SECURITY.md`), Dependabot alerts and security updates on.
- Ruleset for `main` (CI required, no force pushes) and for tags `v*` (no
  deletion, no update).

### When a job of the release workflow fails

- **Network error or flaky runner:** *Re-run failed jobs*. The `release` job
  only runs when `verify`, `linux`, `arch` and `windows` succeeded.
- **Error in the workflow:** fix it on `main` and release a new PATCH version;
  the tag runs the workflow of the tagged commit. Never move a tag.
- **Error in the code:** a new PATCH version.
- If the release already exists (workflow run a second time), `gh release
  create` fails; edit the release on GitHub instead.

## Update signature (Tauri updater)

The in-app updater only installs files whose signature matches the public key
in the app. **Only the maintainer** creates the key, on their own machine; the
private part never goes into the repository.

```bash
pnpm tauri signer generate -w ~/.tauri/ps2-tracker.key
# asks for a password; writes ps2-tracker.key and ps2-tracker.key.pub
```

1. **Store the private key** as secrets of the environment `release`:
   `TAURI_SIGNING_PRIVATE_KEY` (content of `ps2-tracker.key`) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
2. **Enter the public key:** the base64 of `ps2-tracker.key.pub` goes into
   `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`. Without it the
   app shows under *Settings → Updates & Discord*: “In-app updates are not set
   up yet” and never asks for updates.
3. With the secret set, `linux` and `windows` create the `.sig` files
   (`src-tauri/tauri.updater.conf.json`) and `release` writes `latest.json`
   (`tools/updater_manifest.py`). The app's endpoint is
   `https://github.com/Elementary-Beginnings/ps2-tracker/releases/latest/download/latest.json`.

Current key: minisign ID `9D64F76BBF11AD27` (public key in `tauri.conf.json`
since the first GitHub release; the private half and its password are secrets
of the environment `release`). Update checks stay opt-in in the app
(`updates.check_on_start` is off by default; installing needs a click).

Keep the key safe: if it is lost, existing installations can no longer accept
updates (a new public key only arrives with a manually installed version).

## AUR package `ps2-tracker-bin`

The package installs the prebuilt binary from the release tarball. Every
release carries the ready `ps2-tracker-bin.PKGBUILD` and
`ps2-tracker-bin.SRCINFO` (version and checksum filled in). Not published in
the AUR yet.

Once (maintainer): AUR account, SSH key in the AUR profile, `~/.ssh/config`:

```
Host aur.archlinux.org
  User aur
  IdentityFile ~/.ssh/aur
```

Per release:

```bash
git clone ssh://aur@aur.archlinux.org/ps2-tracker-bin.git   # empty the first time
cd ps2-tracker-bin
gh release download vX.Y.Z --repo Elementary-Beginnings/ps2-tracker -p 'ps2-tracker-bin.*'
mv ps2-tracker-bin.PKGBUILD PKGBUILD && mv ps2-tracker-bin.SRCINFO .SRCINFO
makepkg -si                       # downloads, checks sha256, installs
git add PKGBUILD .SRCINFO
git commit -m "ps2-tracker-bin X.Y.Z"
git push
```

Locally: `bash tools/aur_update.sh X.Y.Z ps2-tracker-x86_64-linux.tar.gz PKGBUILD`
and `makepkg --printsrcinfo > .SRCINFO`. The workflows never push to the AUR.

## Flatpak

`flatpak-builder` needs bubblewrap with user namespaces, which the
GitHub-hosted `ubuntu-22.04` runner allows. The build is offline: the frontend
(`dist/`) is built first, the crates come from `cargo-sources.json`
(flatpak-cargo-generator, pinned commit and SHA-256). Flathub would need its
own manifest repository there (a separate application).
