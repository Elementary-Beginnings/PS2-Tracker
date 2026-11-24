# Changelog

Format after [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versions
after [SemVer](https://semver.org/).

## [Unreleased]

### Fixed

- Linux: the app no longer crashes at start when the tray library (libayatana-appindicator3) is missing, e.g. in the Flatpak. On GNOME without a tray extension, and wherever no tray icon can be shown, closing the main window quits the app instead of leaving it running unseen, and "Start in the tray" opens the window.
- Windows: "Open folder" opens the folder through the Windows shell instead of starting explorer.exe with the path as its command line.
- Voice packs: a pack whose voicepack.toml was written on Windows (backslashes in file paths, or file names that differ in upper/lower case from the files) now loads on Linux too.

## [1.2.1] – 2026-11-24

### Fixed

- Windows: signing in to online sync (and the privacy link) opened a File Explorer window instead of the browser. Links now open with the default browser.

## [1.2.0] – 2026-11-24

### Added

- **Online sync** for ELME members (opt-in, off by default; *Settings →
  Online sync*): finished sessions of your own characters are uploaded to the
  ELME stats server (stats.elme.games) in the background. Sign-in with Discord
  (authorization code with PKCE, loopback redirect on 127.0.0.1); the refresh
  token is kept only in the system keyring (Secret Service on Linux, Credential
  Manager on Windows) and the sign-in is refused when there is none. Only
  characters the server lists as linked to your account are uploaded, squad
  sessions never. Persistent upload queue with retries (30 s up to 1 h,
  `Retry-After` on HTTP 429); *History* shows per session whether it is not
  synced, queued, synced or failed (with the reason), with *Upload now* /
  *Retry upload* in the detail view and *Upload existing sessions…* in the
  settings. *Sign out* and *Delete my online data…* in the settings. Without
  switching it on and signing in, the app makes no request to the stats server.
  Changing the API address while a sign-in or token refresh is running never
  hands a token of one server to the other. See the wiki page *Online Sync*.
- Online sync: more robust handling of server address changes. The built-in
  Discord client ID is used only with the default address
  (stats.elme.games); another address needs its own client ID. Changing the
  address asks for confirmation, says that the sign-in goes to that address,
  and signs out at the previous one. A notice is shown while another address
  is set.
- Online sync: answers of the stats server are read up to a size limit (1 MiB
  for JSON, the first 4 KiB of an error text); a larger answer counts as an
  error.
- Online sign-in: the local sign-in page serves connections side by side with
  a shorter read timeout, so the browser's return is answered right away.
- Online sync: when the stats server has not confirmed the character link yet,
  the settings say so and the sign-in is kept; the tracker asks again after
  15 minutes. A full storage quota marks the session as failed with the
  reason (not retried by itself). The settings show the storage quota next to
  the used space when the server sends it.
- Online sign-in: the local sign-in ports are 47831, 47832 and 47833, apart
  from the default port of the OBS endpoint (47823).
- Flatpak: permission `--talk-name=org.freedesktop.secrets` for the keyring of
  the online sync.
- Official builds can embed the Discord client ID of the stats server
  (repository variable `SYNC_DISCORD_CLIENT_ID`).

### Changed

- Database schema 4: new table `sync_queue` (online sync); older databases are
  migrated at start, no data is removed.
- OBS endpoint: stricter request validation. Only requests addressed to
  `127.0.0.1`, `localhost` or `[::1]` with the configured port are answered,
  others get HTTP 421 (OBS browser sources with `http://127.0.0.1:<port>/`
  work as before).
- `ps2-tracker-core`: new default feature `app` for everything the desktop
  app needs. With `default-features = false` the crate only reads, checks and
  summarises session files and writes the Excel export, without the Census,
  ESS, SQLite, keyring and voice pack dependencies.
- Exports (session file, chart image, voice pack ZIP) check the file chosen in
  the save dialog before writing: the extension of the format, an existing
  folder, a plain file and not inside the app's own folders.

### Security

- Voice pack import: a `.rtst_vpk` imported directly (XML or binary) takes
  the sound files it names from the folder it lies in (e.g. Downloads). Only
  audio files (recognised by their content, at most 20 MB, checked before
  reading) are taken over; other files a pack definition names are skipped
  and never copied into the library.
- CSV export: name cells starting with `=`, `+`, `-`, `@`, tab or CR get a
  leading `'`, so a spreadsheet program does not run them as formulas (names
  can come from an imported session file).

## [1.1.1] – 2026-11-23

### Added

- Session buttons for the resume window on the live page: while a session
  waits for a relogin, a banner shows *Logged out – waiting for relogin* with
  the time left, *Resume now* (continue at once, e.g. when the relogin was not
  detected) and *End session* (ends at the time of the logout and saves).
- *Resume last session*: without a running session, the most recently ended
  session of the selected character can be opened again up to 6 hours after
  it ended. It continues in the same history entry (no copy); the time in
  between counts as a pause, not as playtime. Older sessions cannot be resumed
  (button disabled, the reason in the tooltip).
- New commands `resume_session`, `last_session` and `reopen_session`, allowed
  only for the main window.

### Changed

- Smoother pop-up animation: pack image and text slide and scale in together
  (about 220 ms, ease-out) and fade out in 300 ms, animating only opacity and
  transform; a pop-up that follows a visible one (kill chain) swaps the text
  with a short pulse instead of fading out and in. With *prefers-reduced-motion*
  only a fade is used. The display duration setting still applies.

### Fixed

- The duration in the *History* list leaves out the time logged out within
  the resume window, like the live page and the session detail.
- The achievement/voice pack pop-up no longer stays on screen as a still
  image (KDE Plasma/KWin over XWayland, NVIDIA): the pop-up window is now
  hidden after the fade-out and only mapped again for the next pop-up; the
  focus watcher no longer shows the empty window on every focus change to the
  game. The move mode still shows the pop-up for positioning.
- *End session* during the resume window and quitting the app while logged
  out end the session at the logout instead of counting the time logged out
  as playtime.
- The duration on the live page, in the overlay and in the session detail no
  longer counts paused time.

### Security

- Session import checks the pause times added with the resume window in 1.1.0:
  a file with negative pause seconds, more pause than session length, or a
  pause in a finished session is rejected instead of skewing playtime and
  rates.

### Documentation

- README, session file schema (`paused_at`, `paused_secs`) and *Permissions and
  Privacy* describe the resume window and the new import checks.

## [1.1.0] – 2026-11-23

### Added

- Resume a session after a logout or a game crash: after a logout the session
  waits for a relogin (default 15 minutes, *Settings → Sessions*, 0 = off) and
  then goes on instead of starting a new one. The time logged out does not
  count as playtime; if the window runs out, the session ends at the logout.

### Fixed

- Adding a squad member could hang for minutes during play, often right after
  the first one: every kill started its own Census name lookup, and with
  several tracked characters these queued up faster than the rate limit
  allows. Names are now collected for a second and looked up in one batch, and
  lookups you wait for (adding a character or squad member, outfit search,
  saving settings) use their own queue with a 15 s timeout. The add button
  shows "Searching …" meanwhile.
- The pack achievement pop-up could stay on screen after its display time
  (default 4 s): it only faded to opacity 0, and with a transparent window
  some compositors kept the last frame. The pack image and text are now
  removed after the fade, so the window is transparent again.
- The log no longer fills with "mapping packet failed (buffer underrun)"
  warnings from the Ogg decoder: some voice pack files end with an incomplete
  Ogg page. The sound plays in full; the decoder now only logs errors.

## [1.0.4] – 2026-11-23

### Security

- `cookie` 0.18.2 → 0.18.3 (RUSTSEC-2026-0594: private cookie nonces could
  repeat in processes that fork). Indirect dependency; lockfile only.

## [1.0.3] – 2026-11-23

### Fixed

- Flatpak bundle: built with Flathub's tooling (`flatpak-github-actions` in
  the `gnome-51` container) instead of Ubuntu 22.04's `flatpak-builder`, which
  lacks `appstream-compose` for the GNOME 51 runtime; 1.0.2 shipped without
  the bundle.

### Changed

- Release workflow: can also be started by button (*Run workflow* with the
  version); it then creates the tag itself and builds that commit. The
  Flatpak build is a workflow of its own that can be run before a tag
  (manually and on pull requests touching the packaging). Helper jobs run on
  Ubuntu 24.04; the Linux binaries stay on 22.04 for the older glibc.

## [1.0.2] – 2026-11-23

### Fixed

- Release workflow: the Flatpak MetaInfo is validated with the current
  `appstreamcli` from `org.flatpak.Builder`; the older one in Ubuntu 22.04
  rejected the valid `<developer>` tag and the `vcs-browser` URL, so 1.0.1
  shipped without the Flatpak bundle.

## [1.0.1] – 2026-11-23

### Added

- **Built-in Discord application in official builds**: Rich Presence works
  without an own application ID (`PS2_DEFAULT_DISCORD_APP_ID` at compile time).
  An own valid ID in the settings still wins.
- **In-app updates are set up**: the public key of the update signature
  (minisign ID `9D64F76BBF11AD27`) is in the app, and GitHub releases carry
  signed AppImage/Windows setup files plus `latest.json`. Checking stays
  opt-in (off by default), installing needs a click.

### Fixed

- Release workflow: the Flatpak job installs a current `tomlkit` (pinned by
  hash) for flatpak-cargo-generator, and release files are collected in
  `out/` instead of the frontend build folder `dist/`.

### Changed

- **The project moved to GitHub**
  (<https://github.com/Elementary-Beginnings/ps2-tracker>): links in the app,
  the README files, the PKGBUILDs and the Flatpak metainfo point there, and so
  does the updater endpoint (`releases/latest/download/latest.json`).
- Releases are built by GitHub Actions (Linux, Arch, Windows natively,
  Flatpak) and carry build provenance attestations
  (`gh attestation verify`). Security reports go through GitHub private
  vulnerability reporting; `SECURITY.md` names the supported versions (1.x)
  and the response times.
- Code scanning with CodeQL (Rust, JavaScript/TypeScript, workflows), OpenSSF
  Scorecard and dependency review on pull requests (vulnerabilities from
  moderate up, no GPL/AGPL/LGPL dependencies).
- **Guides moved to the GitHub wiki** (source in `docs/wiki/`, published by a
  workflow); the README files are short overviews with links. *Settings →
  Advanced → About* shows the wiki and issue addresses; the Flatpak metainfo
  links the wiki and the FAQ.
- Issues use forms: bug report, feature request, voice pack problem, question.
- Six advisories of transitive dependencies (gtk-rs 0.18 via Tauri, zbus 5,
  chrono via a never-compiled Linux dependency) are documented as accepted
  (`.cargo/audit.toml`, wiki page Security).

### Fixed

- The Discord settings no longer claim that continent and server are never
  shown; they appear once the running session knows them.

## [1.0.0] – 2026-11-22

### Added

- **Built-in Census service ID in official builds**: the app works without an
  own service ID. Order: `PS2_SERVICE_ID` > own ID from the settings > built-in
  default (compile time, `PS2_DEFAULT_SERVICE_ID`) > `example`. The built-in
  value is readable from the binary, but never written to `config.toml`, never
  logged and never shown; Settings shows only where the ID in use comes from.
  An empty field means "default"; a stored `example` from older versions is
  cleared. Details in `docs/permissions.md`.
- **Official class icons** in the kill feed, Squad, Squad top 3, the overlay
  and the class tables: loaded at runtime from Census
  (`/files/ps2/images/static/<id>.png`, IDs 204/62/65/201/59/207), checked
  (PNG, max. 64 KB and 256×256) and cached; no Daybreak graphic in the
  repository. Without network, and in the OBS source, the own symbols stay.
- **Version shown in the app**: “v1.0.0” at the bottom of the sidebar and
  version, commit and build date under *Settings → Advanced → About*; a click
  copies the line for bug reports. Log start and `--version` print the same.

### Changed

- Documentation in English (README, docs/, CHANGELOG, CONTRIBUTING,
  SECURITY); the German README stays as `README.de.md`. `docs/berechtigungen.md`
  is now `docs/permissions.md`.
- Releases: `tools/release.sh patch|minor|major` counts the version up from
  `Cargo.toml` and sets it everywhere; `tools/check_versions.sh` (CI job
  `check`) fails when the version files disagree or a tag does not match.

### Fixed

- **Binary Recursion packs: sounds dropped on import.** The sound list of an
  event was only read directly on the entry; Deadpool v2 keeps it in
  `dynamicSounds.sounds`, so every event with several sounds lost them
  (Deadpool v2: 63 of 342 embedded files taken over, PER_KILL and DEATH_INSULT
  without files, normal kills silent; now 206, PER_KILL 3, DEATH_INSULT 2,
  HEAD_SHOT 3). The list is now read there too, and as a fallback every
  embedded sound named anywhere inside an event is taken over, plus numbered
  variants of a sound of
  the same event (`… 002.ogg`) that no event names. The import dialog lists
  events whose sounds could not be taken over and the number of embedded
  sounds without an event. Packs need to be imported again.
- **No sound on normal kills with packs without `PER_KILL`**: the pack card in
  *Sounds* warns “No sound for every kill (PER_KILL)” and offers a key of the
  pack for “Use this pack key for every kill” (proposal: the key with the most
  sounds).
- Every kill/death now leaves a `voice:` line in the log, also when no rule
  produced a sound event.

## [0.2.0] – 2026-11-22

### Added

- **Move layer**: Ctrl+F2 puts a dimmed layer over the game's monitor; overlay
  and pop-up can be dragged there directly over the game (snapping to edges,
  centre and the other object, Alt = free, optional grid, coordinate display),
  keyboard as before, Enter/Ctrl+F2 saves, Esc discards, bar with
  Save/Cancel/Reset/“Move to monitor…”. If the game holds the cursor, a hint
  appears (“Esc in the game”); the keyboard always works. The real windows stay
  OSD and click-through meanwhile.
- **Finish move mode even when Ctrl+F2 is taken**: own key “Finish move mode”
  (default Ctrl+Esc, on Windows Ctrl+Shift+F2 – Ctrl+Esc is reserved for the
  Start menu there –, fallback Ctrl+Alt+F2; existing Windows configurations
  with Ctrl+Esc are switched on start and logged; changeable, with conflict
  detection), valid during the whole move session, also while the game grabs
  the cursor; also a tray entry “Finish move mode”, a button on the Overlay page
  and a banner in the main window, CLI `--finish-move`, and on its own after
  5 minutes without input (saves, adjustable/can be switched off). Hotkeys that
  cannot be registered are logged (`hotkey registered … ok|failed`) and shown
  in the settings, on the Overlay page and on the move layer; log
  `move layer finished via=…`.
- **Classes**: class icon (own SVG icons) before the name in the kill feed of
  app, overlay and OBS – victim on a kill, attacker on a death –, optionally the
  own class (“Show my class in the kill feed”); current class per member in
  “Squad top 3” and in the squad table (column *Class*); Live page with
  kills/deaths by enemy class and own class usage; Analytics chart “Enemy
  classes”; Excel export with columns *Enemy class*/*My class* and sheet
  *Classes*. Mapping from the Census collection `loadout` (24 loadouts,
  built in, no new query).
- **More bundled crosshairs** (generated by the project, CC0): `cross` in three
  sizes, `cross-dashed` and `dot` in green/magenta/red, range ladder
  `range-ladder` (orange) for 1440p and 4K; tests for dimensions, symmetry and
  centre. Large previews in the library are limited.
- The pop-up can be on a different monitor than the overlay (`popup_monitor`).
- **Overlay block “Squad top 3”**: the three best characters of the running
  sessions by K/D, kills or KPM (minimum number of kills for K/D and KPM), with
  faction badge, own row marked; also in the OBS endpoint. Fixed size: always
  three rows, no size change during the session.
- **Server and continent** in the squad table, in the sidebar and optionally in
  Discord Rich Presence; names through Census (`world`/`zone`, only occupied
  IDs, in the name cache).
- **Binary Recursion packs** (`.rtst_vpk`): SharpSerializer (Deadpool, C&C V6,
  Camikaze78), .NET BinaryFormatter and protobuf (ArnoldVoicePackBETA); all
  simple SharpSerializer types (among them `guid`), entries
  `fileName`/`dataType`/`data`, background image through `backgroundImage`,
  can be switched off with `backgroundImageDisable`; ZIPs over 20 MB.
- **Background image of the pop-up** (`AchievementBG.png`) from the pack.
- **One event system for sounds**: pack keys serve the base events (kill →
  `DOUBLE_KILL` … > `HEAD_SHOT` > `PER_KILL`; own death → `DEATH_INSULT`;
  session start → `SESSION_STARTED`); the standard tones are only the last
  resort. Option “Use this pack key for every kill” for packs without
  `PER_KILL`. Log per event with the checked key chain.
- **Remove pack** with a confirmation dialog and 10 s undo; newly imported
  packs are activated at rank 1; hint when no pack is active; import summary
  (format, events, sounds embedded/loose, background image).
- **Interface v2**: faction-neutral colours with a faction accent, grouped
  navigation, first-start assistant, contrast check (WCAG).
- **Overlay v2**: rounded card in the style of the app, faction accent, clear
  hierarchy, styles Card/Slim/text only, size S/M/L/XL or automatic by monitor
  height, kill feed with symbols (kill green, death orange, own headshot “HS”
  in its own colour, headshot taken “HS taken”). The same design in the OBS
  endpoint, which now also shows pop-ups.
- **Overlay position** through anchor and offset; always fully on the monitor.
- **Achievement pop-up as an object of its own**: pack image without a frame
  with its own alpha, text with shadow (can be switched off), fade, duration,
  integer scaling, own position.
- **Move mode (Ctrl+F2)** in the game: drag both objects, Tab/arrow keys/
  Enter/Esc as temporary global hotkeys; “Reset position” per object.
- **K/D incl. revives** as in the game (revived deaths do not count),
  selectable as the large K/D, as an overlay/OBS block and in all exports.
- **Kill feed colours in the app too** (kill green, death orange, teamkill
  yellow, “HS” cyan, “HS taken” coral orange) with a contrast test; missing
  names are looked up, otherwise the reason is shown (Yourself, Environment,
  Looking up…, Unknown player).
- **New Events page**: base events at the top with an expandable key chain and
  “last played”, special achievements below; **conditions per event in the
  interface** (template + parameters, several rules, reset, “No trigger”),
  saved in `config.toml`.
- **Hit feedback in the crosshair**: flash or image change on kill, headshot
  kill, kill assist, destroyed vehicle and vehicle damage (ESS events only;
  the API does not deliver single hits).
- **Excel export** (.xlsx with eight sheets), **versioned JSON format** with a
  schema and **import** of JSON/Excel (validation, limits, duplicate
  detection).
- **Analytics page**: filter and compare sessions, charts (Chart.js) with PNG
  export.
- **Squad/outfit mode**: follow up to 20 more characters live (Squad page,
  outfit import, sessions per character, delete function); sounds and hit
  feedback only for own characters.
- **Discord Rich Presence** (optional, own application ID).
- **Updates in the app** (optional, signed; AppImage/Windows), set up once the
  signing key is stored.
- **AUR package `ps2-tracker-bin`** (PKGBUILD, .SRCINFO generated in CI on
  tags, instructions in `docs/release.md`) and binary tarball as a release
  file.
- **Session data version 2**: revives/assists/captures, XP per minute,
  achievements and the enemy's faction are stored; database migration to
  schema 2 on start.
- **Game window selection** for “only while the game is active” (list of
  windows, automatic PS2 detection, own rule).
- **Windows version** (NSIS setup, cross build), **Flatpak**, Arch package and
  AppImage from the CI, `SHA256SUMS`, release job for tags `v*`.
- Shared menu component: kept inside the window, keyboard, click outside
  closes.
- Docs: `docs/permissions.md` (all access with code references),
  `docs/roadmap.md`.

### Changed

- Session data version 4 (loadouts of both sides per kill/death and the last
  own loadout); sessions of older versions stay readable, only without classes.
- Session data version 3 (`world_id`, `zone_id`), SQLite schema 3; older
  sessions stay readable.
- CI: the AppImage is no longer a job artefact (artefact limit of the
  instance); on tags the job uploads it straight into the Generic Package
  Registry.
- The overlay is treated like the crosshair: not focusable, fixed size from the
  measured content, no more change of the window type (no moving by dragging;
  positioning is done on the Overlay page). Default: top right, 24 px.
- Own sounds per event are copied into the data folder.
- Audio: without a device choice the ALSA device `pipewire` first; a saved
  “Default Audio Device” is switched to the system default.
- Tighter Tauri permissions per window; a test checks that every command is
  declared and allowed.
- Overlay styles refined: Card (recommended) with gradient and blur, Slim flat
  with an accent bar and full opacity, Text only with a real outline; empty
  rows without “–”.
- JSON export in the new format `ps2-tracker-session` (the old one is still
  read on import).
- Demo data and screenshots with clearly synthetic names.
- `SECURITY.md`: reporting through a confidential GitLab issue, supported
  versions, handling of third-party packs.
- CI: one job for lint and tests, caches, cancelling outdated pipelines,
  apt-get with a time limit and retries.

### Fixed

- **Windows: no more console window on start.** Release builds use the GUI
  subsystem; started from a terminal, the app attaches to its console
  (`--help`, new `--version`, messages of the CLI switches). New debug mode
  `--debug-console` or *Settings → Advanced → Show debug console on start*: log
  level debug, on Windows with its own console window, on Linux in the calling
  terminal. The Win32 calls (AttachConsole, AllocConsole, plus GetClipCursor
  for the move layer) go through `windows-sys` in a single module
  `win_console.rs`; `unsafe_code` stays forbidden everywhere else (`deny`,
  checked by a test).
- **Windows: the move layer detects the cursor grab** of the game
  (`GetClipCursor`) and then shows “The game holds the cursor” as on Linux.
- **Squad**: an added member did not appear in the overlay. Causes: “Squad top
  3” only showed characters with a running session and at least the minimum
  number of kills, and a squad change rebuilt the ESS connection without
  sending overlay/OBS a new state. Now changes are subscribed or unsubscribed
  on the open connection and reported to the Squad page, overlay and OBS at
  once; members below the minimum are shown dimmed behind the others. The Squad
  page points out when the block is off.
- **Hang after Enter/Ctrl+Esc** in the move layer (intermittent): registering
  the hotkeys again ran into the key release; the plugin held its lock on the
  main thread while the X11 hotkey thread waited for the same lock with the
  release. Hotkeys are now registered again only after the release (at most
  1.5 s wait).
- A forwarded command line call (`--toggle-edit` and similar, e.g. as a KDE
  shortcut) rotated the log of the running app; its lines ended up in `.log.1`
  and were lost on the next start.
- A damaged session database made the app quit silently on start; now it is
  moved aside as `tracker.sqlite3.corrupt-<time>`, a new one is created and a
  notice is shown.
- Many events in a short time (fights with a squad, many XP events): every
  event sent the full snapshot and the status to all windows; from a few dozen
  events per second on, the event stream could not keep up and the connection
  dropped. Now waiting events are processed together and the windows updated
  once (status at most once per second); 5000 events at 200/s without a
  connection drop.
- Permissions: unused commands removed (`voice_preview_path` played an
  arbitrary path, `crosshair_next`, `overlay_toggle`, `overlay_nudge`); the main
  window only gets the commands it calls.
- Sounds: errors visible, output reopens after a failure; preview reports
  errors.
- The crosshair stays above the fullscreen game (KDE on-screen display type,
  XWayland).
- Grey window with NVIDIA (`WEBKIT_DISABLE_DMABUF_RENDERER=1` when the driver
  is detected, can be switched off).
- Import “value of guid has an unknown type” with SharpSerializer packs.
- The tracker's own window does not count as a foreign window for focus.
- The import menu ran out of the window on the left.
- Overlay in the Slim style was cut off on the right.
- CI: `windows-nsis` did not find `cargo-xwin` (PATH after the change of
  `CARGO_HOME`); `arch-package` failed on write permissions of the pnpm store.
- Faster CI: one Linux release build for AppImage and Arch package, sccache,
  mold; release files in the Generic Package Registry (permanent URLs).

### For developers

- `tools/fake_ess.py`: local event stream with synthetic kills; debug builds
  connect to it with `PS2_TRACKER_ESS_URL`.
- `tools/e2e/move_layer.sh` and `tools/e2e/grabwin.c`: E2E test of the move
  layer under Xvfb + xfwm4 with a fullscreen window that holds the cursor with
  `XGrabPointer`.

## [0.1.0] – 2026-11-21

First version: Census connection (REST and event stream), sessions,
achievements, overlay, crosshair, voice packs (own format, Recursion XML),
tray, OBS endpoint.
