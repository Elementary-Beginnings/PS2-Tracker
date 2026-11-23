# PS2 Session Tracker

[English](README.md) · **Deutsch**

[![CI](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml/badge.svg)](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Elementary-Beginnings/ps2-tracker/badge)](https://scorecard.dev/viewer/?uri=github.com/Elementary-Beginnings/ps2-tracker)

<table>
  <tr>
    <td><img src="docs/screenshots/live-nc.png" alt="Live-Session mit Killfeed" width="100%"></td>
    <td><img src="docs/screenshots/overlay-styles.png" alt="Overlay-Stile über dem Spiel" width="100%"></td>
  </tr>
</table>

Session-Statistik, Overlay, Crosshair und Voice-Packs für **PlanetSide 2** – für
Linux (CachyOS/Arch, AppImage, Flatpak) und Windows. Der Tracker verfolgt deine
Charaktere live, zeigt Kills, Tode, K/D, Headshots und Klassen in der App und in
einem Overlay über dem Spiel, spielt Voice-Packs ab (auch Recursion-Packs) und
zeichnet ein pixelgenaues Crosshair. **Er greift nicht ins Spiel ein:** keine
Injection, keine Hooks, kein Speicherlesen – alle Daten kommen aus der
öffentlichen Daybreak Census API, Overlay und Crosshair sind eigene Fenster.

Die Anleitungen im [Wiki](https://github.com/Elementary-Beginnings/ps2-tracker/wiki)
und die Oberfläche der App sind englisch.

## Funktionen

- **Live-Sessions** mit Killfeed, Klassen, Lifetime-Vergleich und Erfolgen; nach Logout oder Spielabsturz läuft die Session weiter – [Sessions, Analytics and Export](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Sessions-Analytics-and-Export)
- **Verlauf und Auswertung** mit Export/Import (Excel, JSON, CSV) – [Sessions, Analytics and Export](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Sessions-Analytics-and-Export#analytics)
- **Overlay** über dem Spiel, direkt an Ort und Stelle verschiebbar (Strg+F2), OBS-Browserquelle – [Overlay and Move Mode](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Overlay-and-Move-Mode)
- **Crosshair**-Fenster mit eingebauten und eigenen PNGs und Treffer-Feedback – [Crosshair](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Crosshair)
- **Voice-Packs**: offenes Format und Recursion-`.rtst_vpk` – [Voice Packs](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Voice-Packs)
- **Squad und Outfit** mitverfolgen, „Squad top 3“ im Overlay – [Squad and Outfit Tracking](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Squad-and-Outfit-Tracking)
- **Discord Rich Presence** (optional) – [Discord Rich Presence](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Discord-Rich-Presence)
- **Signierte Updates in der App** (opt-in) – [Auto-Update](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Auto-Update)
- **Globale Hotkeys** und Kommandozeile – [Hotkeys](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Hotkeys)
- **Keine Telemetrie**; jede Verbindung und jeder Dateizugriff ist dokumentiert – [Permissions and Privacy](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Permissions-and-Privacy)

## Download

Aus dem [neuesten Release](https://github.com/Elementary-Beginnings/ps2-tracker/releases/latest):

- **CachyOS / Arch:** `ps2-tracker-x86_64.pkg.tar.zst` (AUR-Dateien `ps2-tracker-bin.*` liegen bei)
- **Linux (alle):** `ps2-tracker-x86_64.AppImage` oder `ps2-tracker-x86_64-linux.tar.gz`
- **Flatpak:** `ps2-tracker.flatpak`
- **Windows 10/11:** `ps2-tracker-x64-setup.exe` (nicht codesigniert, SmartScreen warnt)

Prüfen mit `sha256sum -c SHA256SUMS --ignore-missing` und optional
`gh attestation verify <Datei> --repo Elementary-Beginnings/ps2-tracker`.
Einzelheiten je Plattform: [Installation](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Installation).

## Schnellstart

1. Paket für die Plattform installieren und **PS2 Session Tracker** starten.
2. Im Einrichtungsassistenten den Charakter suchen und hinzufügen (offizielle
   Builds brauchen keine eigene Census-Service-ID).
3. In PlanetSide 2 einloggen – die Session startet von selbst; mit **Strg+F2**
   das Overlay platzieren.

Mehr: [Getting Started](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Getting-Started),
[Troubleshooting and FAQ](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Troubleshooting-and-FAQ).

## Links

- [Wiki](https://github.com/Elementary-Beginnings/ps2-tracker/wiki) – Anleitungen
  (Quelle in [`docs/wiki/`](docs/wiki/Home.md))
- [Sicherheitsrichtlinie](SECURITY.md) – Schwachstellen vertraulich melden
- [Mitwirken](CONTRIBUTING.md) · [Verhaltenskodex](CODE_OF_CONDUCT.md) ·
  [Building from Source](https://github.com/Elementary-Beginnings/ps2-tracker/wiki/Building-from-Source)
- [Changelog](CHANGELOG.md)

## Lizenz

Nach Wahl **MIT** oder **Apache-2.0** (`MIT OR Apache-2.0`), siehe
[`LICENSE-MIT`](LICENSE-MIT) und [`LICENSE-APACHE`](LICENSE-APACHE). Eigene
Assets (Standardtöne, Crosshairs, Icon, Testdaten, Screenshots) sind CC0, siehe
[`assets/LICENSES.md`](assets/LICENSES.md); Lizenzen der Abhängigkeiten:
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

Eigenentwicklung (Clean Room): kein Code, keine Grafiken oder Sounds eines
anderen Trackers übernommen; das Recursion-Voice-Pack-Format wurde
ausschließlich anhand eines eigenen Packs erschlossen.

## Haftungsausschluss und Marken

Dieses Projekt ist ein unabhängiges Fanprojekt. Es ist **nicht** mit Daybreak
Game Company, Rogue Planet Games, PlanetSide 2 oder Recursion Tracker verbunden,
von ihnen unterstützt oder geprüft. PlanetSide und PlanetSide 2 sind Marken der
Daybreak Game Company LLC. „Recursion“ wird nur als Kompatibilitätsangabe
verwendet: Die App liest das Dateiformat von Recursion-Voice-Packs mit eigenen,
unabhängig geschriebenen Readern; Recursion-Dateien, -Sounds oder -Packs sind
**nicht** enthalten. Nutzung auf eigene Gefahr, ohne Gewährleistung (siehe
Lizenz).
