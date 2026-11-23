# Roadmap

State after 0.2.0 (unreleased). Rough effort: **S** = up to a day, **M** = a few
days, **L** = a week or more.

## Done in 0.2.0

- [x] **Kill feed names instead of “–”**: names for the visible kill feed are
  looked up every 30 s and the entry is updated (cache in SQLite); special cases
  are named (Yourself, Environment, Looking up…, Unknown player).
- [x] **Kill feed colours** in the app, overlay and OBS (kill green, death orange,
  own headshot cyan, “HS taken” coral) with a contrast test
  (`src/lib/contrast.test.ts`, reads the values from `app.css`/`Overlay.svelte`).
- [x] **Data model**: session data version 2 (revives/assists/captures, XP per
  minute, achievements, enemy faction), SQLite schema 2 with migration and test.
- [x] **Excel export** (`rust_xlsxwriter`): Summary, Kills, Deaths, Weapons,
  Vehicles, Revives & Assists, Achievements, Timeline as real tables.
- [x] **Import** of JSON (`ps2-tracker-session`, schema in
  `docs/session-file.schema.json`) and own .xlsx files; validation, limits,
  duplicates by character + start time, only through the file dialog.
- [x] **Analytics page**: selection/filter by character and period, trends,
  weapons, progress, enemy factions, A vs. B, PNG export, dark/light. Chart
  library: **Chart.js 4.6.1** (MIT). Evaluated were uPlot (very small, but only
  lines/areas, bars only through plugins) and ECharts (very large, ~1 MB);
  Chart.js covers line, bar and combined charts, draws on canvas, needs no `eval`
  (CSP `script-src 'self'` stays) and is loaded only when the page is opened
  (~68 kB gzip).
- [x] **Privacy documentation** for export/import in `docs/wiki/Permissions-and-Privacy.md`.
- [x] **Events page** reorganised, conditions editable in the user interface.
- [x] **Hit feedback** in the crosshair (kill, headshot, assist, vehicle, vehicle
  damage).

## Open

### Analytics and export

- **Filter by weapon and class/loadout** (M): loadout IDs are in the ESS events
  (`attacker_loadout_id`, `character_loadout_id`) but are not stored in the
  session yet → data version 3.
- **Server and lifetime comparison in the Excel sheet Summary** (S): `world_id` is
  in the event but not in the session; lifetime values would need a Census request
  at export time.
- **Several sessions in one Excel file** (S–M).
- **Time zone**: Excel times are UTC; local time as an option (S).

### Sounds

- `SESSION_ENDED`, `SESSION_RESUMED`, `SESSION_SAVED` have no event yet (S).
- `SUICIDE_BOMBER`, `VIP_KILL`/`VIP_TEAM_KILL`, `QUAD_DAMAGE`: no preset without
  reliable data; freely assignable through “Choose a condition…”. Check again if
  Census/ESS get suitable fields (S).
- Cache the background images of the packs on the Sounds page instead of encoding
  them again on every opening (S; noticeable only with many packs with large
  images).

### Crosshair

- Centring on the game window instead of the monitor as an option (M; today's
  behaviour stays the default).
- Hit feedback: swap PNGs larger than the crosshair are cut off today – the window
  size is deliberately not changed (the positioning logic stays untouched).

### Squad, Discord, updates

- [x] Overlay block “Squad top 3” (S–M): done, with a fixed size (always three
  rows, no new measuring during the session).
- [x] Continent/server in Squad, side bar and Rich Presence (S): done, session data
  version 3.
- Updates: usable only once the signing key is configured and the public key is
  entered (docs/release.md).

### Quality

- Continuous, coverage-guided fuzzing of the pack readers (cargo-fuzz) (M); today
  there are mutation tests (`fuzz_no_panic`).
- Automate tests under KWin/Plasma on real hardware (L; today Xvfb + xfwm4 and
  manual checks).
