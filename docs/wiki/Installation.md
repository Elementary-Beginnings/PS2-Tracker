# Installation

Every release on the
[releases page](https://github.com/Elementary-Beginnings/ps2-tracker/releases)
carries all files, `SHA256SUMS` and build provenance attestations (built by
GitHub Actions, `.github/workflows/release.yml`).

| Platform | File |
|---|---|
| CachyOS / Arch | `ps2-tracker-x86_64.pkg.tar.zst` |
| Arch (AUR) | `ps2-tracker-bin.PKGBUILD` / `.SRCINFO` as release files |
| Linux (any) | `ps2-tracker-x86_64.AppImage` |
| Linux (binary tarball) | `ps2-tracker-x86_64-linux.tar.gz` |
| Linux (Flatpak) | `ps2-tracker.flatpak` |
| Windows 10/11 | `ps2-tracker-x64-setup.exe` |
| Checksums | `SHA256SUMS` |

**Check before installing:** `sha256sum -c SHA256SUMS --ignore-missing`, and
optionally the provenance:
`gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`.

Official builds work out of the box with a built-in Census service ID; your own
ID takes precedence and can be entered at any time (see
[Getting Started](Getting-Started#census-service-id)). Builds from source
without one need your own ID; the first-start assistant asks for it.

## CachyOS / Arch

```bash
sudo pacman -U ps2-tracker-x86_64.pkg.tar.zst     # package from the release
# or build it yourself:
git clone https://github.com/Elementary-Beginnings/ps2-tracker.git
cd ps2-tracker/packaging/arch && makepkg -si
```

**AUR:** `ps2-tracker-bin` (e.g. `paru -S ps2-tracker-bin`) installs the
prebuilt binary of a release, once the package is published in the AUR; until
then use the package from the release or `makepkg` from the repository.

Dependencies (installed by `pacman`/`makepkg`): `webkit2gtk-4.1`, `gtk3`,
`libayatana-appindicator`, `alsa-lib`, `librsvg`; for building also `rust`,
`nodejs`, `pnpm`. Under PipeWire, `pipewire-alsa` is needed.

Autostart: `cp /usr/share/applications/ps2-tracker.desktop ~/.config/autostart/`
and set `Exec=ps2-tracker --hidden` there.

## AppImage (other distributions)

```bash
chmod +x ps2-tracker-x86_64.AppImage && ./ps2-tracker-x86_64.AppImage
```

The AppImage can update itself, see [Auto-Update](Auto-Update).

## Flatpak

```bash
flatpak install --user ps2-tracker.flatpak     # bundle from the release
flatpak run dev.herz.Ps2Tracker
```

The bundle needs the runtime `org.gnome.Platform//51` from Flathub (fetched
during installation when Flathub is set up). The permissions and their reasons
are in the manifest
[`packaging/flatpak/dev.herz.Ps2Tracker.yml`](https://github.com/Elementary-Beginnings/ps2-tracker/blob/main/packaging/flatpak/dev.herz.Ps2Tracker.yml):
X11 (overlay, crosshair and focus detection run through XWayland), Wayland,
PulseAudio, network, tray. No access to the home directory – packs and
crosshairs are imported through the file dialog of the portal. The data is
under `~/.var/app/dev.herz.Ps2Tracker/`. Global hotkeys go through X11 and work
in the sandbox with `--socket=x11`; under Wayland create KDE shortcuts as
described in [Hotkeys](Hotkeys) (`flatpak run dev.herz.Ps2Tracker --toggle-crosshair`).

## Windows 10/11

Run `ps2-tracker-x64-setup.exe` (installation for the current user, no
administrator rights). If WebView2 is missing, the installer downloads
Microsoft's bootstrapper (Windows 11 and current Windows 10 systems already
have WebView2).

- **Not signed:** there is no code signing certificate, so SmartScreen warns
  (“Windows protected your PC” → *More info* → *Run anyway*). Compare the
  checksum first: `Get-FileHash .\ps2-tracker-x64-setup.exe -Algorithm SHA256`
  against `SHA256SUMS`.
- **Overlay and crosshair only in windowed or borderless fullscreen mode:** in
  exclusive fullscreen the game owns the display alone; Microsoft describes
  that overlays can be composited “on top of the game in a safe and stable way”
  only in a borderless window or with fullscreen optimizations
  ([DirectX Developer Blog: Demystifying Full Screen Optimizations](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations)).
  In PlanetSide 2 choose *Windowed Fullscreen* or *Borderless*.
- Overlay and crosshair are borderless, transparent windows with
  `WS_EX_TOPMOST`, `WS_EX_LAYERED | WS_EX_TRANSPARENT` (click-through) and
  `WS_EX_NOACTIVATE` (take no focus).
- Focus detection: the foreground window and the file name of its process
  (`PlanetSide2_x64.exe`), opened only with
  `PROCESS_QUERY_LIMITED_INFORMATION`. It sees only the foreground window, so
  the move layer opens on the game's monitor only when the game is in front.
- Data: `%APPDATA%\ps2-tracker\` (configuration, database, packs, log).
- No console window at startup; see
  [Troubleshooting](Troubleshooting-and-FAQ#windows) for the debug console.
- Hotkeys: Ctrl+Esc is reserved by Windows (Start menu), so “Finish move mode”
  is **Ctrl+Shift+F2** there (or Ctrl+Alt+F2 if that is taken), see
  [Hotkeys](Hotkeys).
- The move layer reads with `GetClipCursor` whether the game confines the
  mouse pointer and then shows “The game holds the cursor” as on Linux. The
  Win32 calls without a safe wrapper are in `src-tauri/src/win_console.rs`, the
  only module with `unsafe` code.
