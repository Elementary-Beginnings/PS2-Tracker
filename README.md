# PS2 Session Tracker

**English** · [Deutsch](README.de.md)

[![CI](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml/badge.svg)](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Elementary-Beginnings/ps2-tracker/badge)](https://scorecard.dev/viewer/?uri=github.com/Elementary-Beginnings/ps2-tracker)

Session stats, overlay, crosshair and voice packs for **PlanetSide 2** – for
Linux (CachyOS/Arch, AppImage, Flatpak) and Windows. Working title `ps2-tracker`.

> **Does not touch the game.** No injection, no hooks, no memory reading, no
> Vulkan layer. All game data comes exclusively from the public **Daybreak Census
> API** (REST and event streaming over WebSocket). Overlay and crosshair are
> separate, external windows; the tracker runs next to the game like any other
> desktop program.

<table>
  <tr>
    <td><img src="docs/screenshots/live-nc.png" alt="Live session (NC)" width="100%"><br><sub>Live session with kill feed (kill green, death orange, HS/HS taken), NC accent</sub></td>
    <td><img src="docs/screenshots/live-light.png" alt="Light mode" width="100%"><br><sub>Light mode (TR)</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/analytics.png" alt="Analytics" width="100%"><br><sub>Analytics: trends, weapons, progress, factions, A vs. B</sub></td>
    <td><img src="docs/screenshots/history.png" alt="History" width="100%"><br><sub>History with import and export (Excel, JSON, CSV)</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/overlay-styles.png" alt="Overlay styles" width="100%"><br><sub>Overlay styles Card, Slim, Text only over a light and a dark scene</sub></td>
    <td><img src="docs/screenshots/overlay.png" alt="Overlay page" width="100%"><br><sub>Overlay page with presets, size and position</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/crosshair.png" alt="Crosshair" width="100%"><br><sub>Crosshair, pixel-exact</sub></td>
    <td><img src="docs/screenshots/hit-feedback.png" alt="Hit feedback" width="100%"><br><sub>Hit feedback: a peak image per event</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/sounds-events.png" alt="Events" width="100%"><br><sub>Events: base events with their key chain, achievements below</sub></td>
    <td><img src="docs/screenshots/trigger-editor.png" alt="Edit condition" width="100%"><br><sub>Condition of an event in the user interface</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/squad.png" alt="Squad" width="100%"><br><sub>Squad: live values of friends/outfit with server and continent, own row highlighted</sub></td>
    <td><img src="docs/screenshots/integrations.png" alt="Updates and Discord" width="100%"><br><sub>Updates (opt-in) and Discord Rich Presence</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/sounds-packs.png" alt="Voice packs" width="100%"><br><sub>Voice packs (open format, three Recursion formats)</sub></td>
    <td><img src="docs/screenshots/setup-assistant.png" alt="First-start assistant" width="100%"><br><sub>First-start assistant</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/overlay-squad.png" alt="Overlay with Squad top 3" width="100%"><br><sub>Overlay block “Squad top 3”, own row marked, empty places as –</sub></td>
    <td><img src="docs/screenshots/overlay-classes.png" alt="Overlay with class symbols" width="100%"><br><sub>Classes in the kill feed (own › enemy) and in “Squad top 3”</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/live-classes.png" alt="Live page with class evaluation" width="100%"><br><sub>Live page: kills/deaths by enemy class, own classes of the session</sub></td>
    <td></td>
  </tr>
</table>

The pictures show the browser preview with synthetic sample data
(“DemoSoldier…”, no real characters or IDs); made with `pnpm build` and a
headless browser on `vite preview` (parameters such as
`?page=analytics&theme=light` are in `src/lib/demo.ts`).

## Downloads

Every tag `v*` creates a [GitHub release](https://github.com/Elementary-Beginnings/ps2-tracker/releases) with all files,
`SHA256SUMS` and build provenance attestations (GitHub Actions,
`.github/workflows/release.yml`).

| Platform | File |
|---|---|
| CachyOS / Arch | `ps2-tracker-x86_64.pkg.tar.zst` |
| Arch (AUR) | `ps2-tracker-bin.PKGBUILD` / `.SRCINFO` as release files, see [docs/release.md](docs/release.md) |
| Linux (any) | `ps2-tracker-x86_64.AppImage` |
| Linux (binary tarball) | `ps2-tracker-x86_64-linux.tar.gz` |
| Linux (Flatpak) | `ps2-tracker.flatpak` |
| Windows 10/11 | `ps2-tracker-x64-setup.exe` |
| Checksums | `SHA256SUMS` |

Check before installing: `sha256sum -c SHA256SUMS --ignore-missing`, and
optionally the provenance: `gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`.

Own development (clean room). Features are derived from public descriptions; no
code, graphics or sounds of another tracker were taken. The Recursion voice pack
format was worked out solely from an own pack (see
[docs/voicepacks.md](docs/voicepacks.md)).

The user interface and the documentation are in English (this README also in
German); commit messages are in German.

## Privacy and permissions

In short: the app connects only to `census.daybreakgames.com` and
`push.planetside2.com` – and, only after opt-in, to the update endpoint
(GitHub releases) and locally to Discord –, sends no telemetry, reads only its
own folders and the files you import (packs, crosshairs, sessions – always through
the file dialog), writes exports only to the chosen place, writes the service ID
only to `config.toml` (mode 0600) and never accesses the game. Under X11/XWayland
it reads window titles and classes to recognise the game window. All details with
the source code locations and instructions to check them yourself:
**[docs/permissions.md](docs/permissions.md)**.

## Features

- **Live session**: kills, deaths, K/D, KPM, headshots/HSR, revives, assists,
  XP/h, teamkills, suicides, vehicle kills, kill streak and best streak, kills/HS
  per weapon.
- **K/D incl. revives** as in game: deaths you were revived from (experience 7
  “Revive”, 53 “Squad Revive”) do not count. Optionally as the big K/D on the live
  page (*Settings → Big K/D on the live page*), as an overlay/OBS block and in all
  exports; the true K/D stays visible next to it. A lifetime comparison exists only
  for the true K/D (Census keeps no lifetime K/D adjusted for revives).
- **Kill feed** in the app, overlay and OBS: kill green, death orange, teamkill
  yellow, own headshot “HS” cyan, headshot taken “HS taken” coral, always with
  symbol and text (colour is never the only information, contrast checked by a
  test). Missing names are looked up later; without a name the reason is shown
  (Yourself, Environment, Looking up…, Unknown player).
- **Classes** (Infiltrator, Light Assault, Combat Medic, Engineer, Heavy Assault,
  MAX; NSO counts as the same classes): from `attacker_loadout_id`/
  `character_loadout_id` of Death events and `loadout_id` of GainExperience. In
  the kill feed (app, overlay, OBS) a small, own class symbol (drawn for this
  project, no game graphics) stands before the name, with the class name as
  tooltip – the victim's class for a kill, the attacker's for a death; optionally
  the own class before it (*Overlay → Widgets → Show my class in the kill feed*).
  “Squad top 3” and the squad table show the current class of each member (last
  loadout seen, “–” without an event). The live page has the collapsible section
  *Classes*: kills and deaths by enemy class and the own classes of the session
  (kills, deaths, approximate time from the gaps between kills/deaths in the same
  class). The mapping loadout → class comes from the Census collection `loadout`
  (24 entries, `crates/ps2-tracker-core/data/loadouts.json`) and is built in –
  there is no extra request; unknown IDs stay “–”. Sessions before data version 4
  have no classes.
- **Comparison with lifetime values** (`characters_stat_history`,
  `characters_weapon_stat_by_faction`), also per weapon.
- **Several characters** (e.g. one per faction), automatic start at login or the
  first event, end at logout or after inactivity.
- **Sessions** in SQLite (schema version 3, session data version 4; old databases
  are migrated at startup; a damaged file is moved aside and created anew), list,
  detail view, export **Excel**/JSON/CSV, **import** of JSON and Excel – see
  [Analytics, export and import](#analytics-export-and-import).
- **Analytics page**: filter and compare sessions, charts with PNG export.
- **Achievements** as an own TOML rule set (multikill, streak, headshot series,
  revenge, vehicle kills …).
- **Overlay window**: transparent, borderless, always on top, click-through;
  blocks and presets (Minimal/Streamer/Full), three styles – **Card**
  (recommended), **Slim** (flat, no frame) and **Text only** (outline instead of
  a panel) –, size S/M/L/XL or automatic by monitor height. Two separately
  positioned objects: the stats overlay and the achievement/voice pack pop-up
  (the pack image unchanged with its alpha channel, no frame, text with shadow,
  fade in/out, adjustable duration).
- **Moving directly over the game (move layer, Ctrl+F2)** – see
  [Moving the overlay](#moving-the-overlay-move-layer). Alternatively anchor,
  offset and “Reset position” on the Overlay page; always fully on the monitor.
- **Crosshair window**: own PNG, pixel-exact, exactly centred, scaling, opacity,
  tint, X/Y offset, monitor, hotkeys; optionally only while the game is active.
- **Built-in crosshairs** (generated in code, `crosshair.rs`, CC0, pixel-exact
  without anti-aliasing): dot, cross, circle as before, plus `cross` in three
  sizes (32, 24, 12 px; 2 px bars, the large ones with a gap in the middle),
  `cross-dashed` and `dot`, each in green, magenta and red with a light dark
  outline, and the range ladder `range-ladder` in orange for 1440p and 4K (line
  with dots, diamond in the middle, below it a ladder with 1–9; diamond exactly at
  the image centre, the spacing is a guide, not calibrated to a weapon). The
  crosses with 2 px bars have an even side length; their centre lies between two
  pixels, like the centre of a monitor with an even resolution. New built-in
  crosshairs are added to the library at the next start; existing files stay
  unchanged. As files: `cargo run -p ps2-tracker-core --example crosshair_dump -- <folder>`.
  **Own PNGs** – also templates from the web – can be added at any time with
  *Crosshair → Import PNG* (up to 512×512).
- **Hit feedback** in the crosshair (like the hit markers in Recursion, with
  honest limits): see [Crosshair feedback](#crosshair-feedback-hit-feedback).
- **Sounds & voice packs**: own open format and Recursion packs (`.rtst_vpk`,
  three formats), priority queue, selectable output device, conditions per event
  editable in the user interface – see [Voice packs](#voice-packs).
- **Squad/outfit mode**: follow friends or outfit members (up to 20) – page
  *Squad* with live values per character (K/D, KPM, HSR, kills, revives, status),
  sortable, own row highlighted, import from an outfit (tag or name, optionally
  only online members), stored sessions per character and a delete function,
  column *Server / Continent* (last server and continent of the running session,
  names from Census). Sounds, pop-ups and hit feedback still apply only to the own
  active character.
- **Overlay block “Squad top 3”** (Overlay page → Widgets, also in the OBS
  endpoint): the three best characters of the running sessions (own and squad) by
  K/D, kills or KPM, with faction badge; the own row is marked. K/D and KPM need a
  minimum number of kills (default 5): characters below it are still listed, but
  after the others, sorted by kills, with a dimmed value “kills/minimum” (header
  “min. 5 kills”). Squad members appear even without an event; empty places (“–”)
  exist only when there are fewer than three characters. The block always has
  three rows, so the overlay keeps its size; only switching it on or off measures
  again. The block is off by default; the Squad page points to it with a link. A
  new or removed member (also by outfit import) is subscribed or unsubscribed
  right away on the open ESS connection (`ESS subscription updated added=…
  removed=…` in the log); Squad page, overlay and OBS show it immediately.
- **Discord Rich Presence** (optional, off by default): character/faction, K/D,
  kills, session duration, optionally continent and server in the Discord status
  over the local Discord IPC; needs an own Discord application ID (*Settings →
  Updates & Discord*).
- **In-app updates** (optional, off by default; AppImage and Windows): signed
  updates, only after opt-in or “Check now” and a confirmation; Arch/AUR/Flatpak
  update through the package manager. Active only once the maintainer has set up
  the signing key ([docs/release.md](docs/release.md)).
- **OBS browser source** (optional, `127.0.0.1` only).
- **Tray icon**, single instance, no telemetry.

## Moving the overlay (move layer)

<table>
  <tr>
    <td><img src="docs/screenshots/move-layer.png" alt="Move layer over the game" width="100%"><br><sub>Move layer: both objects at real size, bar at the bottom</sub></td>
    <td><img src="docs/screenshots/move-layer-grab.png" alt="Hint when the game holds the cursor" width="100%"><br><sub>The game holds the cursor: Esc in the game or the keyboard</sub></td>
  </tr>
</table>

**Ctrl+F2** (or `ps2-tracker --toggle-edit`, or *Overlay → Move mode*) puts a
dimmed layer over the whole monitor of the game. On it, the stats overlay and the
pop-up are frames at their real size and real position, with their real content.

- **Drag** with the mouse; the objects snap to the monitor edges and centre and to
  the other object (hold **Alt**: free), optionally a 10 px grid. The position in
  pixels is shown under the frame.
- **Keyboard** (always works, also without a mouse): **Tab** switches the object,
  **arrow keys** 1 px, **Shift+arrows** 10 px, **Enter** or **Ctrl+F2** saves,
  **Esc** discards and keeps the old position. The bar at the bottom also has
  *Save*, *Cancel*, *Reset* (default corner) and *Move to monitor…*.
- **Finishing always works**: **Ctrl+Esc**, on Windows **Ctrl+Shift+F2** (own key
  “Finish move mode”, changeable on the Overlay page) saves and ends – as a global
  hotkey for the whole move session, also while the game holds the cursor. Also:
  tray menu *Finish move mode*, the button on the Overlay page or the banner at the
  top of the main window, `ps2-tracker --finish-move` (or `--toggle-edit`), and by
  itself after 5 minutes without input (saves, with a notice in the main window;
  adjustable or switchable off on the Overlay page). Discarding only with *Cancel*
  or Esc without a pointer grab.
- **If the game holds the mouse pointer** (mouse look), the layer shows *“The game
  holds the cursor”* in large letters: **press Esc in the game** (opens the game
  menu and releases the pointer) or work with Tab/arrow keys. While the game holds
  the pointer, Esc is not a hotkey of the app, so it reaches the game; afterwards
  Esc means “Cancel” again.
- Only when saving are the real windows moved to the new position, **once** (with
  a position check in the log). As before, positions are stored relative to the
  monitor (corner + offset).

Technique: like the crosshair, the layer is a window of the type “on-screen
display” and therefore stays above the fullscreen game under KWin. Dragging happens
inside the layer, not through the window manager: KWin never moves OSD windows
(`X11Window::isMovable()` is false for such windows). The layer requests the
keyboard focus with `_NET_ACTIVE_WINDOW`; whether the game holds the pointer it
detects with a short own `XGrabPointer` attempt (answer “AlreadyGrabbed”). In the
log: `move layer opened monitor=… focus=granted|denied pointer=free|grabbed`,
`move layer saved stats=… popup=…` or `move layer cancelled`, then
`move layer finished via=hotkey|tray|button|cli|timeout`.

**KDE/KWin and Ctrl+F2:** KWin often binds Ctrl+F2 to “Switch to Desktop 2”. Then
the app cannot register the key: the log shows
`hotkey registered failed hotkey=Ctrl+F2 …`, Settings and the Overlay page warn,
and the move layer says “Ctrl+F2 is taken by your desktop – use Ctrl+Escape or the
tray menu”. Recommendation: free the key in the KDE system settings (*Keyboard →
Shortcuts → KWin*) or choose another key on the Overlay page; the mode can always
be ended with Ctrl+Esc, through the tray menu or with `--finish-move`.

## Voice packs

Folders, ZIPs and single `.rtst_vpk` files are imported (*Sounds → Packs →
Import*). Three Recursion formats are recognised (details and markers in
[docs/voicepacks.md](docs/voicepacks.md)):

| Format | Recognised by | Examples |
|---|---|---|
| XML (as of 2014) | text with `<NAME>` entries, sounds as files in the folder next to it | ArnoldVoicePack |
| SharpSerializer, binary | first bytes `02 01 04 52 6f 6f 74 01`, sounds embedded or as a list | Deadpool v2, C&C V6, Camikaze78 |
| protobuf (protobuf-net) | first bytes `0a …`, type names with `RecursionTracker.`, sounds embedded | ArnoldVoicePackBETA |

A .NET BinaryFormatter reader is included as a precaution. Packs are only read
(size and depth limits, no path outside the pack folder, foreign absolute paths are
ignored); the repo contains no Recursion files.

Mapping: a game event produces all matching events; the most specific match in
the active packs is played (by rank), the built-in default tones only as a last
resort.

| Game event | Keys (specific → general) |
|---|---|
| Kill | kill chain > kill streak > `ROAD_KILL`/`KNIFE_FIGHT`/`FIRST_BLOOD`/`REVENGE` … > `HEAD_SHOT` (headshot) > `PER_KILL` > own key “every kill” > default `kill`/`headshot` |
| Own death | `BEING_FARMED`/`DEATH_STREAK` > `DEATH_INSULT` > default `death` |
| Teamkill done | `TEAM_KILLER` > default `teamkill` |
| Killed by a teammate | `WHOSE_SIDE` > `PREVENT_FF` > default `teamkilled` |
| Session start | `SESSION_STARTED` > default `session_start` |

| Kill chain | Recursion classic | Unreal |
|---|---|---|
| 2 / 3 / 4 | `EXCELLENT` / `OUTSTANDING` / `SUPERB` | `DOUBLE_KILL` / `TRIPLE_KILL` / `MULTI_KILL` |
| 5 … 10 | `MULTI_KILL` … `HOLY_SHIT` | `MEGA_KILL` … `HOLY_SHIT_2` |

Style under *Sounds → Events → Kill chain style* (“Automatic” = classic if a pack
has `EXCELLENT`). 35 of the 96 Recursion keys have a preset; all others can be
assigned under *Sounds → Events* with “Choose a condition…” (template +
parameters, no TOML). The Events page shows the base events with their key chain
and “last played” at the top.

## Analytics, export and import

- **Export** (*History → Session*): **Excel** (.xlsx) with the sheets Summary,
  Kills, Deaths (with the columns *Enemy class* and *My class*), Classes (kills
  and deaths by enemy class, own classes with approximate minutes), Weapons,
  Vehicles, Revives & Assists, Achievements and Timeline (series per minute) as
  real Excel tables with number, percent and date formats (UTC). **JSON** in the
  versioned format `ps2-tracker-session` (schema:
  [docs/session-file.schema.json](docs/session-file.schema.json)), **CSV** as a
  simple event list.
- **Import** (*History → Import session…*, only through the file dialog): JSON and
  Excel files exported by the tracker (the session is also embedded as data in the
  .xlsx; if a spreadsheet program saves the file again, that data is lost and the
  import rejects the file with a notice). Checked are format, version, times, size
  (25 MB) and list lengths; a session of the same character with the same start
  time is not created twice.
- **Analytics**: choose sessions by character and period (up to 50), charts of
  K/D, K/D incl. revives, KPM, HSR, XP/h across sessions, weapon comparison,
  progress of one session, enemy factions (faction colours), enemy classes and
  session A vs. B with differences; dark/light; PNG export per chart. Charts with
  Chart.js 4 (canvas, no `eval`, the CSP stays `script-src 'self'`; loaded only
  when the page is opened).
- Sessions from versions before 0.2.0 (marked “basic”) contain only kills, deaths
  and vehicles in the detail view; XP progress, revives/assists, achievements and
  the enemies' faction are missing there.

## Crosshair feedback (hit feedback)

*Crosshair → Hit feedback*: the crosshair briefly lights up in a colour or
switches for a moment to another PNG from the library – per event with duration
(default 150 ms), fade, opacity peak and preview.

**Limits, honestly:** the Census API and the event stream provide **no hit or
damage events** for infantry, only deaths, vehicle destructions and experience.
Single hits would only be visible in the game itself – which the app never
touches. The triggers are therefore:

| Event | Source | Default |
|---|---|---|
| Kill | Death event, you are the attacker | on, white |
| Headshot kill | the same with `is_headshot` | on, red |
| Kill assist | experience 2, 371, 372 (Kill Player / Priority / High Priority Assist) | off |
| Vehicle destroyed | VehicleDestroy, you are the attacker | off |
| Vehicle damage | experience “… Damage (Infantry vs Vehicle)” or “… Damage” (25 IDs, listed in `hitfx.rs`) | off |

Own death, suicide and teamkill trigger nothing. The feedback comes with the delay
of the event stream (typically a few hundred milliseconds); the log gives the time
since the ESS timestamp per effect (`crosshair feedback … delay_ms`; the timestamp
has a resolution of one second). Only the content of the crosshair window is drawn
– size, position and window type stay the same.

## Installation

Official builds work out of the box with a built-in Census service ID; your own
ID (see below) takes precedence and can be entered at any time. Builds from
source without one need your own ID; the first-start assistant asks for it.

### CachyOS / Arch

```bash
git clone https://github.com/Elementary-Beginnings/ps2-tracker.git
cd ps2-tracker/packaging/arch && makepkg -si
# or the finished package from the release:
sudo pacman -U ps2-tracker-x86_64.pkg.tar.zst
```

**AUR:** `ps2-tracker-bin` (e.g. `paru -S ps2-tracker-bin`) installs the
prebuilt binary of a release. The package works only once there is a tagged
release and the package is published in the AUR; until then `makepkg` from the
repo (above) is the way.

`makepkg` installs the dependencies (`webkit2gtk-4.1`, `gtk3`,
`libayatana-appindicator`, `alsa-lib`, `librsvg`; for building `rust`, `nodejs`,
`pnpm`). Under PipeWire, `pipewire-alsa` is needed. Autostart:
`cp /usr/share/applications/ps2-tracker.desktop ~/.config/autostart/` and set
`Exec=ps2-tracker --hidden` there.

### AppImage (other distributions)

```bash
chmod +x ps2-tracker-x86_64.AppImage && ./ps2-tracker-x86_64.AppImage
```

### Flatpak

```bash
flatpak install --user ps2-tracker.flatpak     # bundle from the release
flatpak run dev.herz.Ps2Tracker
```

The bundle needs the runtime `org.gnome.Platform//51` from Flathub (fetched during
installation when Flathub is set up). The permissions and their reasons are in the
manifest `packaging/flatpak/dev.herz.Ps2Tracker.yml`: X11 (overlay, crosshair and
focus detection run through XWayland), Wayland, PulseAudio, network, tray. No
access to the home directory – packs and crosshairs are imported through the file
dialog of the portal. The data is under `~/.var/app/dev.herz.Ps2Tracker/`. Global
hotkeys go through X11 and work in the sandbox with `--socket=x11`; under Wayland,
create own KDE shortcuts as with the other packages
(`flatpak run dev.herz.Ps2Tracker --toggle-crosshair`).

### Windows 10/11

Run `ps2-tracker-x64-setup.exe` (installation for the current user, no
administrator rights). If WebView2 is missing, the installer downloads Microsoft's
bootstrapper (`downloadBootstrapper`, Tauri's default; Windows 11 and current
Windows 10 systems already have WebView2).

- **Not signed:** there is no code signing certificate, so SmartScreen warns
  (“Windows protected your PC” → *More info* → *Run anyway*). Compare the checksum
  first: `Get-FileHash .\ps2-tracker-x64-setup.exe -Algorithm SHA256` against
  `SHA256SUMS`.
- **Overlay and crosshair only in windowed or borderless fullscreen mode:** in
  exclusive fullscreen the game owns the display alone; Microsoft describes that
  overlays are then only possible by interfering with the game's rendering, while
  in a borderless window or with fullscreen optimizations “the overlay can be
  composited on top of the game in a safe and stable way”
  ([DirectX Developer Blog: Demystifying Full Screen Optimizations](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations)).
  In PlanetSide 2, therefore choose *Windowed Fullscreen* or *Borderless*.
- Windows: overlay and crosshair are borderless, transparent windows with
  `WS_EX_TOPMOST`, `WS_EX_LAYERED | WS_EX_TRANSPARENT` (click-through) and
  `WS_EX_NOACTIVATE` (take no focus) – set by Tauri/tao (`always_on_top`,
  `set_ignore_cursor_events`, `focusable(false)`).
- Focus detection: the foreground window and the file name of its process
  (`PlanetSide2_x64.exe`, matches the pattern `planetside2`). The process is opened
  only with `PROCESS_QUERY_LIMITED_INFORMATION` to read the name.
- Data: `%APPDATA%\ps2-tracker\` (configuration, database, packs, log).
- No console window at startup (GUI subsystem); output only when the app is
  started from a terminal, and in debug mode – see [Troubleshooting](#troubleshooting).
- Hotkeys through `RegisterHotKey` (global-hotkey). Ctrl+Esc is reserved system-wide
  on Windows (Start menu); “Finish move mode” is therefore **Ctrl+Shift+F2** there
  (if another hotkey of the app already uses that combination: Ctrl+Alt+F2). An
  existing Windows configuration with Ctrl+Esc is switched at startup (log:
  `finish move mode key migrated`). If a key cannot be registered, the app shows
  that as under KWin (Settings, Overlay page, move layer); then choose another key
  on the Overlay page. Tray menu, button and `--finish-move` always work.
- Move layer: borderless window with `WS_EX_TOPMOST` over the game (borderless
  fullscreen), foreground through Tauri/tao (`SetForegroundWindow`, with tao's way
  around the foreground lock). Whether the game confines the mouse pointer, the app
  reads with `GetClipCursor` (clip rectangle smaller than the virtual screen =
  confined) before opening and then every 500 ms; the layer then shows “The game
  holds the cursor” as on Linux. The layer takes the foreground when it opens; many
  games release the pointer then (not checked for PlanetSide 2 on Windows). The
  keyboard of the layer works in any case. The X11 parts (OSD window type,
  `_NET_ACTIVE_WINDOW`, grab probe) exist only on Linux. The Win32 calls without a
  safe wrapper (console, `GetClipCursor`) are in `src-tauri/src/win_console.rs`,
  the only module with `unsafe` code (otherwise `unsafe_code = "deny"`, checked by
  a test).
- Focus detection on Windows sees only the foreground window (listing all windows
  would need `EnumWindows`, also unsafe FFI); the move layer therefore opens on the
  game's monitor only when the game is in front, otherwise on the overlay's monitor.

## Census service ID

The Census API needs a service ID. Which one the app uses, in this order:

1. the environment variable `PS2_SERVICE_ID` (never written to the file),
2. your own ID from **Settings → Census API** (without `s:`; stored in the local
   configuration),
3. the built-in default of official builds,
4. `example` (only very few requests possible).

**Official builds contain a project service ID.** It is embedded at build time
(CI variable, not in the repo) and can be read from the binary. It grants no
access to any account; Daybreak uses it only to attribute requests to this app
and for rate limiting. The app never shows it, never logs it and never writes it
to the configuration – Settings only says "built-in default". You can enter your
own ID at any time; register for free at
<https://census.daybreakgames.com/#devSignup>. Leave the field empty to go back
to the default. Builds from source have no built-in default
(`PS2_DEFAULT_SERVICE_ID` at compile time, letters, digits and `_` only) and use
`example` until you enter an ID.

## Configuration and data

| What | Path |
|---|---|
| Configuration (also through the UI) | `$XDG_CONFIG_HOME/ps2-tracker/config.toml` |
| Own achievements | `$XDG_CONFIG_HOME/ps2-tracker/achievements.toml` |
| Own sound triggers (whole rule set) | `$XDG_CONFIG_HOME/ps2-tracker/triggers.toml` |
| Conditions changed in the user interface | `config.toml`, `[voice.trigger_overrides]` |
| Sessions and name cache | `$XDG_DATA_HOME/ps2-tracker/tracker.sqlite3` |
| Voice packs | `$XDG_DATA_HOME/ps2-tracker/voicepacks/` |
| Crosshairs | `$XDG_DATA_HOME/ps2-tracker/crosshairs/` |
| Log (current run, previous one as `.log.1`) | `$XDG_STATE_HOME/ps2-tracker/ps2-tracker.log` (usually `~/.local/state/ps2-tracker/ps2-tracker.log`) |

Templates: `crates/ps2-tracker-core/assets/achievements.default.toml` and
`triggers.default.toml` (in the package under `/usr/share/ps2-tracker/`).

## Overlay and crosshair under KDE Plasma (Wayland)

Both are separate, transparent Tauri/GTK windows.

### Why the app runs through XWayland

KWin orders windows in layers (source: `Window::belongsToLayer()` in
`kwin/src/window.cpp`, order in `src/effect/globals.h`):

`Above` (“Keep above others”) < `Notification` < `Active` (active fullscreen
window) < `Popup` < `CriticalNotification` < `OnScreenDisplay` < `Overlay`

A game in *Fullscreen Windowed* mode is an active fullscreen window and therefore
lies **above** every “keep above” window – exactly the reported bug (the
crosshair disappears in borderless fullscreen, not in windowed mode). Only the
layers `CriticalNotification`, `OnScreenDisplay` and `Overlay` are above it.

- A **native Wayland window** can choose neither its position nor such a layer
  itself; GTK3/Tauri do not offer layer shell (`wlr-layer-shell`).
- An **X11 window** (through XWayland, like the game under Proton) can set the
  window type `_KDE_NET_WM_WINDOW_TYPE_ON_SCREEN_DISPLAY`; KWin then puts it into
  the layer `OnScreenDisplay`, above the fullscreen game.

That is why the app restarts itself once with `GDK_BACKEND=x11` in a Wayland
session (log: `backend="x11"`). After that it sets the window type “on-screen
display” for crosshair and overlay through X11 (fallback
`_NET_WM_WINDOW_TYPE_NOTIFICATION` for other window managers) and maps the window
once more, because KWin reads the type only when it starts managing a window. The
window takes no focus (`WM_HINTS input=false`) and stays click-through. The
overlay no longer changes its type (it used to get the normal type back for
moving; every change needs a new mapping, after which the window manager may place
the window anew). It is handled like the crosshair: fixed size (the measured size
of its content, changing only with the settings, never by new kill feed rows or
pop-ups), position from anchor and offset. The log shows `overlay placed:
preset=…, size=…, requested x,y, monitor=…` and shortly after `overlay position
check requested=… actual=…` (on a difference the app sets the position again, up
to three times in 10 s).

Switch off: untick *Settings → Appearance → Run windows through XWayland*
(`ui.x11_backend = false`) or set `PS2_TRACKER_WAYLAND=1`. Switch off only the
window type (diagnosis): `PS2_TRACKER_NO_OSD=1`.

### KWin window rule (additionally recommended)

*System Settings → Window Management → Window Rules → Import* and load
`/usr/share/ps2-tracker/ps2-tracker.kwinrule` (in the repo
`packaging/kde/ps2-tracker.kwinrule`). The rule forces the **layer “On-screen
display”** for overlay, pop-up and crosshair (matched by the exact window title)
(`layer=osd`, `layerrule=2` = force), “Keep above others”, “Accept focus: no”, and
hides them from the taskbar, pager and window switcher. The rule property “Layer”
exists in KWin since at least 6.2 (checked in `src/rulesettings.kcfg` of the tags
v6.2.0 to v6.6.0). The rule works even if the window type could not be set. With
the Wayland backend it also applies to the layer, but the position of the
crosshair is then left to KWin.

A KWin script is no alternative: in the script API `layer` is read-only
(`Q_PROPERTY(KWin::Layer layer READ layer)` in `src/window.h`), and `keepAbove`
only lifts into the layer `Above`, which is below the fullscreen game.

### More

- **Transparency**: needs a compositor; KWin always composites.
- **Click-through**: GTK input region. tao (Tauri) leaves a 1×1 pixel area at the
  top left of the window clickable; for the crosshair it lies in the transparent
  border.
- **Hotkeys**: global hotkeys work only through X11/XWayland. Alternatively create
  commands under *System Settings → Shortcuts → Custom Shortcuts*:

  | Action | Command |
  |---|---|
  | Overlay on/off | `ps2-tracker --toggle-overlay` |
  | Overlay move mode on/off | `ps2-tracker --toggle-edit` |
  | Crosshair on/off | `ps2-tracker --toggle-crosshair` |
  | Next crosshair | `ps2-tracker --next-crosshair` |

  The running instance takes over the command.
- **Crosshair only while the game is active**: reads `_NET_ACTIVE_WINDOW`,
  `WM_CLASS` and the window title from the X server (XWayland); under Proton
  PlanetSide 2 reports itself as `steam_app_218230`. It is hidden only when another
  X window is certainly active. If the detection is uncertain (no X server, query
  failed, native Wayland window active), the crosshair stays visible. The detected
  state is under *Crosshair → Check active window* and in the log.
- **HiDPI/fractional scaling**: window size and position in physical pixels; the
  log names monitor, scaling and window position on every change.

A crosshair overlay is an external window above the game and does not change the
game.

**OBS**: enable *Settings → OBS browser source* and add a browser source in OBS
for `http://127.0.0.1:47823/`.

## Troubleshooting

The app writes its log to `~/.local/state/ps2-tracker/ps2-tracker.log`
(`$XDG_STATE_HOME`; the previous run stays as `ps2-tracker.log.1`). The path is
also under *Settings → Files*. Detailed logging:

```bash
pkill ps2-tracker                       # end the running instance
ps2-tracker --debug-console             # debug mode, log also in the terminal
# or: RUST_LOG=ps2_tracker=debug,ps2_tracker_core=debug ps2-tracker
# then send ~/.local/state/ps2-tracker/ps2-tracker.log
```

**Debug mode** (`--debug-console` or *Settings → Advanced → Show debug console on
start*, off by default): log level debug for the app. On Linux the log also
appears in the terminal the app was started from (started from the menu, no
terminal opens; all `.desktop` files have `Terminal=false`).

**Windows:** the release build starts without a console window (GUI subsystem).
Started from a command prompt or PowerShell, it writes `--help`, `--version` and
the messages of `--toggle-*`/`--finish-move` to that window (the command prompt
shows its prompt again before the output). For the debug mode, create a shortcut
to `ps2-tracker.exe` and append ` --debug-console` under *Target*, or set the
setting above and restart the app: a console window of its own, “PS2 Session
Tracker – debug log”, then opens with the log. The log is always under
`%APPDATA%\ps2-tracker\` (path under *Settings → Advanced*).

What the log contains:

- Start: version, session type, `WAYLAND_DISPLAY`/`DISPLAY`/`GDK_BACKEND`, the
  window backend used, monitors with scaling.
- Sound: audio host, default device and all output devices; which device was
  opened (rate, channels, buffer); per game event the chosen sound or the reason
  why none plays (muted, no entry/cooldown, `SOUND=false`); start, end,
  displacement and dropping in the queue; errors when opening or decoding, with
  file path and codec; errors of the output stream. If the output stream stalls
  (the device no longer fetches samples), that is detected (`audio output
  stalled`) and the device is opened again. Panics in any thread are logged too.
- Crosshair: every visibility change with its reason (start, configuration,
  hotkey, focus watcher), monitor/position, detected focus state with `WM_CLASS`,
  X11 window type set; per hit feedback effect its kind and the delay since the
  ESS timestamp.
- Sounds: per event the key chain checked, with the file
  (`voice: kill: tried DOUBLE_KILL(miss) … PER_KILL(hit, Pack: file.ogg)`).
- Import: the reason of a rejection (format, version, size, missing embedded data)
  as a message in the user interface; `session imported` in the log.

The sound preview (▶) reports errors as a message in the user interface (e.g.
file missing, codec not supported, no output device). Supported are Ogg Vorbis,
MP3, FLAC and WAV; Ogg Opus is recognised and rejected with a notice. Under
PipeWire the app needs `pipewire-alsa`: if no device is chosen (*System default
(PipeWire)*), it first opens the ALSA device `pipewire` and only then the ALSA
default device (where `dmix` occasionally fails with “unable to open slave”). The
opened device is in the log (`audio output opened`).

### Grey/empty window (NVIDIA)

With the proprietary NVIDIA driver, the WebKitGTK window often stays grey or empty
(DMA-BUF renderer over GBM). If the app detects the NVIDIA driver at startup
(`/sys/module/nvidia` or `/proc/driver/nvidia/version`), it sets
`WEBKIT_DISABLE_DMABUF_RENDERER=1` and restarts itself with it. The Tauri
documentation describes this environment variable as a workaround for empty
windows (<https://v2.tauri.app/develop/debug/linux-graphics/>) and advises against
setting it across the board – hence only for NVIDIA. The log has a line
`graphics environment` with the detected driver and the origin of the variable
(`set by ps2-tracker`, `set by the user` or `unset`).

Switch off: `PS2_TRACKER_NO_DMABUF_FIX=1 ps2-tracker` or in `config.toml` under
`[ui]` `webkit_dmabuf_workaround = false`. If you set the variable yourself, your
value is kept.

`PS2_TRACKER_WAYLAND=1` starts without XWayland, `PS2_TRACKER_NO_OSD=1` without the
on-screen display window type – both only for narrowing down problems.

## Command line

```
ps2-tracker --help
```

## Development

```bash
pnpm install
pnpm tauri dev                      # app with hot reload
pnpm build && cargo test --workspace
pnpm check && pnpm test
cargo clippy --workspace --all-targets -- -D warnings
node tools/contrast.mjs              # WCAG 2.1 AA contrast of the accent colours (src/lib/theme.ts)
PS2_SERVICE_ID=<id> cargo run -p ps2-tracker-core --example census_probe -- <character>
PS2_SERVICE_ID=<id> cargo run -p ps2-tracker-core --example ess_probe -- 10 20
```

Structure:

| Part | Content |
|---|---|
| `crates/ps2-tracker-core` | no GUI: Census client (rate limit, retry), ESS client (reconnect, backoff), event model, session logic, tracker, achievements, voice (facts, triggers, selection, queue), pack loader (open/Recursion), crosshair library, SQLite |
| `src-tauri` | Tauri app: commands, audio (rodio), windows, tray, hotkeys, OBS endpoint, window focus (XWayland) |
| `src/` | Svelte 5 user interface (main UI, overlay, crosshair); texts in `src/lib/i18n/en.ts` |

Security: Tauri capabilities minimal per window (overlay/crosshair read-only), CSP
without remote content, Census read-only, no telemetry. `gitleaks` runs as a
pre-commit hook: `git config core.hooksPath .githooks`.

Test data: real ESS events (public game data) in
`tests/fixtures/ess_real_2026-11-20.jsonl`; audio and Recursion fixtures are
generated by the project (`tools/gen_test_ogg.py`).

## Licence

At your option **MIT** or **Apache-2.0** (`MIT OR Apache-2.0`), see
[`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE). Own assets
(default tones, crosshairs, icon, test fixtures, screenshots) are CC0, see
[`assets/LICENSES.md`](assets/LICENSES.md). Licences of the dependencies:
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) (also included in every release;
regenerate with `python3 -I tools/third_party_notices.py`), checked in the CI with
`cargo deny check licenses`.

Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md), security issues:
[`SECURITY.md`](SECURITY.md), code of conduct:
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## Disclaimer and trademarks

This project is an independent fan project. It is **not** affiliated with,
endorsed or reviewed by Daybreak Game Company, Rogue Planet Games, PlanetSide 2 or
Recursion Tracker. PlanetSide and PlanetSide 2 are trademarks of Daybreak Game
Company LLC.
“Recursion” is used only as a statement of compatibility: the app reads the file
format of Recursion voice packs with own, independently written readers; **no**
Recursion files, sounds or packs are included. Use at your own risk, without
warranty (see the licence).
