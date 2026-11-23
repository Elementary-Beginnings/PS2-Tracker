# PS2 Session Tracker

**English** · [Deutsch](README.de.md)

[![CI](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml/badge.svg)](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Elementary-Beginnings/ps2-tracker/badge)](https://scorecard.dev/viewer/?uri=github.com/Elementary-Beginnings/ps2-tracker)

<table>
  <tr>
    <td><img src="docs/screenshots/live-nc.png" alt="Live session with kill feed" width="100%"></td>
    <td><img src="docs/screenshots/overlay-styles.png" alt="Overlay styles over the game" width="100%"></td>
  </tr>
</table>

Session stats, overlay, crosshair and voice packs for **PlanetSide 2** – for
Linux (CachyOS/Arch, AppImage, Flatpak) and Windows. The tracker follows your
characters live, shows kills, deaths, K/D, headshots and classes in the app and
in an in-game overlay, plays voice packs (also Recursion packs) and draws a
pixel-exact crosshair. **It does not touch the game:** no injection, no hooks,
no memory reading – all data comes from the public Daybreak Census API, and
overlay and crosshair are separate windows.

## Features

- **Live sessions** with kill feed, classes, lifetime comparison and
  achievements; a session resumes after a logout or game crash – [Sessions, Analytics and Export](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Sessions-Analytics-and-Export)
- **History and analytics** with Excel/JSON/CSV export and import – [Sessions, Analytics and Export](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Sessions-Analytics-and-Export#analytics)
- **Overlay** over the game, moved directly in place (Ctrl+F2), OBS browser source – [Overlay and Move Mode](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Overlay-and-Move-Mode)
- **Crosshair** window with built-in and own PNGs and hit feedback – [Crosshair](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Crosshair)
- **Voice packs**: open format and Recursion `.rtst_vpk` – [Voice Packs](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Voice-Packs)
- **Squad and outfit** tracking with “Squad top 3” in the overlay – [Squad and Outfit Tracking](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Squad-and-Outfit-Tracking)
- **Discord Rich Presence** (optional) – [Discord Rich Presence](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Discord-Rich-Presence)
- **Online sync** for ELME members (opt-in, off by default): finished sessions to stats.elme.games, sign-in with Discord, token in the system keyring – [Online Sync](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Online-Sync)
- **Signed in-app updates** (opt-in) – [Auto-Update](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Auto-Update)
- **Global hotkeys** and command line – [Hotkeys](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Hotkeys)
- **No telemetry**; every connection and file access is documented – [Permissions and Privacy](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Permissions-and-Privacy)

## Download

From the [latest release](https://github.com/Elementary-Beginnings/ps2-tracker/releases/latest):

- **CachyOS / Arch:** `ps2-tracker-x86_64.pkg.tar.zst` (AUR files `ps2-tracker-bin.*` included)
- **Linux (any):** `ps2-tracker-x86_64.AppImage` or `ps2-tracker-x86_64-linux.tar.gz`
- **Flatpak:** `ps2-tracker.flatpak`
- **Windows 10/11:** `ps2-tracker-x64-setup.exe` (not code-signed, SmartScreen warns)

Check with `sha256sum -c SHA256SUMS --ignore-missing` and optionally
`gh attestation verify <file> --repo Elementary-Beginnings/ps2-tracker`.
Details per platform: [Installation](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Installation).

## Quick start

1. Install the package for your platform and start **PS2 Session Tracker**.
2. In the first-start assistant, search and add your character (official
   builds need no Census service ID of your own).
3. Log in to PlanetSide 2 – the session starts by itself; press **Ctrl+F2** to
   place the overlay.

More: [Getting Started](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Getting-Started),
[Troubleshooting and FAQ](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Troubleshooting-and-FAQ).

## Links

- [Wiki](https://github.com/Elementary-Beginnings/ps2-tracker/wiki) – guides
  (source in [`docs/wiki/`](docs/wiki/Home.md))
- [Security policy](SECURITY.md) – report vulnerabilities privately
- [Contributing](CONTRIBUTING.md) · [Code of conduct](CODE_OF_CONDUCT.md) ·
  [Building from source](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Building-from-Source)
- [Changelog](CHANGELOG.md)

## Licence

At your option **MIT** or **Apache-2.0** (`MIT OR Apache-2.0`), see
[`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE). Own assets
(default tones, crosshairs, icon, test fixtures, screenshots) are CC0, see
[`assets/LICENSES.md`](assets/LICENSES.md); licences of the dependencies:
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

Own development (clean room): no code, graphics or sounds of another tracker
were taken; the Recursion voice pack format was worked out solely from an own
pack.

## Disclaimer and trademarks

This project is an independent fan project. It is **not** affiliated with,
endorsed or reviewed by Daybreak Game Company, Rogue Planet Games, PlanetSide 2
or Recursion Tracker. PlanetSide and PlanetSide 2 are trademarks of Daybreak
Game Company LLC. “Recursion” is used only as a statement of compatibility: the
app reads the file format of Recursion voice packs with own, independently
written readers; **no** Recursion files, sounds or packs are included. Use at
your own risk, without warranty (see the licence).
