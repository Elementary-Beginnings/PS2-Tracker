# Permissions and Privacy

This page describes what the PS2 Session Tracker does on the computer and on the
network. Each point names the place in the source code where it can be checked
(paths relative to the [repository](https://github.com/Elementary-Beginnings/ps2-tracker)).
State: version 1.0.0.

**In short:** the app connects only to `census.daybreakgames.com` and
`push.planetside2.com` – and, only after opt-in, to the update endpoint
(GitHub releases), to the ELME stats server (online sync) and locally to
Discord –, sends no telemetry, reads only its
own folders and the files you import (packs, crosshairs, sessions – always
through the file dialog), writes exports only to the chosen place, writes the
service ID only to `config.toml` (mode 0600) and never accesses the game. Under
X11/XWayland it reads window titles and classes to recognise the game window.

## Network

**Hosts contacted** – two always, the others only after opt-in, all fixed in the code (the online sync address can be changed in the settings):

| Host | Protocol | Purpose | Code |
|---|---|---|---|
| `census.daybreakgames.com` | HTTPS (REST) | character search, names of weapons/vehicles, lifetime values; the six official class icons (`/files/ps2/images/static/<id>.png`, once, then cached) | `crates/ps2-tracker-core/src/census.rs` (`CENSUS_BASE`, `static_image`), `class_icons.rs` |
| `push.planetside2.com` | WSS (WebSocket) | events (kills, deaths, login …) of the own characters and the squad members | `crates/ps2-tracker-core/src/ess.rs` (`ESS_URL`) |
| `github.com` (release downloads, redirected to GitHub's download host) | HTTPS | **only after opt-in** (“Check for updates on start”) or “Check now”: `latest.json` of the updater, after confirmation the signed file | `src-tauri/tauri.conf.json` (`plugins.updater.endpoints`), `src-tauri/src/update.rs` |
| `stats.elme.games` | HTTPS | **only after opt-in** (*Settings → Online sync*) and sign-in: sign-in, token refresh, upload of finished sessions of the own characters (see [Online sync](#online-sync-optional)) | `crates/ps2-tracker-core/src/online_sync/` (`DEFAULT_API_BASE`, `api.rs`) |
| `discord.com` | HTTPS, in the **browser** | only when you click *Sign in with Discord*: the tracker opens Discord's authorization page in the system browser; the app itself sends nothing to Discord | `online_sync/mod.rs` (`authorize_url`), `src-tauri/src/sync.rs` (`open_url`) |

The app opens no other connections. There is **no telemetry, no update check, no
analytics, no crash reports**:

- HTTP clients (`reqwest`) are created only in `census.rs`
  (`CensusClient::with_base`) and, for the optional online sync, in
  `online_sync/api.rs` (`SyncApi::new`, its own client, used only while online
  sync is on). The only WebSocket is in `ess.rs`
  (`tokio_tungstenite::connect_async`).
- Tauri plugins: `dialog`, `global-shortcut`, `single-instance` and, since 0.2.0,
  `updater` (`src-tauri/Cargo.toml`, `src-tauri/src/main.rs`). Only the updater
  uses the network, and only through `update::check`: at startup only with
  `updates.check_on_start = true` (off by default), otherwise only by “Check now”.
  It sends the usual HTTPS request (the app version is in the user agent), no
  identifier. Without a public key (`plugins.updater.pubkey` empty) the app never
  asks. Installed is only after confirmation and only a file with a valid
  signature (`requireSignedVersion`: the signature must carry the announced
  version). For Flatpak and package manager installations (marker
  `/usr/share/ps2-tracker/package-managed`) the updater is off. The user interface
  gets no updater plugin commands, only `update_status`/`update_check`/`update_install`.
- The user interface (WebView) may not open network connections: the CSP in
  `src-tauri/tauri.conf.json` allows only `default-src 'self'` and
  `connect-src ipc: http://ipc.localhost`. Images only `'self'` and `data:`, no
  foreign fonts, no foreign scripts.

**Sent:** the Census service ID (as part of the URL `…/s:<ID>/…` or
`?service-id=s:<ID>`), character names in a search, character IDs and
weapon/vehicle IDs in requests, in the WebSocket a subscription request for the
own character IDs (`ess.rs`, `subscribe`).
**Received:** public game data (character info, statistics, events).

**Class icons** (`class_icons.rs`, command `class_icons`): only the six image
IDs are built in (`data/loadouts.json`, `class_images`: 204, 62, 65, 201, 59,
207); no Daybreak graphic is in the repository or the packages. When a window
first shows a class, the backend downloads the missing icons once per run from
`https://census.daybreakgames.com/files/ps2/images/static/<id>.png` – the path
contains no service ID. Accepted only with content type `image/png`, at most
64 KB, a valid PNG of at most 256×256 pixels; the file name is the number. The
icons are cached and handed to the windows as `data:` URLs, so the CSP stays
unchanged. Without network (or in the OBS source) the app shows its own CC0
symbols.

**Name lookup through Census** (`src-tauri/src/engine.rs`, `resolve`,
`resolve_feed_names`): for kills, deaths and vehicles the app requests the names
of the characters and weapons involved (bulk request `character` or `item`, at
most 100 IDs per request, through the shared rate limiter). If a name is still
missing in the visible kill feed (failed or throttled request, a session restored
after a restart), it is requested again every 30 s. If Census does not know a
character, the app asks again only after 10 minutes (`census.rs`,
`NameCache::characters_to_fetch`, `NOT_FOUND_RETRY`). The names go into the local
name cache (`tracker.sqlite3`, table `name_cache`). Only IDs from received events
are sent.

**Server and continent** (`engine.rs`, `resolve_locations`; `census.rs`,
`world_names`/`zone_names`): for the server and continent IDs of the running
sessions the app requests Census `world` or `zone` – only IDs that occurred in
events, each ID once per program run after a successful answer. The names also go
into `name_cache` (kinds `world`/`zone`). The session stores only the IDs (data
version 3).

**Classes** (`classes.rs`): the mapping loadout ID → class is a built-in table
(`crates/ps2-tracker-core/data/loadouts.json`, from the Census collection
`loadout`). There is no request for it at runtime; only an explicitly started,
ignored test (`loadouts_match_census`) compares it with Census (using the public ID
`example`). The session stores the loadout IDs from the events (data version 4).

The service ID does not appear in error messages or in the log: errors of the HTTP
client are passed on without the URL (`census.rs`, `get`, `without_url`; test
`errors_do_not_contain_the_service_id`), disconnect reasons of the event stream
are redacted (`ess.rs`, `run`; test `service_id_not_in_logs_or_reasons`), and the
debug output of `CensusClient`/`EssConfig` shows `***`.

**Built-in service ID of official builds:** official builds contain a project
service ID, embedded at compile time (`option_env!("PS2_DEFAULT_SERVICE_ID")` in
`service_id.rs`; set from a GitHub Actions secret of the `release` environment,
not in the repo). It
can be read from the binary. It grants no access to any account; Daybreak uses
it only to attribute requests to this app and for rate limiting. Order of use:
`PS2_SERVICE_ID` > own ID from the settings > built-in default > `example`
(`ServiceId::resolve`, test `precedence`). The built-in value is never written
to `config.toml` (test `built_in_default_is_not_saved`), never logged and never
sent to the user interface: the UI only learns the source
(`service_id_source`: environment, config, builtin, example). An own ID can be
entered at any time and takes precedence.

## Online sync (optional)

Off by default (`[online_sync] enabled = false`). Details for users:
[Online Sync](Online-Sync). Code: `crates/ps2-tracker-core/src/online_sync/`,
`src-tauri/src/sync.rs`.

- **Nothing without opt-in:** while it is off the app makes no request to the
  stats server and does not touch the keyring (`worker.rs`, `step` returns at
  once; test `switched_off_means_no_request_and_no_queue`). The keyring is
  contacted lazily (`secret.rs`, `KeyringSecret`).
- **Sign-in:** Discord OAuth 2, authorization code with PKCE (S256), scope
  `identify`, random `state` checked on the callback (`pkce.rs`, `loopback.rs`).
  The system browser opens Discord (`sync.rs`, `open_url`: https only, no
  shell). The redirect goes to a listener **only on 127.0.0.1**, first free
  port of 47831/47832/47833, only during the sign-in (at most 5 minutes), for
  exactly one `/callback` with the right `state`; other requests get an error
  page and do not end the wait. Connections are served side by side (at most
  16 at a time, 2 s to send the request), so an idle connection does not hold
  up the callback. The callback page is static HTML without script
  (`no-referrer`, CSP `default-src 'none'`). Code, verifier and redirect URI go
  to `POST /auth/discord` of the stats server; the app has no client secret.
- **Tokens:** the refresh token (30 days, single use) only in the **system
  keyring** (Secret Service on Linux, Credential Manager on Windows; crates
  `keyring-core`, `zbus-secret-service-keyring-store`,
  `windows-native-keyring-store`). No file fallback: without a keyring the
  sign-in is refused (test `sign_in_not_linked_and_without_keyring`). The access
  token (15 minutes) only in memory. Tokens print as `***` (`api.rs`, `Token`);
  error texts carry the HTTP status and the server's error code, never a token
  (test `errors_are_classified_without_secrets`). The client follows no
  redirects, so the bearer token cannot be sent to another host.
- **Sent:** the session file of finished sessions of the **own** characters
  that the server lists for the account (`PUT /sessions/{character_id}/{started_at}`),
  exactly as the JSON export writes it – including the names and IDs of other
  players in the kill feed. Squad sessions are never queued or uploaded (test
  `uploads_with_refresh_rotation_and_retry_on_401`). Also sent: the client name
  and version at sign-in (`client`, shown on the server as "signed in from").
- **Stored locally:** the upload queue in `tracker.sqlite3`, table
  `sync_queue` (state, attempts, next attempt, last error; no tokens). Deleting
  a session deletes its row.
- **API address:** changeable, `https://` only (`http://localhost` only in debug
  builds; `online_sync::api_base`). Each address has its own keyring entry.
  Changing it asks first and signs out at the previous address (`worker.rs`,
  `configure`/`finish_signouts`; test
  `changing_the_api_address_signs_out_at_the_previous_one`). The built-in
  Discord client ID is used only with the default address
  (`SyncSettings::effective_client_id`); another address needs its own client
  ID, otherwise the sign-in stays off.
- **Deleting:** *Sign out* (`POST /auth/logout`, keyring entry deleted),
  *Delete my online data…* (`DELETE /me`: account and all uploaded sessions on
  the server).

**Local OBS endpoint** (`src-tauri/src/obs.rs`): **off** by default (*Settings →
Data & Privacy*). When switched on, it listens only on `127.0.0.1` (port 47823,
configurable) and serves only the overlay page with its assets, `/api/snapshot`,
`/api/events` (server-sent events) and `/api/background`. The main user interface
is not served there (path filter in `asset`); own CSP,
`X-Content-Type-Options: nosniff`, no CORS headers. Only requests with `Host`
`127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>` are answered, others
get 421 (`local_host_only`, test `requests_for_another_host_are_refused`).

## File system

The app reads and writes only in its own folders
(`crates/ps2-tracker-core/src/config.rs`, `Paths::xdg`):

| Content | Linux | Flatpak | Windows |
|---|---|---|---|
| Configuration (`config.toml`, contains the service ID), own `achievements.toml`/`triggers.toml` | `$XDG_CONFIG_HOME/ps2-tracker/` | `~/.var/app/dev.herz.Ps2Tracker/config/ps2-tracker/` | `%APPDATA%\ps2-tracker\` |
| Sessions and name cache (`tracker.sqlite3`), imported voice packs, crosshairs | `$XDG_DATA_HOME/ps2-tracker/` | `~/.var/app/dev.herz.Ps2Tracker/data/ps2-tracker/` | `%APPDATA%\ps2-tracker\` |
| Log (`ps2-tracker.log`, previous run `.log.1`) | `$XDG_STATE_HOME/ps2-tracker/` | `~/.var/app/dev.herz.Ps2Tracker/.local/state/ps2-tracker/` | `%APPDATA%\ps2-tracker\` |
| Class icons from Census (cache, can be deleted) | `$XDG_CACHE_HOME/ps2-tracker/class-icons/` | `~/.var/app/dev.herz.Ps2Tracker/cache/ps2-tracker/class-icons/` | `%LOCALAPPDATA%\ps2-tracker\cache\class-icons\` |

- **Service ID:** your own ID only in `config.toml` (empty = default). The file
  is created with mode **0600** when saving (`config.rs`, `Config::save`:
  temporary file with `set_permissions(0o600)`, then `rename`). Alternatively the
  environment variable `PS2_SERVICE_ID`; it is never written to the file, nor is
  the built-in default of official builds.
- **Imports** (voice packs, crosshairs, own sounds) are read only from paths the
  user chooses in the file dialog or drops on the window
  (`src/main/VoicePage.svelte`, `CrosshairPage.svelte` → commands `voice_import`,
  `crosshair_import`, `voice_add_file`). On import the files are copied into the
  data folder; paths with `..`, absolute paths and symlinks are refused
  (`voicepack.rs`, `recursion.rs` `resolve_path`), sizes are limited.
- **Exports** (session as JSON/CSV/XLSX, chart image, voice pack ZIP; commands
  `export_session`, `save_png`, `voice_export`) are written only to the file
  chosen in the save dialog, after a check (`src-tauri/src/save_target.rs`):
  absolute path without `..`, the extension of the format, an existing folder,
  not a hidden file, not a symlink or folder, and not inside the app's own
  configuration, data or log folder.
- **Own sound for an event** (“Choose custom sound”, command
  `voice_set_override`): the file chosen in the file dialog is checked (codec,
  size) and copied to `<data folder>/custom-sounds/`; the configuration points to
  the copy, not to the original. If the assignment is removed or replaced, the app
  deletes the copy unless another assignment uses it – only files directly in
  `custom-sounds/` (`commands.rs`, `remove_unused_custom`).
- **Removing a pack** (“Remove pack …”, commands `voice_pack_size`,
  `voice_remove_pack`, `voice_undo_remove`): only the pack's folder under
  `<data folder>/voicepacks/<id>` is deleted. The ID is checked (no `..`, no
  separator, not starting with `.`, not the built-in “Standard tones”), the folder
  must be a real directory directly in the pack folder (compared canonicalised,
  symlinks refused) (`voicepack.rs`, `removable_pack_dir`). It is first moved to
  `voicepacks/.trash/` (undo for 10 s) and then deleted with `remove_dir_all`;
  symlinks in the tree are removed as links, not followed. Leftovers in `.trash/`
  are cleaned up at the next start. The source of the original import (ZIP,
  folder, `.rtst_vpk`) is never touched. Afterwards the app removes the pack from
  the active list (the ranks move up) and assignments that point to files in this
  folder.
- **Exports** (sessions as Excel/JSON/CSV, packs, charts as PNG) are written only
  to the place chosen in the save dialog (commands `export_session`,
  `voice_export`, `save_png`). `save_png` accepts only paths ending in `.png` and
  only data with a PNG signature, at most 40 MB (`commands.rs`, `save_png`).
- **Session import** (*History → Import session…*, command `import_session`):
  reads exactly the file chosen in the file dialog, read-only; drag and drop is
  not provided for it. Whether JSON or Excel is decided by the content (ZIP
  signature), not the extension. Limits: file at most 25 MB, embedded data of an
  .xlsx at most 64 MB unpacked, at most 200 000 entries per list; format, version,
  times (including the pause times of the resume window: `paused_secs` between 0
  and the session length, `paused_at` only in a running session), character and
  name lengths are checked before anything is stored
  (`crates/ps2-tracker-core/src/sessionfile.rs`, `read_file`, `validate`). From an
  .xlsx only the own part `customXml/ps2-tracker-session.xml` is read; formulas,
  macros or other parts of the file are not evaluated. Duplicates (same character
  and start time) are not stored. Names from the file add to the name cache
  without overwriting existing ones.
- **What exports contain:** Excel, JSON and CSV files contain, besides the own
  values, the names and character IDs of enemies and teammates from the events,
  plus weapon and vehicle names. These are public game data from the Census API;
  whoever passes a file on passes them on too. The service ID is in no export.
- **Database migration:** at startup the app brings an older `tracker.sqlite3` to
  the current schema version (4: columns, an index and the table `sync_queue` of
  the online sync, in one transaction; `store.rs`, `migrate`). No data is deleted.
- “Open folder” opens only one of the fixed app folders (`commands.rs`,
  `open_folder`: fixed list `config`/`data`/`voicepacks`/`crosshairs`/`logs`, no
  paths from the user interface) with `xdg-open` or Explorer.

## X11 / XWayland

Read are (`src-tauri/src/focus.rs`, `src-tauri/src/x11.rs`):

| Property | Purpose |
|---|---|
| `_NET_ACTIVE_WINDOW` (root) | which window is active (crosshair/overlay only while the game is active) |
| `_NET_CLIENT_LIST` (root) | list of windows for the “Game window” selection; finding the own windows |
| `WM_CLASS`, `_NET_WM_NAME` | recognising the game window (`steam_app_218230`, “PlanetSide 2”) |
| `_NET_WM_PID` | process name of the candidate window (see /proc); finding the own windows |

For the move layer also (`focus.rs`, `game_window_center`): position and size of
the game window (`GetGeometry`, `TranslateCoordinates`), to open the layer on its
monitor.

Set only on the **own** windows (overlay, pop-up, crosshair, move layer): window
type `_KDE_NET_WM_WINDOW_TYPE_ON_SCREEN_DISPLAY` and `_NET_WM_STATE` (above, on all
desktops, not in taskbar/pager) in `x11.rs` `set_window_type`; the input region
(click-through) and `WM_HINTS input=false` are set by GTK/Tauri (`windows.rs`:
`set_ignore_cursor_events`, `focusable(false)`). Only for the own move layer
(`x11.rs`, `activate_layer`): raise it (`ConfigureWindow` `StackMode::Above`),
request the focus (`_NET_ACTIVE_WINDOW` message with source 2, fallback
`SetInputFocus` on the layer) and a short `GrabPointer` on the layer that is
released right away – only to detect whether another client (the game in mouse
look) holds the pointer (“AlreadyGrabbed”). Other windows are never changed.

The app records **neither keyboard nor screen**. Honest note: under X11, any X
client can in principle read the properties of other windows; that is a property
of X11, not of this app. It uses only the ones above.

## /proc

Only for recognising the game window on Linux: `/proc/<pid>/cmdline` (first
argument, file name) or `/proc/<pid>/comm` of the PID from `_NET_WM_PID` of the
window, read-only (`focus.rs`, `process_name`). In Flatpak this is skipped,
because the sandbox has its own PID namespace and the host PID would name a
different process there; `WM_CLASS` and the title are enough then.

**No** ptrace, no reading of process memory, no injection, no Vulkan or DirectX
layer, no access to game files. So there is nothing the anti-cheat software
BattlEye would have to treat as interference with the game: the app is a separate
program with its own windows that reads public data from the network.

## Global hotkeys

Through `tauri-plugin-global-shortcut` (`src-tauri/src/hotkeys.rs`). The plugin
registers exactly the configured combinations (overlay on/off, move mode, by
default Ctrl+F2, crosshair on/off and next crosshair).

**Temporarily in move mode:** while the move layer is open, the app also
registers Tab, the arrow keys, Shift+arrow keys, Enter and Escape
(`hotkeys.rs`, `MOVE_KEYS`), so the overlay and the pop-up can be moved even when
the game holds the focus or the pointer. Escape only while the pointer is free:
if the game holds the pointer, Esc has to reach the game (it releases the pointer
there). When the mode ends (Enter, Esc, Ctrl+F2 or through the user interface)
they are released right away (`hotkeys.rs`, `register`). During that time these
keys do not reach other programs – which is why the mode is only active as long as
it is needed. Registration uses `XGrabKey` under X11/XWayland (crate
`global-hotkey`, `platform_impl/x11`) and `RegisterHotKey` on Windows. The app is
only notified when one of these combinations is pressed – no keylogger, no other
keys. Under native Wayland there are no global hotkeys; the command line switches
(`--toggle-crosshair` …) are there for own desktop shortcuts.

## Audio

Output only (`src-tauri/src/audio.rs`, rodio/cpal: ALSA or PipeWire through
pipewire-alsa, in Flatpak through PulseAudio, on Windows WASAPI). **No input
device** is opened (rodio without the feature `recording`, `src-tauri/Cargo.toml`).

## Discord (optional, local)

Discord Rich Presence (`src-tauri/src/discord.rs`, crate `discord-rich-presence`)
is off by default and becomes active only when switched on and an application ID
is available: the user's own, otherwise the one built into official builds
(`PS2_DEFAULT_DISCORD_APP_ID`, a public client ID, not a secret). Connection **only locally** to the running Discord app: Unix socket
`$XDG_RUNTIME_DIR/discord-ipc-0…9` or named pipe `\\.\pipe\discord-ipc-N` on
Windows. Only the details chosen in the settings are sent (character/faction, K/D,
kills, start time of the session), at most every 15 s; Discord shows them in the
own profile. If Discord is not running, the app retries silently with a growing
interval (up to 5 min). Flatpak: only `xdg-run/discord-ipc-0` and
`xdg-run/discord-ipc-1` are allowed.

## Squad data

Squad and outfit members (*Squad* page, `config.toml` `[[squad]]`, at most 20) are
added to the same ESS subscription; their sessions are stored locally in
`tracker.sqlite3` like the own ones. These are public game data from the Census
API. The outfit import requests Census `outfit`, `outfit_member` (with name and
faction) and `characters_online_status` (`census.rs`, `outfit_members`). “Remove”
removes the character from the list and, on request, deletes all its sessions
(`store.rs`, `delete_sessions_of`). Sounds, pop-ups, lifetime requests and hit
feedback exist only for the own characters.

## D-Bus (Linux)

- Single instance: own name `dev.herz.ps2tracker.SingleInstance` on the session
  bus (`tauri-plugin-single-instance`), so a second start only notifies the first.
- Tray icon through StatusNotifierItem (`libayatana-appindicator`, `tray.rs`).

## Tauri capabilities and CSP

Each window may call only the commands listed in `src-tauri/capabilities/*.json`:

| Window | Allowed |
|---|---|
| `main` | `core:event:default`, `core:window:default`, `core:webview:default` (read-only), dialogs `ask`/`open`/`save` and the app commands the main window calls (not `toast_background`, `crosshair_render_info`, `overlay_content_size` and the commands of the move layer); among them `hotkey_status` (hotkeys that could not be registered, read-only) and the session controls `start_session`, `stop_session`, `resume_session`, `last_session` and `reopen_session` (they take only a character ID, parsed as a number, and work on the app's own database; no file access); the online sync commands `sync_status`, `sync_login`, `sync_cancel_login`, `sync_logout`, `sync_delete_account`, `sync_upload_session` (a session ID), `sync_upload_existing` and `sync_open_privacy` (opens only the privacy page of the configured server). HTTP calls of the online sync run in the Rust backend, the CSP stays unchanged. The other windows cannot start, end or resume sessions. A test (`commands.rs`, `every_command_is_declared_and_allowed`) checks both directions: every command is declared and allowed for a window, and every permission belongs to an existing command |
| `overlay` | events, `get_snapshot`, `get_config`, `toast_background`, `get_overlay_state`, `overlay_content_size` (reports the size of the content), `class_icons` (cached class icons, read-only) |
| `popup` | events, `get_config`, `toast_background`, `get_overlay_state`, `overlay_content_size` |
| `movelayer` | events, `get_config`, `get_snapshot`, `toast_background`, `get_overlay_state` (preview of the objects) and `move_layer_state`/`_set`/`_select`/`_end`/`_reset`/`_monitor` (end of a drag, bar) and `move_layer_activity` (pointer movement only resets the idle timer); positions are limited to the monitor in the backend |
| `crosshair` | events (among them `crosshair-fx` for the hit feedback: kind, colour/image, duration), `crosshair_render_info` |

CSP (`tauri.conf.json`): `default-src 'self'; script-src 'self'; style-src 'self'
'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc:
http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none';
frame-ancestors 'none'`, plus `freezePrototype`.

## Flatpak

Manifest: `packaging/flatpak/dev.herz.Ps2Tracker.yml`.

| finish-arg | Reason | Without it |
|---|---|---|
| `--socket=x11`, `--share=ipc` | overlay/crosshair as XWayland windows (position, window type above fullscreen), focus detection of the XWayland game window, global hotkeys | crosshair ends up below the game, no detection, no hotkeys |
| `--socket=wayland` | main window native when `ui.x11_backend` is off | X11 only |
| `--device=dri` | GPU rendering of WebKitGTK | software rendering |
| `--socket=pulseaudio` | sound | silent |
| `--share=network` | Census API and event stream | no data |
| `--talk-name=org.kde.StatusNotifierWatcher` | tray icon | no tray |
| `--own-name=dev.herz.ps2tracker.SingleInstance` | single instance over D-Bus | several instances |
| `--filesystem=xdg-run/discord-ipc-0`, `…-1` | optional Discord Rich Presence (local socket) | no Rich Presence |
| `--talk-name=org.freedesktop.secrets` | optional online sync: refresh token in the keyring (Secret Service) | no online sync sign-in |

No access to the home directory or other files: imports go through the
FileChooser portal (Cargo feature `dialog-portal`), “Open folder” through the
OpenURI portal.

## Arch package and AppImage

The package installs exactly (`packaging/arch/PKGBUILD`, `package()`):
`/usr/bin/ps2-tracker`, `/usr/share/applications/ps2-tracker.desktop`, icons under
`/usr/share/icons/hicolor/{32x32,128x128,256x256}/apps/ps2-tracker.png`,
`/usr/share/ps2-tracker/ps2-tracker.kwinrule`, `triggers.default.toml`,
`achievements.default.toml`. No setuid binaries, no systemd services, no udev
rules, no hooks; no root is needed at runtime. The AppImage installs nothing.

## Windows

- Installer for the current user (`tauri.conf.json`,
  `bundle.windows.nsis.installMode = "currentUser"`): no administrator rights,
  neither for installation nor at runtime.
- WebView2 is fetched through Microsoft's bootstrapper when needed
  (`downloadBootstrapper`).
- Focus detection: the foreground window and the name of its process; the process
  is opened only with `PROCESS_QUERY_LIMITED_INFORMATION` (crate
  `active-win-pos-rs`, `src/win/platform_api.rs`). There is no list of all windows
  on Windows (it would need unsafe FFI calls that the project keeps out).
- Not code-signed: SmartScreen warns; compare the checksum, see [Installation](Installation#windows-1011).

## What the app does not do

- no telemetry, no advertising, no accounts of its own; update check and online
  sync (Discord sign-in) only after opt-in
- no access to the game process, its memory or its files
- no recording of keyboard, mouse, screen or microphone
- no reading of files outside the own folders and the imports chosen by the user
- no services, no autostart (unless you create the entry yourself), no root
- no connections except to the two hosts named (and, when switched on, the update
  endpoint, the ELME stats server, the local OBS endpoint on 127.0.0.1 and the
  local Discord IPC)

## Check it yourself

```bash
ss -tnp | grep ps2-tracker                 # open connections (Census/push only)
strace -f -e trace=connect -p "$(pidof ps2-tracker)"   # watch connections being opened
ls -l ~/.config/ps2-tracker/config.toml    # -rw------- (0600)
flatpak info --show-permissions dev.herz.Ps2Tracker
grep -rn "https://\|wss://" crates/ src-tauri/src/      # all hosts in the code
cat src-tauri/capabilities/*.json          # allowed commands per window
```

## Deleting data

Remove everything: delete the three folders from the table under “File system”,
i.e. on Linux `~/.config/ps2-tracker`, `~/.local/share/ps2-tracker`,
`~/.local/state/ps2-tracker`; in Flatpak `~/.var/app/dev.herz.Ps2Tracker`; on
Windows `%APPDATA%\ps2-tracker` (uninstall first). Also remove an autostart entry
you created yourself (`~/.config/autostart/ps2-tracker.desktop`), if any.
If you used the online sync: *Delete my online data…* (or *Sign out*) first;
otherwise the keyring entry (service `dev.herz.ps2tracker.online-sync`) stays in
your keyring and the uploaded sessions on the server.

## Dependencies and supply chain

- CI (`.github/workflows/ci.yml`, GitHub-hosted runners): `cargo audit`,
  `cargo deny check advisories bans sources licenses`, `pnpm audit`, gitleaks on
  every push and pull request; Dependabot for Cargo, npm and GitHub Actions
  (weekly, 7-day cooldown, minor/patch grouped).
- Code scanning: CodeQL for Rust, JavaScript/TypeScript and the workflows
  (`codeql.yml`), OpenSSF Scorecard (`scorecard.yml`, results public), and
  dependency review on pull requests (`dependency-review.yml`: fails on known
  vulnerabilities from moderate up and on GPL/AGPL/LGPL dependencies, which
  do not fit the statically linked `MIT OR Apache-2.0` binaries).
- Workflows: `permissions: {}` by default and minimal per job, every action
  pinned to a commit SHA, no `pull_request_target`, checkout without persisted
  credentials; secrets only in the `release` environment (tag builds).
- Release files with `SHA256SUMS` and build provenance attestations
  (`actions/attest-build-provenance`; verify with `gh attestation verify`).
- Local pre-commit hook with gitleaks (`.githooks/pre-commit`).
- No code signing on Windows.
