# PS2 Session Tracker

[English](README.md) · **Deutsch**

[![CI](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml/badge.svg)](https://github.com/Elementary-Beginnings/ps2-tracker/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Elementary-Beginnings/ps2-tracker/badge)](https://scorecard.dev/viewer/?uri=github.com/Elementary-Beginnings/ps2-tracker)

Session-Statistik, Overlay, Crosshair und Voice-Packs für **PlanetSide 2** – für
Linux (CachyOS/Arch, AppImage, Flatpak) und Windows. Arbeitstitel `ps2-tracker`.

> **Greift nicht ins Spiel ein.** Keine Injection, keine Hooks, kein Speicherlesen,
> kein Vulkan-Layer. Alle Spieldaten kommen ausschließlich aus der öffentlichen
> **Daybreak Census API** (REST und Event Streaming per WebSocket). Overlay und
> Crosshair sind eigene, externe Fenster; der Tracker läuft wie jedes andere
> Desktop-Programm neben dem Spiel.

<table>
  <tr>
    <td><img src="docs/screenshots/live-nc.png" alt="Live-Session (NC)" width="100%"><br><sub>Live-Session mit Killfeed (Kill grün, Tod orange, HS/HS taken), Akzent NC</sub></td>
    <td><img src="docs/screenshots/live-light.png" alt="Light Mode" width="100%"><br><sub>Light Mode (TR)</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/analytics.png" alt="Analytics" width="100%"><br><sub>Analytics: Trends, Waffen, Verlauf, Fraktionen, A vs. B</sub></td>
    <td><img src="docs/screenshots/history.png" alt="History" width="100%"><br><sub>History mit Import und Export (Excel, JSON, CSV)</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/overlay-styles.png" alt="Overlay-Stile" width="100%"><br><sub>Overlay-Stile Card, Slim, Text only über heller und dunkler Szene</sub></td>
    <td><img src="docs/screenshots/overlay.png" alt="Overlay-Seite" width="100%"><br><sub>Overlay-Seite mit Presets, Größe und Position</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/crosshair.png" alt="Crosshair" width="100%"><br><sub>Crosshair, pixelgenau</sub></td>
    <td><img src="docs/screenshots/hit-feedback.png" alt="Hit feedback" width="100%"><br><sub>Hit feedback: Spitzenbild je Ereignis</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/sounds-events.png" alt="Ereignisse" width="100%"><br><sub>Ereignisse: Basis-Ereignisse mit Schlüsselkette, darunter Achievements</sub></td>
    <td><img src="docs/screenshots/trigger-editor.png" alt="Bedingung bearbeiten" width="100%"><br><sub>Bedingung eines Ereignisses in der Oberfläche</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/squad.png" alt="Squad" width="100%"><br><sub>Squad: Live-Werte von Freunden/Outfit mit Server und Kontinent, eigene Zeile hervorgehoben</sub></td>
    <td><img src="docs/screenshots/integrations.png" alt="Updates und Discord" width="100%"><br><sub>Updates (opt-in) und Discord Rich Presence</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/sounds-packs.png" alt="Voice-Packs" width="100%"><br><sub>Voice-Packs (offenes Format, drei Recursion-Formate)</sub></td>
    <td><img src="docs/screenshots/setup-assistant.png" alt="Erststart-Assistent" width="100%"><br><sub>Erststart-Assistent</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/overlay-squad.png" alt="Overlay mit Squad top 3" width="100%"><br><sub>Overlay-Baustein „Squad top 3“, eigene Zeile markiert, leere Plätze als –</sub></td>
    <td><img src="docs/screenshots/overlay-classes.png" alt="Overlay mit Klassensymbolen" width="100%"><br><sub>Klassen im Killfeed (eigene › gegnerische) und in „Squad top 3“</sub></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/live-classes.png" alt="Live-Seite mit Klassen-Auswertung" width="100%"><br><sub>Live-Seite: Kills/Tode nach Gegnerklasse, eigene Klassen der Session</sub></td>
    <td></td>
  </tr>
</table>

Die Bilder zeigen die Browser-Vorschau mit synthetischen Beispieldaten
(„DemoSoldier…“, keine echten Charaktere oder IDs); neu erzeugt mit
`pnpm build` und einem Headless-Browser auf `vite preview` (Parameter wie
`?page=analytics&theme=light` stehen in `src/lib/demo.ts`).

## Downloads

Jeder Tag `v*` erzeugt ein [GitHub-Release](https://github.com/Elementary-Beginnings/ps2-tracker/releases) mit allen Dateien,
`SHA256SUMS` und Herkunftsnachweisen (Build Provenance) aus GitHub Actions
(`.github/workflows/release.yml`).

| Plattform | Datei |
|---|---|
| CachyOS / Arch | `ps2-tracker-x86_64.pkg.tar.zst` |
| Arch (AUR) | `ps2-tracker-bin.PKGBUILD` / `.SRCINFO` als Release-Dateien, siehe [docs/release.md](docs/release.md) |
| Linux (beliebig) | `ps2-tracker-x86_64.AppImage` |
| Linux (Binary-Tarball) | `ps2-tracker-x86_64-linux.tar.gz` |
| Linux (Flatpak) | `ps2-tracker.flatpak` |
| Windows 10/11 | `ps2-tracker-x64-setup.exe` |
| Prüfsummen | `SHA256SUMS` |

Vor der Installation prüfen: `sha256sum -c SHA256SUMS --ignore-missing`, optional
die Herkunft: `gh attestation verify <Datei> --repo Elementary-Beginnings/ps2-tracker`.

Eigenentwicklung (Clean Room). Funktionen sind aus öffentlichen Beschreibungen
abgeleitet; es wurde kein Code, keine Grafik und kein Sound eines anderen Trackers
übernommen. Das Recursion-Voice-Pack-Format wurde ausschließlich anhand eines
eigenen Packs ermittelt (siehe [docs/voicepacks.md](docs/voicepacks.md)).

Oberfläche und Dokumentation sind englisch (diese README zusätzlich deutsch); Commit-Nachrichten sind deutsch.

## Datenschutz und Berechtigungen

Kurz: Die App verbindet sich nur mit `census.daybreakgames.com` und
`push.planetside2.com` – und nur nach Opt-in mit dem Update-Endpunkt
(GitHub-Releases) bzw. lokal mit Discord –, sendet keine Telemetrie, liest nur ihre eigenen Ordner und
die Dateien, die Sie importieren (Packs, Crosshairs, Sessions – immer über den
Dateidialog), schreibt Exporte nur an den gewählten Ort, schreibt die Service-ID nur in `config.toml`
(Rechte 0600) und greift nie auf das Spiel zu. Unter X11/XWayland liest sie
Fenstertitel und -klassen, um das Spielfenster zu erkennen. Alle Details mit den
Quellcode-Stellen und Anleitungen zum Selbstprüfen:
**[docs/permissions.md](docs/permissions.md)**.

## Funktionen

- **Live-Session**: Kills, Tode, K/D, KPM, Headshots/HSR, Revives, Assists, XP/h,
  Teamkills, Suizide, Fahrzeug-Kills, Killstreak und beste Streak, je Waffe Kills/HS.
- **K/D incl. revives** wie im Spiel: Tode, aus denen man wiederbelebt wurde
  (Experience 7 „Revive“, 53 „Squad Revive“), zählen nicht. Wahlweise als große
  K/D auf der Live-Seite (*Settings → Big K/D on the live page*), als Overlay-/
  OBS-Baustein und in allen Exporten; die True K/D bleibt daneben sichtbar.
  Einen Lifetime-Vergleich gibt es nur für die True K/D (Census führt keine
  um Revives bereinigte Lifetime-K/D).
- **Killfeed** in App, Overlay und OBS: Kill grün, Tod orange, Teamkill gelb,
  eigener Headshot „HS“ cyan, erlittener „HS taken“ korallenorange, immer mit
  Symbol und Text (Farbe ist nie die einzige Information, Kontrast per Test
  geprüft). Fehlende Namen werden nachgeladen; ohne Namen steht der Grund da
  (Yourself, Environment, Looking up…, Unknown player).
- **Klassen** (Infiltrator, Light Assault, Combat Medic, Engineer, Heavy Assault,
  MAX; NSO zählt zu denselben Klassen): aus `attacker_loadout_id`/
  `character_loadout_id` der Death-Ereignisse bzw. `loadout_id` von
  GainExperience. Im Killfeed (App, Overlay, OBS) steht vor dem Namen ein
  kleines, eigenes Klassensymbol (selbst gezeichnet, keine Spielgrafik) mit dem
  Klassennamen als Tooltip – beim Kill die Klasse des Opfers, beim Tod die des
  Angreifers; optional davor die eigene Klasse (*Overlay → Widgets → Show my
  class in the kill feed*). „Squad top 3“ und die Squad-Tabelle zeigen die
  aktuelle Klasse je Mitglied (zuletzt gesehenes Loadout, ohne Ereignis „–“).
  Die Live-Seite hat den aufklappbaren Bereich *Classes*: Kills und Tode nach
  Gegnerklasse und die eigenen Klassen der Session (Kills, Tode, ungefähre Zeit
  aus den Abständen zwischen Kills/Toden in derselben Klasse). Die Zuordnung
  Loadout → Klasse stammt aus der Census-Sammlung `loadout` (24 Einträge,
  `crates/ps2-tracker-core/data/loadouts.json`) und ist eingebaut – es gibt
  keine zusätzliche Abfrage; unbekannte IDs bleiben „–“. Sessions vor
  Datenversion 4 haben keine Klassen.
- **Vergleich mit Lifetime-Werten** (`characters_stat_history`,
  `characters_weapon_stat_by_faction`), auch je Waffe.
- **Mehrere Charaktere** (z. B. einer je Fraktion), automatischer Start beim Login
  bzw. ersten Ereignis, Ende bei Logout oder Inaktivität.
- **Sessions** in SQLite (Schema-Version 3, Session-Datenversion 4, alte Datenbanken werden beim Start
  migriert; eine beschädigte Datei wird beiseitegelegt und neu angelegt), Liste, Detailansicht, Export **Excel**/JSON/CSV, **Import** von
  JSON und Excel – siehe [Analytics, Export und Import](#analytics-export-und-import).
- **Analytics-Seite**: Sessions filtern und vergleichen, Diagramme mit PNG-Export.
- **Achievements** als eigenes TOML-Regelset (Multikill, Streak, Headshot-Serie,
  Rache, Fahrzeug-Kills …).
- **Overlay-Fenster**: transparent, rahmenlos, immer im Vordergrund, durchklickbar;
  Bausteine und Presets (Minimal/Streamer/Full), drei Stile – **Card**
  (empfohlen), **Slim** (flach, ohne Rahmen) und **Text only** (Kontur statt
  Panel) –, Größe S/M/L/XL oder automatisch nach Monitorhöhe. Zwei getrennt
  positionierbare Objekte: das Stats-Overlay und die Achievement-/Voice-Pack-
  Einblendung (das Pack-Bild unverändert mit seinem Alpha-Kanal, ohne Rahmen,
  Text mit Schatten, Ein-/Ausblenden, Dauer einstellbar).
- **Verschieben direkt über dem Spiel (Move-Ebene, Ctrl+F2)** – siehe
  [Overlay verschieben](#overlay-verschieben-move-ebene). Alternativ Anker, Abstand
  und „Reset position“ auf der Overlay-Seite; immer vollständig auf dem Monitor.
- **Crosshair-Fenster**: eigenes PNG, pixelgenau, exakt mittig, Skalierung,
  Deckkraft, Tönung, X/Y-Versatz, Monitor, Hotkeys; optional nur bei aktivem Spiel.
- **Mitgelieferte Crosshairs** (selbst erzeugt im Code, `crosshair.rs`, CC0,
  pixelgenau ohne Kantenglättung): Punkt, Kreuz, Kreis wie bisher, dazu
  `cross` in drei Größen (32, 24, 12 px; 2-px-Balken, die großen mit Lücke in
  der Mitte), `cross-dashed` und `dot`, jeweils in Grün, Magenta und Rot mit
  leichter dunkler Kontur, und die Entfernungsleiter `range-ladder` in Orange für
  1440p und 4K (Linie mit Punkten, Raute in der Mitte, darunter eine Leiter mit
  1–9; Raute exakt im Bildmittelpunkt, die Abstände sind eine Orientierung, nicht
  auf eine Waffe kalibriert). Die Kreuze mit 2-px-Balken haben eine gerade
  Kantenlänge, ihre Mitte liegt wie die Mitte eines Monitors mit gerader
  Auflösung zwischen zwei Pixeln. Neue Standard-Crosshairs kommen beim nächsten
  Start in die Bibliothek; vorhandene Dateien bleiben unverändert. Als Dateien:
  `cargo run -p ps2-tracker-core --example crosshair_dump -- <Ordner>`.
  **Eigene PNGs** – auch Vorlagen aus dem Netz – lassen sich jederzeit mit
  *Crosshair → Import PNG* hinzufügen (bis 512×512).
- **Hit feedback** im Crosshair (wie die Treffermarker in Recursion, mit
  ehrlichen Grenzen): siehe [Crosshair-Rückmeldung](#crosshair-rückmeldung-hit-feedback).
- **Sounds & Voice-Packs**: eigenes offenes Format und Recursion-Packs
  (`.rtst_vpk`, drei Formate), Prioritäts-Warteschlange, Ausgabegerät wählbar,
  Bedingungen je Ereignis in der Oberfläche änderbar – siehe
  [Voice-Packs](#voice-packs).
- **Squad/Outfit-Modus**: Freunde oder Outfit-Mitglieder (bis 20) mitverfolgen –
  Seite *Squad* mit Live-Werten je Charakter (K/D, KPM, HSR, Kills, Revives,
  Status), sortierbar, eigene Zeile hervorgehoben, Import aus einem Outfit (Tag
  oder Name, optional nur online), je Charakter gespeicherte Sessions und
  Löschfunktion, Spalte *Server / Continent* (letzter Server und Kontinent der
  laufenden Session, Namen über Census). Sounds, Einblendungen und Hit feedback
  gelten weiter nur für den eigenen aktiven Charakter.
- **Overlay-Baustein „Squad top 3“** (Overlay-Seite → Widgets, auch im
  OBS-Endpunkt): die drei besten Charaktere der laufenden Sessions (eigene und
  Squad) nach K/D, Kills oder KPM, mit Fraktionsabzeichen; die eigene Zeile ist
  markiert. Für K/D und KPM gilt eine Mindestzahl Kills (Standard 5): wer sie
  noch nicht hat, steht trotzdem in der Liste, aber hinter den anderen, nach
  Kills sortiert und mit gedimmtem Wert „Kills/Mindestzahl“ (Kopfzeile
  „min. 5 kills“). Squad-Mitglieder erscheinen auch ohne Ereignis; leere Plätze
  („–“) gibt es nur, wenn es weniger als drei Charaktere gibt. Der Baustein hat
  immer drei Zeilen, das Overlay behält also seine Größe; nur das Ein- und
  Ausschalten misst neu. Der Baustein ist standardmäßig aus; die Squad-Seite
  weist mit Link darauf hin. Ein neues oder entferntes Mitglied (auch per
  Outfit-Import) wird sofort auf der offenen ESS-Verbindung abonniert bzw.
  abbestellt (`ESS subscription updated added=… removed=…` im Log), Squad-Seite,
  Overlay und OBS zeigen es sofort.
- **Discord Rich Presence** (optional, Standard aus): Charakter/Fraktion, K/D,
  Kills, Session-Dauer, wahlweise Kontinent und Server im Discord-Status über die
  lokale Discord-IPC; braucht
  eine eigene Discord-Application-ID (*Settings → Updates & Discord*).
- **Updates in der App** (optional, Standard aus; AppImage und Windows):
  signierte Updates, erst nach Opt-in bzw. „Check now“ und Bestätigung; für
  Arch/AUR/Flatpak über den Paketmanager. Eingerichtet ist das erst, wenn der
  Maintainer den Signaturschlüssel hinterlegt hat ([docs/release.md](docs/release.md)).
- **OBS-Browser-Source** (optional, nur `127.0.0.1`).
- **Tray-Icon**, Single-Instance, keine Telemetrie.

## Overlay verschieben (Move-Ebene)

<table>
  <tr>
    <td><img src="docs/screenshots/move-layer.png" alt="Move-Ebene über dem Spiel" width="100%"><br><sub>Move-Ebene: beide Objekte in echter Größe, Leiste unten</sub></td>
    <td><img src="docs/screenshots/move-layer-grab.png" alt="Hinweis, wenn das Spiel den Zeiger hält" width="100%"><br><sub>Das Spiel hält den Zeiger: Esc im Spiel oder Tastatur</sub></td>
  </tr>
</table>

**Ctrl+F2** (oder `ps2-tracker --toggle-edit`, oder *Overlay → Move mode*) legt
eine abgedunkelte Ebene über den ganzen Monitor des Spiels. Darin liegen das
Stats-Overlay und die Einblendung als Rahmen in echter Größe und an ihrer echten
Position, mit dem echten Inhalt.

- **Ziehen** mit der Maus; die Objekte rasten an Monitorrand und -mitte und am
  jeweils anderen Objekt ein (**Alt** gedrückt: frei), optional ein 10-px-Raster.
  Unter dem Rahmen steht die Position in Pixeln.
- **Tastatur** (funktioniert immer, auch ohne Maus): **Tab** wechselt das Objekt,
  **Pfeiltasten** 1 px, **Shift+Pfeile** 10 px, **Enter** oder **Ctrl+F2**
  speichert, **Esc** verwirft und lässt die alte Position stehen. Die Leiste
  unten hat dazu *Save*, *Cancel*, *Reset* (Standardecke) und *Move to monitor…*.
- **Beenden geht immer**: **Ctrl+Esc**, unter Windows **Ctrl+Shift+F2** (eigene
  Taste „Finish move mode“, auf der Overlay-Seite änderbar) speichert und beendet – als globaler Hotkey für die
  ganze Move-Session, auch während das Spiel den Zeiger hält. Außerdem: Tray-Menü
  *Finish move mode*, der Knopf auf der Overlay-Seite bzw. das Banner oben im
  Hauptfenster, `ps2-tracker --finish-move` (oder `--toggle-edit`) und nach
  5 Minuten ohne Eingabe von selbst (speichert, mit Hinweis im Hauptfenster;
  auf der Overlay-Seite einstellbar oder abschaltbar). Verwerfen nur über
  *Cancel* bzw. Esc ohne Zeiger-Grab.
- **Hält das Spiel den Mauszeiger** (Maus-Look), zeigt die Ebene groß *„The game
  holds the cursor“*: **im Spiel Esc drücken** (öffnet das Spielmenü und gibt den
  Zeiger frei) oder mit Tab/Pfeiltasten arbeiten. Solange das Spiel den Zeiger
  hält, ist Esc kein Hotkey der App, damit es im Spiel ankommt; danach heißt Esc
  wieder „Cancel“.
- Erst beim Speichern werden die echten Fenster **einmal** an die neue Position
  gesetzt (mit Positionsprüfung im Log). Gespeichert wird wie bisher relativ zum
  Monitor (Ecke + Abstand).

Technik: Die Ebene ist wie das Crosshair ein Fenster vom Typ „On-Screen-Display“
und liegt damit unter KWin über dem Vollbildspiel. Gezogen wird innerhalb der
Ebene, nicht über den Fenstermanager: KWin verschiebt OSD-Fenster grundsätzlich
nicht (`X11Window::isMovable()` ist für solche Fenster falsch). Den Tastaturfokus
fordert die Ebene mit `_NET_ACTIVE_WINDOW` an; ob das Spiel den Zeiger hält,
erkennt sie an einem kurzen eigenen `XGrabPointer`-Versuch (Antwort
„AlreadyGrabbed“). Im Log: `move layer opened monitor=… focus=granted|denied
pointer=free|grabbed`, `move layer saved stats=… popup=…` bzw. `move layer
cancelled`, dann `move layer finished via=hotkey|tray|button|cli|timeout`.

**KDE/KWin und Ctrl+F2:** KWin belegt Ctrl+F2 oft mit „Zu Arbeitsfläche 2
wechseln“. Dann kann die App die Taste nicht registrieren: Das Log zeigt
`hotkey registered failed hotkey=Ctrl+F2 …`, die Einstellungen und die Overlay-Seite
warnen, und die Move-Ebene schreibt „Ctrl+F2 is taken by your desktop – use
Ctrl+Escape or the tray menu“. Empfehlung: die Taste in den KDE-Systemeinstellungen
(*Tastatur → Kurzbefehle → KWin*) freigeben oder auf der Overlay-Seite eine andere
Taste wählen; beenden lässt sich der Modus in jedem Fall mit Ctrl+Esc, über das
Tray-Menü oder `--finish-move`.

## Voice-Packs

Importiert werden Ordner, ZIPs und einzelne `.rtst_vpk`-Dateien
(*Sounds → Packs → Import*). Drei Recursion-Formate werden erkannt (Details und
Erkennungsmerkmale in [docs/voicepacks.md](docs/voicepacks.md)):

| Format | Erkennung | Beispiele |
|---|---|---|
| XML (Stand 2014) | Text mit `<NAME>`-Einträgen, Sounds als Dateien im Ordner daneben | ArnoldVoicePack |
| SharpSerializer, binär | erste Bytes `02 01 04 52 6f 6f 74 01`, Sounds eingebettet oder als Liste | Deadpool v2, C&C V6, Camikaze78 |
| protobuf (protobuf-net) | erste Bytes `0a …`, Typnamen mit `RecursionTracker.`, Sounds eingebettet | ArnoldVoicePackBETA |

Ein .NET-BinaryFormatter-Leser ist vorsorglich enthalten. Packs werden nur
gelesen (Größen- und Tiefenlimits, kein Pfad außerhalb des Pack-Ordners, fremde
absolute Pfade werden ignoriert); das Repo enthält keine Recursion-Dateien.

Zuordnung: Ein Spielereignis erzeugt alle passenden Ereignisse; gespielt wird der
spezifischste Treffer in den aktiven Packs (Rang), die eingebauten Standard-Töne
nur als letzter Ausweg.

| Spielereignis | Schlüssel (spezifisch → allgemein) |
|---|---|
| Kill | Kill-Kette > Killstreak > `ROAD_KILL`/`KNIFE_FIGHT`/`FIRST_BLOOD`/`REVENGE` … > `HEAD_SHOT` (Kopfschuss) > `PER_KILL` > eigener Schlüssel „every kill“ > Standard `kill`/`headshot` |
| Eigener Tod | `BEING_FARMED`/`DEATH_STREAK` > `DEATH_INSULT` > Standard `death` |
| Teamkill begangen | `TEAM_KILLER` > Standard `teamkill` |
| Von Teamkamerad getötet | `WHOSE_SIDE` > `PREVENT_FF` > Standard `teamkilled` |
| Session-Start | `SESSION_STARTED` > Standard `session_start` |

| Kill-Kette | Recursion classic | Unreal |
|---|---|---|
| 2 / 3 / 4 | `EXCELLENT` / `OUTSTANDING` / `SUPERB` | `DOUBLE_KILL` / `TRIPLE_KILL` / `MULTI_KILL` |
| 5 … 10 | `MULTI_KILL` … `HOLY_SHIT` | `MEGA_KILL` … `HOLY_SHIT_2` |

Stil unter *Sounds → Events → Kill chain style* („Automatic“ = classic, wenn ein
Pack `EXCELLENT` hat). 35 der 96 Recursion-Schlüssel haben eine Vorgabe; alle
übrigen lassen sich unter *Sounds → Events* mit „Choose a condition…“ belegen
(Vorlage + Parameter, ohne TOML). Die Events-Seite zeigt oben die
Basis-Ereignisse mit Schlüsselkette und „zuletzt gespielt“.

## Analytics, Export und Import

- **Export** (*History → Session*): **Excel** (.xlsx) mit den Blättern Summary,
  Kills, Deaths (mit den Spalten *Enemy class* und *My class*), Classes (Kills
  und Tode nach Gegnerklasse, eigene Klassen mit ungefähren Minuten), Weapons,
  Vehicles, Revives & Assists, Achievements und Timeline
  (Minutenreihe) als echte Excel-Tabellen mit Zahlen-, Prozent- und
  Datumsformaten (UTC). **JSON** im versionierten Format `ps2-tracker-session`
  (Schema: [docs/session-file.schema.json](docs/session-file.schema.json)),
  **CSV** als einfache Ereignisliste.
- **Import** (*History → Import session…*, nur über den Dateidialog): JSON und
  vom Tracker exportierte Excel-Dateien (die Session steckt zusätzlich als
  eingebettete Daten in der .xlsx; speichert ein Tabellenprogramm die Datei neu,
  gehen sie verloren und der Import lehnt die Datei mit Hinweis ab). Geprüft
  werden Format, Version, Zeiten, Größe (25 MB) und Listenlängen; eine Session
  desselben Charakters mit derselben Startzeit wird nicht doppelt angelegt.
- **Analytics**: Sessions nach Charakter und Zeitraum wählen (bis 50), Diagramme
  K/D, K/D incl. revives, KPM, HSR, XP/h über Sessions, Waffenvergleich, Verlauf
  einer Session, Gegnerfraktionen (Fraktionsfarben), Gegnerklassen und Session A vs. B mit
  Differenzen; Dark/Light; PNG-Export je Diagramm. Diagramme mit Chart.js 4
  (Canvas, kein `eval`, die CSP bleibt `script-src 'self'`; wird erst beim
  Öffnen der Seite geladen).
- Sessions aus Versionen vor 0.2.0 (Kennzeichen „basic“) enthalten im Detail nur
  Kills, Tode und Fahrzeuge; XP-Verlauf, Revives/Assists, Achievements und die
  Fraktion der Gegner fehlen dort.

## Crosshair-Rückmeldung (Hit feedback)

*Crosshair → Hit feedback*: Das Crosshair leuchtet kurz in einer Farbe auf oder
wechselt für einen Moment zu einem anderen PNG aus der Bibliothek – je
Ereignis mit Dauer (Standard 150 ms), Fade, Deckkraft-Spitze und Vorschau.

**Grenzen, ehrlich:** Die Census-API bzw. der Event-Stream liefert **keine
Treffer- oder Schadensereignisse** für Infanterie, nur Tode, Fahrzeug­zerstörungen
und Experience. Einzeltreffer wären nur im Spiel selbst sichtbar – darauf greift
die App nie zu. Auslöser sind deshalb:

| Ereignis | Quelle | Standard |
|---|---|---|
| Kill | Death-Event, du bist der Angreifer | an, Weiß |
| Headshot-Kill | dasselbe mit `is_headshot` | an, Rot |
| Kill Assist | Experience 2, 371, 372 (Kill Player / Priority / High Priority Assist) | aus |
| Fahrzeug zerstört | VehicleDestroy, du bist der Angreifer | aus |
| Fahrzeugschaden | Experience „… Damage (Infantry vs Vehicle)“ bzw. „… Damage“ (25 IDs, in `hitfx.rs` belegt) | aus |

Eigener Tod, Suizid und Teamkill lösen nichts aus. Die Rückmeldung kommt mit der
Verzögerung des Event-Streams (typisch einige hundert Millisekunden); das Log
nennt je Effekt die Zeit seit dem ESS-Zeitstempel (`crosshair feedback …
delay_ms`, der Zeitstempel hat nur Sekundenauflösung). Gezeichnet wird nur der
Inhalt des Crosshair-Fensters – Größe, Position und Fenstertyp bleiben gleich.

## Installation

Offizielle Builds funktionieren direkt mit einer eingebauten Census-Service-ID;
eine eigene ID (siehe unten) hat Vorrang und lässt sich jederzeit eintragen.
Builds aus den Quellen ohne eingebaute ID brauchen eine eigene; der
Erststart-Assistent fragt danach.

### CachyOS / Arch

```bash
git clone https://github.com/Elementary-Beginnings/ps2-tracker.git
cd ps2-tracker/packaging/arch && makepkg -si
# oder das fertige Paket aus dem Release:
sudo pacman -U ps2-tracker-x86_64.pkg.tar.zst
```

**AUR:** `ps2-tracker-bin` (z. B. `paru -S ps2-tracker-bin`) installiert das
vorgebaute Binary eines Releases. Das Paket funktioniert erst, wenn es ein
Release mit Tag gibt und das Paket im AUR veröffentlicht ist; bis dahin ist
`makepkg` aus dem Repo (oben) der Weg.

`makepkg` installiert die Abhängigkeiten (`webkit2gtk-4.1`, `gtk3`,
`libayatana-appindicator`, `alsa-lib`, `librsvg`; zum Bauen `rust`, `nodejs`,
`pnpm`). Unter PipeWire wird `pipewire-alsa` gebraucht. Autostart:
`cp /usr/share/applications/ps2-tracker.desktop ~/.config/autostart/` und dort
`Exec=ps2-tracker --hidden` setzen.

### AppImage (andere Distributionen)

```bash
chmod +x ps2-tracker-x86_64.AppImage && ./ps2-tracker-x86_64.AppImage
```

### Flatpak

```bash
flatpak install --user ps2-tracker.flatpak     # Bundle aus dem Release
flatpak run dev.herz.Ps2Tracker
```

Das Bundle braucht die Laufzeit `org.gnome.Platform//51` von Flathub (wird beim
Installieren nachgeladen, wenn Flathub eingerichtet ist). Rechte und ihre Gründe
stehen im Manifest `packaging/flatpak/dev.herz.Ps2Tracker.yml`: X11 (Overlay,
Crosshair und Fokus-Erkennung laufen über XWayland), Wayland, PulseAudio,
Netzwerk, Tray. Kein Zugriff auf das Home-Verzeichnis – Packs und Crosshairs werden
über den Datei-Dialog des Portals importiert. Die Daten liegen unter
`~/.var/app/dev.herz.Ps2Tracker/`. Globale Hotkeys gehen über X11 und
funktionieren im Sandbox mit `--socket=x11`; unter Wayland wie bei den anderen
Paketen eigene KDE-Kurzbefehle anlegen (`flatpak run dev.herz.Ps2Tracker --toggle-crosshair`).

### Windows 10/11

`ps2-tracker-x64-setup.exe` ausführen (Installation für den
aktuellen Benutzer, keine Administratorrechte). Fehlt WebView2, lädt der
Installer Microsofts Bootstrapper nach (`downloadBootstrapper`, Standard von
Tauri; Windows 11 und aktuelle Windows-10-Systeme haben WebView2 bereits).

- **Nicht signiert:** Es gibt kein Code-Signing-Zertifikat. SmartScreen warnt
  deshalb („Der Computer wurde durch Windows geschützt“ → *Weitere Informationen*
  → *Trotzdem ausführen*). Vorher die Prüfsumme vergleichen:
  `Get-FileHash .\ps2-tracker-x64-setup.exe -Algorithm SHA256` gegen `SHA256SUMS`.
- **Overlay und Crosshair nur im Fenster- bzw. randlosen Vollbildmodus:** Im
  exklusiven Vollbild gehört dem Spiel die Anzeige allein; Microsoft beschreibt,
  dass Overlays dann nur durch Eingriff in das Rendering des Spiels möglich sind,
  während im randlosen Fenster bzw. mit Fullscreen Optimizations „the overlay can
  be composited on top of the game in a safe and stable way“
  ([DirectX Developer Blog: Demystifying Full Screen Optimizations](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations)).
  In PlanetSide 2 deshalb *Windowed Fullscreen* bzw. *Borderless* wählen.
- Fenster: Overlay und Crosshair sind randlose, transparente Fenster mit
  `WS_EX_TOPMOST`, `WS_EX_LAYERED | WS_EX_TRANSPARENT` (durchklickbar) und
  `WS_EX_NOACTIVATE` (nehmen keinen Fokus) – gesetzt von Tauri/tao
  (`always_on_top`, `set_ignore_cursor_events`, `focusable(false)`).
- Fokus-Erkennung: Vordergrundfenster und Dateiname seines Prozesses
  (`PlanetSide2_x64.exe`, passt auf das Muster `planetside2`). Der Prozess wird nur
  mit `PROCESS_QUERY_LIMITED_INFORMATION` geöffnet, um den Namen zu lesen.
- Daten: `%APPDATA%\ps2-tracker\` (Konfiguration, Datenbank, Packs, Log).
- Kein Konsolenfenster beim Start (GUI-Subsystem); Ausgaben nur, wenn die App aus
  einem Terminal gestartet wird, und im Debug-Modus – siehe
  [Fehlersuche](#fehlersuche).
- Hotkeys über `RegisterHotKey` (global-hotkey). Ctrl+Esc ist unter Windows
  systemweit reserviert (Startmenü); „Finish move mode“ ist dort deshalb
  **Ctrl+Shift+F2** (ist die Kombination schon von einem anderen Hotkey der App
  belegt: Ctrl+Alt+F2). Eine vorhandene Windows-Konfiguration mit Ctrl+Esc wird beim
  Start umgestellt (Log: `finish move mode key migrated`). Lässt sich eine Taste
  nicht registrieren, zeigt die App das wie unter KWin an (Einstellungen,
  Overlay-Seite, Move-Ebene); dann auf der Overlay-Seite eine andere Taste wählen.
  Tray-Menü, Knopf und `--finish-move` gehen immer.
- Move-Ebene: randloses Fenster mit `WS_EX_TOPMOST` über dem Spiel (randloses
  Vollbild), Vordergrund über Tauri/tao (`SetForegroundWindow`, mit tao's
  Umweg um die Vordergrundsperre). Ob das Spiel den Mauszeiger einfängt, liest
  die App mit `GetClipCursor` (Clip-Rechteck kleiner als der virtuelle
  Bildschirm = eingefangen) vor dem Öffnen und danach alle 500 ms; dann zeigt die
  Ebene wie unter Linux „The game holds the cursor“. Die Ebene übernimmt beim
  Öffnen den Vordergrund; viele Spiele geben den Zeiger dann frei (für
  PlanetSide 2 unter Windows nicht geprüft). Die Tastatur der Ebene geht in jedem
  Fall. Die X11-Teile (OSD-Fenstertyp, `_NET_ACTIVE_WINDOW`, Grab-Probe) gibt es
  nur unter Linux. Die Win32-Aufrufe ohne sichere Hülle (Konsole,
  `GetClipCursor`) stehen in `src-tauri/src/win_console.rs`, dem einzigen Modul
  mit `unsafe`-Code (sonst `unsafe_code = "deny"`, per Test geprüft).
- Fokus-Erkennung unter Windows sieht nur das Vordergrundfenster (die Liste aller
  Fenster bräuchte `EnumWindows`, ebenfalls unsicheres FFI); die Move-Ebene öffnet
  deshalb nur dann auf dem Monitor des Spiels, wenn das Spiel gerade vorne ist,
  sonst auf dem Monitor des Overlays.

## Census-Service-ID

Die Census API braucht eine Service-ID. Die App nimmt, in dieser Reihenfolge:

1. die Umgebungsvariable `PS2_SERVICE_ID` (wird nie in die Datei geschrieben),
2. die eigene ID aus **Settings → Census API** (ohne `s:`; steht in der lokalen
   Konfiguration),
3. die eingebaute Vorgabe offizieller Builds,
4. `example` (nur sehr wenige Anfragen möglich).

**Offizielle Builds enthalten eine Service-ID des Projekts.** Sie wird beim Bauen
eingesetzt (CI-Variable, nicht im Repo) und ist aus dem Binary auslesbar. Sie
gibt keinen Zugriff auf ein Konto; Daybreak nutzt sie nur, um Anfragen dieser
App zuzuordnen und zu begrenzen. Die App zeigt sie nie an, loggt sie nicht und
schreibt sie nicht in die Konfiguration – in den Einstellungen steht nur
„built-in default“. Eine eigene ID lässt sich jederzeit eintragen; kostenlos
registrieren unter <https://census.daybreakgames.com/#devSignup>. Feld leeren =
zurück zur Vorgabe. Builds aus den Quellen haben keine eingebaute Vorgabe
(`PS2_DEFAULT_SERVICE_ID` beim Kompilieren, nur Buchstaben, Ziffern und `_`) und
nutzen `example`, bis eine ID eingetragen ist.

## Konfiguration und Daten

| Was | Pfad |
|---|---|
| Konfiguration (auch per UI) | `$XDG_CONFIG_HOME/ps2-tracker/config.toml` |
| eigene Achievements | `$XDG_CONFIG_HOME/ps2-tracker/achievements.toml` |
| eigene Auslöser für Sounds (ganzes Regelset) | `$XDG_CONFIG_HOME/ps2-tracker/triggers.toml` |
| in der Oberfläche geänderte Bedingungen | `config.toml`, `[voice.trigger_overrides]` |
| Sessions und Namens-Cache | `$XDG_DATA_HOME/ps2-tracker/tracker.sqlite3` |
| Voice-Packs | `$XDG_DATA_HOME/ps2-tracker/voicepacks/` |
| Crosshairs | `$XDG_DATA_HOME/ps2-tracker/crosshairs/` |
| Log (aktueller Lauf, vorheriger als `.log.1`) | `$XDG_STATE_HOME/ps2-tracker/ps2-tracker.log` (meist `~/.local/state/ps2-tracker/ps2-tracker.log`) |

Vorlagen: `crates/ps2-tracker-core/assets/achievements.default.toml` und
`triggers.default.toml` (im Paket unter `/usr/share/ps2-tracker/`).

## Overlay und Crosshair unter KDE Plasma (Wayland)

Beide sind eigene, transparente Tauri-/GTK-Fenster.

### Warum die App über XWayland läuft

KWin ordnet Fenster in Ebenen (Quelle: `Window::belongsToLayer()` in
`kwin/src/window.cpp`, Reihenfolge in `src/effect/globals.h`):

`Above` („Immer im Vordergrund“) < `Notification` < `Active` (aktives
Vollbildfenster) < `Popup` < `CriticalNotification` < `OnScreenDisplay` < `Overlay`

Ein Spiel im Modus *Fullscreen Windowed* ist ein aktives Vollbildfenster und liegt
damit **über** jedem „Immer im Vordergrund“-Fenster – genau das ist der gemeldete
Fehler (Crosshair verschwindet im randlosen Vollbild, im Fenstermodus nicht).
Darüber liegen nur die Ebenen `CriticalNotification`, `OnScreenDisplay` und `Overlay`.

- Ein **natives Wayland-Fenster** kann weder seine Position noch eine solche Ebene
  selbst wählen; Layer-Shell (`wlr-layer-shell`) bietet GTK3/Tauri nicht an.
- Ein **X11-Fenster** (über XWayland, wie das Spiel unter Proton) kann den
  Fenstertyp `_KDE_NET_WM_WINDOW_TYPE_ON_SCREEN_DISPLAY` setzen; KWin legt es dann in
  die Ebene `OnScreenDisplay`, über das Vollbildspiel.

Deshalb startet die App sich in einer Wayland-Sitzung einmal selbst mit
`GDK_BACKEND=x11` neu (Log: `backend="x11"`). Danach setzt sie für Crosshair und
Overlay per X11 den Fenstertyp „On-Screen-Display“ (Rückfall
`_NET_WM_WINDOW_TYPE_NOTIFICATION` für andere Fenstermanager) und bildet das
Fenster einmal neu ab, weil KWin den Typ nur beim Verwalten eines Fensters liest.
Das Fenster nimmt keinen Fokus an (`WM_HINTS input=false`) und bleibt
durchklickbar. Das Overlay wechselt den Typ nicht mehr (früher bekam es zum
Verschieben den normalen Typ zurück; jeder Wechsel braucht ein neues Abbilden,
danach platziert der Fenstermanager das Fenster unter Umständen neu). Es wird wie
das Crosshair behandelt: feste Größe (gemessene Größe des Inhalts, ändert sich nur
mit den Einstellungen, nie durch neue Killfeed-Zeilen oder Einblendungen),
Position aus Anker und Abstand. Im Log stehen `overlay placed: preset=…, size=…,
requested x,y, monitor=…` und kurz danach `overlay position check requested=…
actual=…` (bei Abweichung setzt die App die Position bis zu dreimal in 10 s neu).

Abschalten: *Settings → Appearance → Run windows through XWayland* abwählen
(`ui.x11_backend = false`) oder `PS2_TRACKER_WAYLAND=1` setzen. Nur den Fenstertyp
abschalten (Diagnose): `PS2_TRACKER_NO_OSD=1`.

### KWin-Fensterregel (zusätzlich empfohlen)

*Systemeinstellungen → Fensterverwaltung → Fensterregeln → Importieren* und
`/usr/share/ps2-tracker/ps2-tracker.kwinrule` (im Repo
`packaging/kde/ps2-tracker.kwinrule`) laden. Die Regel erzwingt für Overlay,
Einblendung und Crosshair (Abgleich über den exakten Fenstertitel) die **Ebene „On-screen display“**
(`layer=osd`, `layerrule=2` = Erzwingen), „Immer im Vordergrund“, „Keinen Fokus
annehmen“ und blendet sie aus Taskleiste, Pager und Fensterwechsler aus. Die
Regel-Eigenschaft „Layer“ gibt es in KWin seit mindestens 6.2 (geprüft in
`src/rulesettings.kcfg` der Tags v6.2.0 bis v6.6.0). Die Regel wirkt auch dann,
wenn der Fenstertyp nicht gesetzt werden konnte. Mit dem Wayland-Backend wirkt sie
ebenfalls auf die Ebene, die Position des Crosshairs bleibt dann aber KWin
überlassen.

Ein KWin-Skript ist keine Alternative: In der Skript-API ist `layer` nur lesbar
(`Q_PROPERTY(KWin::Layer layer READ layer)` in `src/window.h`), und `keepAbove`
hebt nur in die Ebene `Above`, die unter dem Vollbildspiel liegt.

### Weiteres

- **Transparenz**: braucht einen Compositor; KWin ist immer compositing.
- **Durchklickbar**: GTK-Eingaberegion. tao (Tauri) lässt eine 1×1-Pixel-Fläche
  oben links im Fenster klickbar; beim Crosshair liegt sie im transparenten Rand.
- **Hotkeys**: Globale Hotkeys funktionieren nur über X11/XWayland. Alternativ in
  *Systemeinstellungen → Kurzbefehle → Eigene Kurzbefehle* Befehle anlegen:

  | Aktion | Befehl |
  |---|---|
  | Overlay ein/aus | `ps2-tracker --toggle-overlay` |
  | Move-Modus des Overlays ein/aus | `ps2-tracker --toggle-edit` |
  | Crosshair ein/aus | `ps2-tracker --toggle-crosshair` |
  | nächstes Crosshair | `ps2-tracker --next-crosshair` |

  Die laufende Instanz übernimmt den Befehl.
- **Crosshair nur bei aktivem Spiel**: liest `_NET_ACTIVE_WINDOW`, `WM_CLASS` und
  den Fenstertitel vom X-Server (XWayland); PlanetSide 2 meldet sich unter Proton
  als `steam_app_218230`. Ausgeblendet wird nur, wenn sicher ein anderes X-Fenster
  aktiv ist. Ist die Erkennung unsicher (kein X-Server, Abfrage fehlgeschlagen,
  natives Wayland-Fenster aktiv), bleibt das Crosshair sichtbar. Der erkannte
  Zustand steht unter *Crosshair → Check active window* und im Log.
- **HiDPI/Fraktionsskalierung**: Fenstergröße und Position in physischen Pixeln;
  das Log nennt Monitor, Skalierung und Fensterposition bei jeder Änderung.

Ein Crosshair-Overlay ist ein externes Fenster über dem Spiel und verändert das
Spiel nicht.

**OBS**: *Settings → OBS browser source* aktivieren und in OBS eine
Browser-Quelle auf `http://127.0.0.1:47823/` anlegen.

## Fehlersuche

Die App schreibt ihr Log nach `~/.local/state/ps2-tracker/ps2-tracker.log`
(`$XDG_STATE_HOME`; der vorige Lauf bleibt als `ps2-tracker.log.1`). Der Pfad steht
auch unter *Settings → Files*. Ausführlich protokollieren:

```bash
pkill ps2-tracker                       # laufende Instanz beenden
ps2-tracker --debug-console             # Debug-Modus, Log auch im Terminal
# oder: RUST_LOG=ps2_tracker=debug,ps2_tracker_core=debug ps2-tracker
# danach: ~/.local/state/ps2-tracker/ps2-tracker.log schicken
```

**Debug-Modus** (`--debug-console` oder *Settings → Advanced → Show debug console
on start*, Standard aus): Log-Level debug für die App. Unter Linux erscheint das
Log zusätzlich im Terminal, aus dem die App gestartet wurde (aus dem Menü gestartet
öffnet sich kein Terminal; alle `.desktop`-Dateien haben `Terminal=false`).

**Windows:** Die Release-Version startet ohne Konsolenfenster (GUI-Subsystem).
Aus einer Eingabeaufforderung oder PowerShell gestartet, schreibt sie
`--help`, `--version` und die Meldungen von `--toggle-*`/`--finish-move` in dieses
Fenster (die Eingabeaufforderung zeigt ihren Prompt dabei schon vorher wieder an).
Für den Debug-Modus eine Verknüpfung auf `ps2-tracker.exe` anlegen und unter
*Ziel* ` --debug-console` anhängen, oder die Einstellung oben setzen und die App neu
starten: dann öffnet sich ein eigenes Konsolenfenster „PS2 Session Tracker – debug
log“ mit dem Log. Das Log liegt in jedem Fall unter
`%APPDATA%\ps2-tracker\` (Pfad unter *Settings → Advanced*).

Was im Log steht:

- Start: Version, Sitzungstyp, `WAYLAND_DISPLAY`/`DISPLAY`/`GDK_BACKEND`, das
  verwendete Fenster-Backend, Monitore mit Skalierung.
- Ton: Audio-Host, Standardgerät und alle Ausgabegeräte; welches Gerät geöffnet
  wurde (Rate, Kanäle, Puffer); je Spielereignis der gewählte Sound bzw. der Grund,
  warum keiner spielt (stummgeschaltet, kein Eintrag/Cooldown, `SOUND=false`);
  Start, Ende, Verdrängen und Verwerfen in der Warteschlange; Fehler beim Öffnen
  oder Dekodieren mit Dateipfad und Codec; Fehler des Ausgabestroms. Bleibt der
  Ausgabestrom stehen (das Gerät holt keine Samples mehr), wird das erkannt
  (`audio output stalled`) und das Gerät neu geöffnet. Panics in jedem Thread
  landen ebenfalls im Log.
- Crosshair: jede Sichtbarkeitsänderung mit Grund (Start, Konfiguration,
  Hotkey, Fokus-Watcher), Monitor/Position, erkannter Fokuszustand mit
  `WM_CLASS`, gesetzter X11-Fenstertyp; je Hit-feedback-Effekt Art und
  Verzögerung seit dem ESS-Zeitstempel.
- Sounds: je Ereignis die geprüfte Schlüsselkette mit Datei
  (`voice: kill: tried DOUBLE_KILL(miss) … PER_KILL(hit, Pack: datei.ogg)`).
- Import: der Grund einer Ablehnung (Format, Version, Größe, fehlende
  eingebettete Daten) als Meldung in der Oberfläche; `session imported` im Log.

Ton-Vorschau (▶) meldet Fehler jetzt als Meldung in der Oberfläche
(z. B. Datei fehlt, Codec nicht unterstützt, kein Ausgabegerät). Unterstützt
werden Ogg Vorbis, MP3, FLAC und WAV; Ogg Opus wird erkannt und mit Hinweis
abgelehnt. Unter PipeWire braucht die App `pipewire-alsa`: Ist kein Gerät gewählt
(*System default (PipeWire)*), öffnet sie zuerst das ALSA-Gerät `pipewire` und erst
danach das ALSA-Standardgerät (dort scheitert `dmix` gelegentlich mit
„unable to open slave“). Das geöffnete Gerät steht im Log (`audio output opened`).

### Graues/leeres Fenster (NVIDIA)

Mit dem proprietären NVIDIA-Treiber bleibt das Fenster von WebKitGTK oft grau oder
leer (DMA-BUF-Renderer über GBM). Erkennt die App beim Start den NVIDIA-Treiber
(`/sys/module/nvidia` bzw. `/proc/driver/nvidia/version`), setzt sie
`WEBKIT_DISABLE_DMABUF_RENDERER=1` und startet sich damit neu. Die Tauri-Doku
beschreibt diese Umgebungsvariable als Umgehung für leere Fenster
(<https://v2.tauri.app/develop/debug/linux-graphics/>) und rät davon ab, sie
pauschal zu setzen – deshalb nur bei NVIDIA. Im Log steht dazu eine Zeile
`graphics environment` mit dem erkannten Treiber und der Herkunft der Variable
(`set by ps2-tracker`, `set by the user` oder `unset`).

Abschalten: `PS2_TRACKER_NO_DMABUF_FIX=1 ps2-tracker` oder in der
`config.toml` unter `[ui]` `webkit_dmabuf_workaround = false`. Wer die Variable
selbst setzt, behält seinen Wert.

`PS2_TRACKER_WAYLAND=1` startet ohne XWayland, `PS2_TRACKER_NO_OSD=1` ohne den
On-Screen-Display-Fenstertyp – beides nur zur Eingrenzung.

## Kommandozeile

```
ps2-tracker --help
```

## Entwicklung

```bash
pnpm install
pnpm tauri dev                      # App mit Hot Reload
pnpm build && cargo test --workspace
pnpm check && pnpm test
cargo clippy --workspace --all-targets -- -D warnings
node tools/contrast.mjs              # WCAG-2.1-AA-Kontrast der Akzentfarben (src/lib/theme.ts)
PS2_SERVICE_ID=<id> cargo run -p ps2-tracker-core --example census_probe -- <Charakter>
PS2_SERVICE_ID=<id> cargo run -p ps2-tracker-core --example ess_probe -- 10 20
```

Aufbau:

| Teil | Inhalt |
|---|---|
| `crates/ps2-tracker-core` | ohne GUI: Census-Client (Rate-Limit, Retry), ESS-Client (Reconnect, Backoff), Event-Modell, Session-Logik, Tracker, Achievements, Voice (Fakten, Auslöser, Auswahl, Warteschlange), Pack-Lader (offen/Recursion), Crosshair-Bibliothek, SQLite |
| `src-tauri` | Tauri-App: Commands, Audio (rodio), Fenster, Tray, Hotkeys, OBS-Endpunkt, Fenster-Fokus (XWayland) |
| `src/` | Svelte-5-Oberfläche (Haupt-UI, Overlay, Crosshair); Texte in `src/lib/i18n/en.ts` |

Sicherheit: Tauri-Capabilities je Fenster minimal (Overlay/Crosshair nur
lesend), CSP ohne Remote-Inhalte, Census nur lesend, keine Telemetrie.
`gitleaks` läuft als pre-commit-Hook: `git config core.hooksPath .githooks`.

Testdaten: echte ESS-Events (öffentliche Spieldaten) in
`tests/fixtures/ess_real_2026-11-20.jsonl`; Audio- und Recursion-Fixtures sind
selbst erzeugt (`tools/gen_test_ogg.py`).

## Lizenz

Wahlweise **MIT** oder **Apache-2.0** (`MIT OR Apache-2.0`), siehe
[`LICENSE-MIT`](LICENSE-MIT) und [`LICENSE-APACHE`](LICENSE-APACHE). Eigene Assets
(Standard-Töne, Crosshairs, Icon, Test-Fixtures, Screenshots) stehen unter CC0,
siehe [`assets/LICENSES.md`](assets/LICENSES.md). Lizenzen der Abhängigkeiten:
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) (liegt auch jedem Release bei;
neu erzeugen mit `python3 -I tools/third_party_notices.py`), geprüft in der CI mit
`cargo deny check licenses`.

Beiträge: [`CONTRIBUTING.md`](CONTRIBUTING.md), Sicherheitslücken:
[`SECURITY.md`](SECURITY.md), Verhaltenskodex:
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## Haftungsausschluss und Marken

Dieses Projekt ist ein unabhängiges Fan-Projekt. Es ist **nicht** mit Daybreak
Game Company, Rogue Planet Games, PlanetSide 2 oder Recursion Tracker verbunden,
wird von ihnen weder unterstützt noch geprüft. PlanetSide und PlanetSide 2 sind
Marken der Daybreak Game Company LLC. „Recursion“ wird nur als Kompatibilitätsangabe verwendet: Die
App liest das Dateiformat von Recursion-Voice-Packs mit eigenen, unabhängig
geschriebenen Lesern; es sind **keine** Recursion-Dateien, -Sounds oder -Packs
enthalten. Nutzung auf eigene Verantwortung, ohne Gewähr (siehe Lizenz).
