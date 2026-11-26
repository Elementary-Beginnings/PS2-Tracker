# Troubleshooting and FAQ

## Log and debug mode

The app writes its log to `~/.local/state/ps2-tracker/ps2-tracker.log`
(`$XDG_STATE_HOME`; the previous run stays as `ps2-tracker.log.1`), on Windows
under `%APPDATA%\ps2-tracker\`. The path is also under *Settings → Advanced*.
Detailed logging:

```bash
pkill ps2-tracker                       # end the running instance
ps2-tracker --debug-console             # debug mode, log also in the terminal
# or: RUST_LOG=ps2_tracker=debug,ps2_tracker_core=debug ps2-tracker
# then attach ~/.local/state/ps2-tracker/ps2-tracker.log to the issue
```

**Debug mode** (`--debug-console` or *Settings → Advanced → Show debug console
on start*, off by default): log level debug. On Linux the log also appears in
the terminal the app was started from.

For a bug report, include the version line (*Settings → Advanced → About*, a
click copies it) and the log. Security problems: see [Security](Security), not
a public issue.

### Windows

The release build starts without a console window. Started from a command
prompt or PowerShell, it writes `--help`, `--version` and the messages of
`--toggle-*`/`--finish-move` to that window. For the debug mode, create a
shortcut to `ps2-tracker.exe` and append ` --debug-console` under *Target*, or
set the setting above and restart: a console window “PS2 Session Tracker –
debug log” then opens with the log.

### What the log contains

- Start: version, session type, `WAYLAND_DISPLAY`/`DISPLAY`/`GDK_BACKEND`, the
  window backend, monitors with scaling.
- Sound: audio host, default and all output devices, the opened device; per
  game event the chosen sound or the reason why none plays; queue events;
  decoding and stream errors. A stalled output stream is detected
  (`audio output stalled`) and the device opened again.
- Sounds: per event the key chain checked
  (`voice: kill: tried DOUBLE_KILL(miss) … PER_KILL(hit, Pack: file.ogg)`).
- Crosshair: every visibility change with its reason, monitor/position,
  detected focus state, window type; per hit feedback effect its delay.
- Import: the reason of a rejection in the user interface; `session imported`
  in the log.

## No sound

The sound preview (▶) reports errors in the user interface (file missing,
codec not supported, no output device). Supported are Ogg Vorbis, MP3, FLAC and
WAV; Ogg Opus is rejected with a notice. Under PipeWire the app needs
`pipewire-alsa`: with *System default (PipeWire)* it first opens the ALSA device
`pipewire` and only then the ALSA default device (where `dmix` occasionally
fails with “unable to open slave”). The opened device is in the log
(`audio output opened`).

## Grey or empty window (NVIDIA)

With the proprietary NVIDIA driver the WebKitGTK window often stays grey or
empty. If the app detects the NVIDIA driver (`/sys/module/nvidia` or
`/proc/driver/nvidia/version`), it sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` and
restarts itself – the workaround the Tauri documentation describes
(<https://v2.tauri.app/develop/debug/linux-graphics/>), only for NVIDIA. The log
line `graphics environment` shows the driver and the origin of the variable.
Switch off: `PS2_TRACKER_NO_DMABUF_FIX=1 ps2-tracker` or `[ui]`
`webkit_dmabuf_workaround = false` in `config.toml`. A value you set yourself
is kept.

## Overlay much darker than its opacity setting (NVIDIA)

Up to 1.2.3 the stats overlay could look almost opaque although its
background opacity was set low, as if the panel lay two or three times on top
of itself (seen with KDE Plasma/KWin, XWayland and the NVIDIA driver). The
overlay draws its background once (one panel, the sections inside have none),
so the likely cause is the same as for the pop-up below: with the DMA-BUF
renderer off, old frames of the transparent window stay below the new one.
The overlay repaints every second, so the half-transparent panel adds up
(70 % becomes about 91 % after two frames and 97 % after three). Since 1.2.4
the app also sets `WEBKIT_DISABLE_COMPOSITING_MODE=1` for the NVIDIA driver:
WebKitGTK then paints without accelerated compositing. The log line
`graphics environment` shows `compositing_source`. Switch off:
`PS2_TRACKER_NO_COMPOSITING_FIX=1 ps2-tracker` or `[ui]`
`webkit_compositing_workaround = false` in `config.toml`. A value you set
yourself is kept. If the overlay still gets darker than its setting, please
open a bug report with the log.

## Overlay or crosshair disappears in fullscreen

- **KDE Plasma:** the app runs through XWayland and uses the “on-screen
  display” window type; import the KWin window rule. Details and switches
  (`PS2_TRACKER_WAYLAND=1`, `PS2_TRACKER_NO_OSD=1`, only for narrowing down
  problems): [Overlay and Move Mode](Overlay-and-Move-Mode#kde-plasma-wayland).
- **Windows:** use *Windowed Fullscreen* or *Borderless* in PlanetSide 2, see
  [Installation](Installation#windows-1011).

## Pop-up stays on screen

Up to 1.1.0 the achievement/voice pack pop-up could stay visible as a still
image after it had faded out (seen with KDE Plasma/KWin, XWayland and the
NVIDIA driver): the empty transparent window stayed mapped, and the
compositor kept its last frame; every focus change to the game showed it
again. Since 1.1.1 the pop-up window is only mapped while a pop-up shows and
is hidden after its fade-out; the focus watcher no longer maps it in between.
In the move mode (Ctrl+F2) the pop-up is still shown for positioning. If it
still happens, the log line `popup placed … reason="pop-up start"` (debug
mode) and the next `hidden: no pop-up active` help to narrow it down.

## No tray icon (GNOME)

GNOME Shell shows tray icons only with an extension such as *AppIndicator and
KStatusNotifierItem Support* (preinstalled on Ubuntu, not on Fedora or plain
GNOME). Without it the app detects that no tray host is running: closing the
main window then quits the app instead of hiding it, and *Start in the tray*
opens the window. With the extension installed, the tray works as on other
desktops. Since 1.2.2 a missing tray library (`libayatana-appindicator3`) no
longer stops the app from starting; the log says `tray icon not available`.

## Overlay on GNOME

Overlay and crosshair are tested on KDE Plasma (X11 and Wayland through
XWayland) and Windows. On GNOME they also run through XWayland; whether GNOME
keeps them above a fullscreen game has not been verified. If they disappear,
set PlanetSide 2 to *Windowed Fullscreen* or *Borderless* and report it with
the log (debug mode).

## Hotkeys under Wayland

Global hotkeys need an X server (XWayland). In a Wayland session without
XWayland the Overlay page lists them as not registered; bind the command line
switches (`ps2-tracker --toggle-overlay` and others) to desktop shortcuts, see
[Hotkeys](Hotkeys).

## Online sync: sign-in opens File Explorer (Windows)

Version 1.2.0 opened a File Explorer window instead of the browser on Windows
when you clicked *Sign in with Discord* or *Privacy policy*. Update to 1.2.1 or
newer; links then open in the default browser. Other sign-in problems (no
keyring, link not confirmed yet): see [Online Sync](Online-Sync#sign-in-and-keyring).

## Voice pack made on Windows does not load (Linux)

Up to 1.2.1 a pack whose `voicepack.toml` was written on Windows could fail to
load on Linux: backslashes in file paths, or file names that differ in
upper/lower case from the files. Since 1.2.2 such packs load.

## Ctrl+F2 does nothing

KWin binds Ctrl+F2 to “Switch to Desktop 2” by default. Free it or choose
another key, see [Hotkeys](Hotkeys).

## FAQ

**Can this get me banned?** The app never touches the game process: no
injection, hooks, memory reading or Vulkan layer. All data comes from the
public Census API; overlay and crosshair are separate windows, like any other
desktop program. What it accesses: [Permissions and Privacy](Permissions-and-Privacy).

**Why are there no hit markers for every hit?** The Census API provides no hit
or damage events for infantry, see [Crosshair](Crosshair#hit-feedback).

**Do I need my own Census service ID?** Not with official builds, see
[Getting Started](Getting-Started#census-service-id).

**The CSV export opens in one column in Excel.** Excel uses the list
separator of the Windows region (`;` in many European languages) and reads a
CSV without byte order mark as ANSI. Use the Excel export (.xlsx), or import
the CSV through *Data → From Text/CSV* with comma and UTF-8.

**Why does Windows SmartScreen warn?** The installer is not code-signed;
compare the checksum, see [Installation](Installation#windows-1011).
