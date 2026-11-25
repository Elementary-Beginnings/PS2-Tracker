# Hotkeys

## Global hotkeys

Defaults, changeable on the Overlay and Crosshair pages:

| Action | Default |
|---|---|
| Overlay on/off | Ctrl+Shift+F10 |
| Overlay move mode on/off | Ctrl+F2 |
| Finish move mode (save) | Ctrl+Esc; on Windows Ctrl+Shift+F2 (Ctrl+Alt+F2 if taken) |
| Crosshair on/off | Ctrl+Shift+F11 |
| Next crosshair | Ctrl+Shift+F12 |

Inside the move layer: Tab, arrow keys, Shift+arrows, Enter, Esc – see
[Overlay and Move Mode](Overlay-and-Move-Mode#move-layer-ctrlf2).

If a key cannot be registered (e.g. KWin binds Ctrl+F2 to “Switch to Desktop
2”), the log shows `hotkey registered failed hotkey=…`, and Settings, the
Overlay page and the move layer warn. Free the key in the desktop settings or
choose another one. Tray menu, buttons and `--finish-move` always work.

Global hotkeys work through X11/XWayland and, on Windows, `RegisterHotKey`.
In a Wayland session without XWayland (no `DISPLAY`) the app does not try to
register them: Settings and the Overlay page list them as not registered with
the reason, and the log says `hotkey registered failed … no X11 display`. Use
desktop shortcuts with the command line switches below instead.

## Desktop shortcuts (Wayland)

Without XWayland, bind the commands to desktop shortcuts. KDE Plasma: *System
Settings → Shortcuts → Custom Shortcuts*; GNOME: *Settings → Keyboard → Custom
Shortcuts*. The running instance takes over the command:

| Action | Command |
|---|---|
| Overlay on/off | `ps2-tracker --toggle-overlay` |
| Overlay move mode on/off | `ps2-tracker --toggle-edit` |
| Finish move mode | `ps2-tracker --finish-move` |
| Crosshair on/off | `ps2-tracker --toggle-crosshair` |
| Next crosshair | `ps2-tracker --next-crosshair` |

Flatpak: `flatpak run dev.herz.Ps2Tracker --toggle-crosshair` and so on.

## Command line

```
PS2 Session Tracker - session stats, overlay, crosshair and voice packs for PlanetSide 2.
Uses only the public Daybreak Census API; it does not touch the game process.

Usage: ps2-tracker [OPTION]

Options:
  --hidden             start minimized to the tray
  --show               show the main window
  --toggle-overlay     show/hide the overlay
  --toggle-edit        start/end the overlay move mode
  --finish-move        end the move mode and save (does nothing outside it)
  --toggle-crosshair   show/hide the crosshair
  --next-crosshair     switch to the next crosshair in the library
  --debug-console      debug mode: log level debug, log in a console window
                       (Windows) or in this terminal
  -V, --version        show the version
  -h, --help           show this help

If the app is already running, the option is forwarded to the running instance.
Bind these commands to KDE shortcuts for hotkeys under Wayland.

Environment:
  PS2_SERVICE_ID       Census service ID (overrides the configuration file)
  RUST_LOG             log level, e.g. info or debug
```

`--hidden` starts in the tray; where no tray icon can be shown (GNOME without
an AppIndicator extension) it opens the window instead.
