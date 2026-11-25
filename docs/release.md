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

**Alternative: release by button.** After step 2 without the tag (or with a
version commit made otherwise) and `main` pushed: *Actions → Release → Run
workflow* on `main` with input `version` = the version in `Cargo.toml`. The
first job checks the version files, creates the annotated tag `vX.Y.Z` on
exactly that commit through the API (`GITHUB_TOKEN`, `contents: write`) and
fails if the tag exists; all builds then check out that commit. A tag created
with `GITHUB_TOKEN` starts no new workflow run, so the same run builds and
publishes it. Prerequisites: the environment `release` allows `main` besides
`v*` tags, and a tag ruleset that restricts creating `v*` tags lists
“GitHub Actions” (or the Actions integration) as bypass actor.

**Test the Flatpak before a tag:** *Actions → Flatpak → Run workflow* on
`main` (or any pull request touching `packaging/flatpak/`, `src-tauri/`,
`Cargo.lock`): the same build as in the release, without environment and
secrets (no built-in Census service ID); the bundle is the artifact
`flatpak`.

### Release workflow

| Job | Runner | Result |
|---|---|---|
| `verify` | ubuntu-24.04 | tag = version (`check_versions.sh`), CHANGELOG section exists; with *Run workflow* also creates the tag |
| `linux` | ubuntu-22.04 (deliberately: glibc 2.35 for older systems) | `ps2-tracker-x86_64-linux.tar.gz`, `ps2-tracker-x86_64.AppImage` (+ `.sig`) |
| `arch` | `archlinux:base-devel` container (digest pinned) | `ps2-tracker-x86_64.pkg.tar.zst` from the tarball, AUR `ps2-tracker-bin.PKGBUILD` / `.SRCINFO` |
| `windows` | windows-latest | `ps2-tracker-x64-setup.exe` (NSIS, built natively, + `.sig`) |
| `flatpak` | `flatpak.yml` (reusable): sources on ubuntu-24.04, build in Flathub's `gnome-51` container | `ps2-tracker.flatpak` (offline build from source; a failure does not stop the release) |
| `release` | ubuntu-24.04 | `SHA256SUMS`, `latest.json` (only with signatures), provenance attestations, GitHub release with the CHANGELOG section as text |

Hardening: `permissions: {}` at the top and minimal permissions per job (only
`release` writes: `contents`, `id-token`, `attestations`; `verify` has
`contents: write` for creating the tag with *Run workflow*), every action pinned
to a commit SHA (Dependabot updates them, with a cooldown), checkout without
persisted credentials, no caches in release builds, no `pull_request_target`;
checked with `zizmor`. Downloads can be verified with
`gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`.

### Repository settings (maintainer)

- **Environment `release`** (*Settings → Environments*): deployment rules
  “tags `v*`” and, for *Run workflow*, branch `main`. Only the build and release jobs use it, so pull request
  workflows never see its secrets. Every value is optional; the release
  succeeds without them:
  - secret `CENSUS_DEFAULT_SERVICE_ID` – built-in Census service ID (without
    it the builds use `example`). The log only says “set” / “not set”;
    `linux` and `windows` check that the value is in the binary without
    printing it.
  - secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` –
    updater signatures (without them there are no `.sig` files and no
    `latest.json`).
- Actions variable `SYNC_DISCORD_CLIENT_ID` – the ELME Discord application
  (public value, 17–20 digits): online sync sign-in and the default
  application of Discord Rich Presence. The app never shows or logs it.
- *Settings → Security*: private vulnerability reporting on (see
  `SECURITY.md`), Dependabot alerts and security updates on.
- Ruleset for `main` (CI required, no force pushes) and for tags `v*` (no
  deletion, no update; if it also restricts creation, add “GitHub Actions” as
  bypass actor for *Run workflow*).

### When a job of the release workflow fails

- **Network error or flaky runner:** *Re-run failed jobs*. The `release` job
  only runs when `verify`, `linux`, `arch` and `windows` succeeded.
- **Error in the workflow:** fix it on `main` and release a new PATCH version;
  the tag runs the workflow of the tagged commit. Never move a tag.
- **Error in the code:** a new PATCH version.
- If the release already exists (workflow run a second time), `gh release
  create` fails; edit the release on GitHub instead.

## Wiki

User guides are in `docs/wiki/` (file name = wiki page name, links between
pages as `[Text](Page-Name)`, images as absolute
`raw.githubusercontent.com/.../main/docs/screenshots/...` URLs).
`.github/workflows/wiki.yml` mirrors the folder into the GitHub wiki on every
push to `main` that changes it (or manually via *Run workflow*), with the
job's `GITHUB_TOKEN` (`contents: write`); pages edited in the wiki itself are
overwritten. `tools/check_links.py` checks the wiki links in CI.

Once (maintainer): enable the wiki (*Settings → Features → Wikis*, editing
restricted to collaborators) and create any first page, so that GitHub creates
the wiki repository; the first workflow run then replaces it.

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

`.github/workflows/flatpak.yml` builds the bundle with Flathub's tooling: a
first job on ubuntu-24.04 builds the frontend (`dist/`) and writes
`cargo-sources.json` (`tools/flatpak_sources.sh`: flatpak-cargo-generator,
pinned commit and SHA-256); the second job runs
`flatpak/flatpak-github-actions/flatpak-builder` (pinned SHA) in the container
`ghcr.io/flathub-infra/flatpak-github-actions:gnome-51` (pinned digest,
`--privileged` for bubblewrap) with the GNOME 51 SDK, current
`flatpak-builder`, `appstream-compose` and `appstreamcli`. `/var/lib/flatpak`
is an anonymous volume: the image's preinstalled SDK is copied there, so
updating it to Flathub's newest build works (on the container's overlayfs
the update fails with “Invalid cross-device link”). The build is offline; no
build cache. The optional built-in IDs go in only through a
temporary `.cargo/config.toml` (`tools/flatpak_cargo_env.sh`), never through
the manifest (flatpak-builder stores it in the bundle). Update the container
digest together with the runtime version in the manifest. Flathub would need
its own manifest repository there (a separate application).
