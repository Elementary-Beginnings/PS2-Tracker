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

## Overlay or crosshair disappears in fullscreen

- **KDE Plasma:** the app runs through XWayland and uses the “on-screen
  display” window type; import the KWin window rule. Details and switches
  (`PS2_TRACKER_WAYLAND=1`, `PS2_TRACKER_NO_OSD=1`, only for narrowing down
  problems): [Overlay and Move Mode](Overlay-and-Move-Mode#kde-plasma-wayland).
- **Windows:** use *Windowed Fullscreen* or *Borderless* in PlanetSide 2, see
  [Installation](Installation#windows-1011).

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

**Why does Windows SmartScreen warn?** The installer is not code-signed;
compare the checksum, see [Installation](Installation#windows-1011).
