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

## KDE shortcuts (Wayland)

Without XWayland, create commands under *System Settings → Shortcuts → Custom
Shortcuts*; the running instance takes over the command:

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
Usage: ps2-tracker [OPTION]

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

Environment:
  PS2_SERVICE_ID       Census service ID (overrides the configuration file)
  RUST_LOG             log level, e.g. info or debug
```

If the app is already running, the option is forwarded to the running instance.
