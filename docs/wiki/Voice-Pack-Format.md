# Voice Pack Format

Reference for pack authors: file formats, triggers and keys. For using packs in
the app see [Voice Packs](Voice-Packs).

The PS2 Session Tracker plays tones or voice lines from voice packs on events. There
are two formats, recognised automatically on import:

| Format | Recognised by | Read | Export |
|---|---|---|---|
| own, open format | `voicepack.toml` in the pack folder | yes | yes (ZIP) |
| Recursion format | `<name>.rtst_vpk` in the pack folder | yes | yes (ZIP with `.rtst_vpk`) |

Import through **Sounds & Voice → “Import .rtst_vpk / ZIP …”** (file) or **“Import
folder …”** (folder). RAR is not supported: there is no Rust library without the
licence conditions of unRAR. Unpack RAR archives first and import the folder or
the `.rtst_vpk`.

Packs are then stored unchanged under `$XDG_DATA_HOME/ps2-tracker/voicepacks/<id>/`.
Several packs can be active; their order is the fallback per event.

## Own format (`voicepack.toml`)

```toml
[pack]
name = "My pack"
author = "Your Name"
version = "1.0.0"
license = "CC-BY-4.0"        # SPDX identifier or free text
description = "optional"

[[event]]
event = "headshot"           # event ID (see below)
files = ["sounds/hs1.ogg", "sounds/hs2.ogg"]   # relative to the pack folder
priority = 30                # 0–100, optional (default per event)
cooldown_ms = 0              # minimum gap between two playbacks
volume = 1.0                 # 0.0–2.0

[[event]]
event = "multikill"
files = ["sounds/triple.ogg"]
[event.when]                 # optional conditions, all must match
min_count = 3
max_count = 3
# headshot = true
# weapon_category = ["Sniper Rifle"]   # Census weapon category
# vehicle = ["Sunderer"]               # vehicle name as in Census

[[event]]
event = "TEAM_KILLER"
silent = true                # no sound, no fallback either
overlay = true               # pop-up in the overlay (default: true)
stream = false               # pop-up in the OBS source (default: true)
```

Rules:

- Audio formats: Ogg/Vorbis, MP3, FLAC, WAV.
- Several files per entry: random choice without direct repetition.
- Several entries for one event: the most specific matching entry wins (the most
  conditions met).
- An entry without `files` means “no sound in this pack”: the sound comes from the
  next active pack, the pop-up follows this entry.
- Paths are relative, without `..` and without absolute paths; symlinks are not
  taken over. Limits: 20 MB per file, 200 MB per pack, 1000 files.
- Own files per event (Sounds & Voice → “File …”) take precedence over all packs.

## Recursion format (`.rtst_vpk`)

**Established** from a real pack of the maintainer (ArnoldVoicePack, as of 2014)
and its read-me. The Recursion program and installer were neither unpacked nor
decompiled for this. The repo contains only a synthetic replica with tones made by
the project (`crates/ps2-tracker-core/tests/fixtures/recursion/`).

Structure:

```
ArnoldVoicePack.rtst_vpk
ArnoldVoicePack/
    Cool.ogg
    ...
```

The `.rtst_vpk` is XML 1.0, UTF-8 with BOM and CRLF:

```xml
<AchievementSettings>
  <ACHIEVEMENTDATA>
    <AUTHOR>…</AUTHOR>
    <DESCRIPTION>…</DESCRIPTION>
    <DATA>
      <ACHIEVEMENT>
        <NAME>HEAD_SHOT</NAME>
        <PATH>.\ArnoldVoicePack\Cool.ogg</PATH>
        <IMAGE_INGAME>true</IMAGE_INGAME>
        <IMAGE_STREAMING>true</IMAGE_STREAMING>
        <SOUND>true</SOUND>
      </ACHIEVEMENT>
      …
```

Mapping in the tracker:

| Recursion | Tracker |
|---|---|
| `NAME` | event ID (the 96 keys are events of their own) |
| `PATH` relative, `.\…\…` | path with `/`, resolved case-insensitively |
| `PATH` = `Default` | no pack sound → fallback to the next pack (standard tones) |
| `SOUND` = `false` | `silent`: no sound, no fallback either |
| `IMAGE_INGAME` | `overlay`: pop-up in the overlay window |
| `IMAGE_STREAMING` | `stream`: pop-up in the OBS browser source |
| unknown `NAME` values | are kept (without a trigger) |
| unknown fields | are kept for the export |

Robustness: BOM and CRLF are tolerated; the XML parser resolves neither a DTD nor
own entities (only `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;` and character
references), XML definition file at most 1 MB (the binary variants below have
their own limit). Paths with `..`, absolute paths and symlinks are refused; missing
or refused files appear as a notice in the import dialog and fall back to
“Default”.

Export in the Recursion format writes the same structure (BOM, CRLF, all 96 keys
in the order of the sample pack, original entries are kept). Recursion knows only
one file per event: the first one is exported. Own events (`kill`, `multikill` …)
do not exist there.

If a Recursion pack is edited and saved in the user interface, the tracker creates
a `voicepack.toml` next to it; the pack is then kept in the own format (the
`.rtst_vpk` stays and serves as the base for a Recursion export).

### Binary variants of the `.rtst_vpk`

Newer Recursion packs are not XML. The tracker recognises the variant by the first
bytes; all three are **read as data only** (no types instantiated, no
deserialisation in the .NET sense) and converted into the own format on import:
embedded sounds are written as files to `sounds/` in the pack folder, the
definition becomes the `voicepack.toml`. Several sounds per key are kept (random
choice), unknown keys too. Limits: definition file at most 256 MB, embedded file
at most 32 MB (sound at most 20 MB), pack at most 200 MB.

| Variant | Recognised by | Example | State |
|---|---|---|---|
| A: SharpSerializer, binary (“burst”) | `02 01 04 52 6f 6f 74 01` = element “ComplexObject”, name `Root`, type name follows | Deadpool v2, C&C Recursion Voice Pack V6 | reader after the public source of SharpSerializer (MIT); structure from the analysis of the packs, tested with synthetic files |
| A': .NET BinaryFormatter ([MS-NRBF]) | `00 01 00 00 00 ff ff ff ff 01 00 00 00 00 00 00 00` | (none known) | as a precaution, because the first analysis of Deadpool said so |
| B: protobuf (protobuf-net) | `0a …` + type names with `RecursionTracker.` | ArnoldVoicePackBETA | field numbers from the analysis, tested with synthetic files |

Variant A (SharpSerializer) has a peculiarity: the type of a simple value is
missing in the file if it equals the declared type of the .NET property. The
tracker derives it from the name (`*Enabled`/`dynamicSounds` → bool,
`Key`/`*Path`/`*File`/`Name`/`author`/`description` → text, `*Image`/`*Data` →
bytes, `guid` → System.Guid) and otherwise tries candidates after which the next
entry is plausible. If that fails, the error message names member, class and
offset; with `RUST_LOG=ps2_tracker_core=debug` every derived member is logged with
type and offset.

Structure of a simple value (SimpleProperty, element `06`): name (flag + text),
type (flag `00` = as declared, `01` = type name follows as text), value (flag
`00` = null, `01` = data follows). `06 01 04 guid 00 01` + 16 bytes therefore
means: name “guid”, no type name, value present. For the member `dataType` the
type name is inline (`01 5f …` = 95 characters “System.RuntimeType, mscorlib,
Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089”), the value is
a text. The data is encoded as in SharpSerializer's
`BinaryReaderTools.readValueCore` (BinaryReader primitives, little endian):
Boolean 1 byte, Byte/SByte 1, Char UTF-8, Int16/UInt16 2, Int32/UInt32 4,
Int64/UInt64 8, Single 4, Double 8, Decimal 16, DateTime and TimeSpan as Int64
ticks, Guid 16 raw bytes, Enum as Int32, Type and String as .NET string (7-bit
length + UTF-8), Byte[] as a number (size byte 0/1/2/4 + value) + bytes. All these
types are covered by the test `all_simple_types` in `sharpser.rs`.

Mapped fields (A/A', names as in the packs): `ComponentInformation` (`Name`,
`author`, `description`, Base64), `AchievementOptions` per key (`Key`,
`fileSoundPath`, `soundEnabled`, `imageEnabledInGame`, `imageEnabledStreaming`,
`pakSoundPath`, `sounds` = list of `BasicAchievementSound` with
`soundFile`/`pakSoundFile`). Embedded data is found by its name
(`<file>.ogg_System.Byte`): as a dictionary entry name → bytes, as an object
`fileName`/`dataType`/`data` (structure as in the Camikaze78 pack) or as an object
with exactly one text and one byte field. `backgroundImage` names the background
image (embedded or loose next to the `.rtst_vpk`); it takes precedence over the
name rule. `backgroundImageDisable = true` means: no background image. Base64
texts are decoded if that results in readable UTF-8 text.

Variant B: the top level repeats field 1 = component (field 8 type name, field 10
payload). In the `GroupManager` per entry field 1 = key (plain text), field 2 =
options (1 `fileSoundPath`, 2/3/4 soundEnabled/imageEnabledInGame/
imageEnabledStreaming, 6 = sound with 1 `soundFile`, 2 `pakSoundFile`). Components
`System.Byte` carry the sounds, `System.Drawing.Bitmap` an image (1 file name,
2 PNG). **Name, author and description are encrypted in this variant** and are not
read (no key, no decryption): the pack name comes from the file name, the author
stays empty.

Rules for all variants: embedded data takes precedence; if it is missing, the loose
file relative to the `.rtst_vpk` is used (also two folder levels, folder names with
`&`). Names with `..`, absolute paths and symlinks are refused. Folders that are
not referenced (e.g. “Unused Units”) are not taken over. “default” means: sound
from the next pack. Error messages distinguish “not a Recursion pack”, “too large
(X MB, limit Y MB)” and “corrupt PAK record at offset N”.

Export: the tracker writes no format for binary packs. Export in the Recursion
format produces the XML variant with an audio folder (the first file per key).

This is tested only with synthetic files of the same structure
(`tests/recursion_binary.rs`). Whether the real packs (Deadpool, C&C V6,
ArnoldVoicePackBETA) are read completely shows only the import at the
maintainer's; the log names the variant, the number of entries, embedded files and
images.

### Background image of the pop-up

Derived from sample packs, not documented: BL2Zer0 puts `AchievementBG.png` into
the audio folder (not referenced by the XML), ArnoldVoicePackBETA embeds
`ArnoldBG.png` as `System.Drawing.Bitmap`. On import the tracker takes such an
image over as `AchievementBG.png` into the pack folder: embedded, otherwise
`AchievementBG.png` (any case) in the root or audio folder, otherwise the only PNG
in the root folder. Checked are: a real PNG, at most 2048×512 pixels, at most 4 MB.

## Events and triggers

New keys of the binary packs (among them `PER_KILL`, `MAX_DOWN`, `SESSION_STARTED`,
`SESSION_ENDED`, `SESSION_RESUMED`, `SESSION_SAVED`, medic/engineer keys such as
`MASTER_MEDIC`, `MIRACLE_WORKER`, `ZOMBIE_SUMMONER`, weapon keys such as
`BOOMSTICKS`, `SIDEARM_SLAYER`, `PISTOL_WHIPPED`): are kept on import. Triggers
exist for now only where they can be derived reliably from ESS: `PER_KILL` = every
kill, `SESSION_STARTED` = session start. `SESSION_ENDED`, `MAX_DOWN` (would need
the enemy's loadout ID) and the others are open. The complete key list of the real
packs is not known (no real file in the repo); unknown keys appear in the user
interface under “show all”.

### One event system: pack keys serve the base events

Every ESS event produces all matching sound events – own IDs and Recursion keys –
with their priority (`voice::choose`). They are checked in priority order across
all active packs (rank); the most specific match is played. The built-in “Standard
tones” are only the last resort: a match in an imported or own pack always beats
them, even when they have rank 1. Within the same level the priority decides, with
equal priority the pack key before the own ID, then the rank.

| ESS event | Order (specific → general) |
|---|---|
| Kill | `LUDICROUS_KILL` … `DOUBLE_KILL` (chain) > `KILLING_SPREE` … `GOD_LIKE` (streak) > `ROAD_KILL`/`KNIFE_FIGHT`/`FIRST_BLOOD`/`REVENGE` … > `HEAD_SHOT` (on a headshot) > `PER_KILL` > default `kill`/`headshot` |
| Own death | `BEING_FARMED`/`DEATH_STREAK` (if they apply) > `DEATH_INSULT` > default `death` |
| Teamkill (own) | `TEAM_KILLER` > default `teamkill` |
| Killed by a teammate | `WHOSE_SIDE` > `PREVENT_FF` (derived from the name) > default `teamkilled` |
| Session start | `SESSION_STARTED` > default `session_start` |

**Standard tones as fallback:** if at least one real pack is active, the built-in
standard tones no longer play by default (“Use built-in tones when the pack has no
sound”, `voice.builtin_fallback`, off by default); an event without a pack sound
then stays silent. Without an active pack they play as before. They never serve
Recursion keys (MUTUAL, FIRST_BLOOD …).

**Derived from the name, please check** (marked so in `triggers.default.toml`):
`HEAD_HUNTER` = 5 headshot kills in a row, `ROAD_RAGE` = kill chain of 3 or more
that ends with a roadkill, `DEATH_FROM_ABOVE` = kill from an aircraft,
`WHOSE_SIDE`/`PREVENT_FF` = killed by a teammate. Without a trigger stay
`SUICIDE_BOMBER`, `VIP_KILL`, `VIP_TEAM_KILL` (would need data the tracker does not
have; can be assigned through “Choose a condition…”) and `QUAD_DAMAGE` (meaning
unclear).

`PER_KILL` = every kill (most general pack key). `DEATH_INSULT` = own death: no
pack has `PER_DEATH`; the assignment is derived from the name and the use in the
packs, not documented by Recursion. `SESSION_ENDED`, `SESSION_RESUMED` and
`SESSION_SAVED` have no event yet.

**Kill chain** (several kills within the multikill window), two styles, set under
*Sounds → Events → Kill chain style* (`voice.chain_style`):

| Chain | Recursion classic | Unreal |
|---|---|---|
| 2 | `EXCELLENT` | `DOUBLE_KILL` |
| 3 | `OUTSTANDING` | `TRIPLE_KILL` |
| 4 | `SUPERB` | `MULTI_KILL` |
| 5 … 10 | `MULTI_KILL`, `MEGA_KILL`, `ULTRA_KILL`, `MONSTER_KILL`, `LUDICROUS_KILL`, `HOLY_SHIT` | `MEGA_KILL` … `HOLY_SHIT` (from 4) |
| from 11 or 10 | `HOLY_SHIT_2` | `HOLY_SHIT_2` |

The assignment of `EXCELLENT`/`OUTSTANDING`/`SUPERB` as chain 2/3/4 comes from the
user's memory of Recursion, not from documentation. “Automatic” (default) takes
the classic style if an active pack has a sound for `EXCELLENT`, otherwise Unreal.
The rules are in `triggers.default.toml` with `chain_style = "classic"` or
`"unreal"` and can be changed there.

In addition, a key can be chosen under *Sounds → Events* that plays on every kill
(“Use this pack key for every kill”, `voice.kill_key`), with the lowest priority –
for packs without `PER_KILL`. The Events page shows the base events at the top
(Kill, Headshot, Death, Teamkill, Teamkilled, Suicide, Session start) with an
expandable key chain: which key has a sound in which active pack and when it was
last played (since the program start). Below them the special achievements by
topic.

The log names the chain checked per event, e.g.
`voice: kill: tried DOUBLE_KILL(miss) HEAD_SHOT(miss) PER_KILL(hit, Deadpool v2: see ya.ogg) (chain style classic)`;
the line `voice: play` follows for what is played.

What triggers an event is not in the pack. The conditions are **own definitions**
from the ESS data, stored in `crates/ps2-tracker-core/assets/triggers.default.toml`.

**Changing conditions in the user interface** (since 0.2.0): *Sounds → Events*,
menu “…” of an entry → “Edit condition…” (or “Choose a condition…” without a
preset). Template = base fact (`on`), plus the filters below; several rules are
OR-combined. “Reset to default” takes the preset again, “No trigger” switches the
event off. Saved in `config.toml` under `[voice.trigger_overrides]`; these rules
replace the rules of *their* event, all others stay. Checked in Rust (known events
and facts, at most 8 rules per event, limits for lists and values).

Whole rule set as a file: copy `triggers.default.toml` to
`$XDG_CONFIG_HOME/ps2-tracker/triggers.toml` and adapt it; if it exists, it
replaces the default rule set completely (restart needed). Conditions changed in
the user interface apply on top of it.

Base events (`on`): `kill`, `first_kill`, `multikill`, `headshot_streak`,
`repeat_victim`, `revenge`, `mutual`, `streak`, `streak_ended`, `death`,
`nemesis`, `death_streak`, `suicide`, `teamkill`, `teamkilled`, `revive`,
`revived`, `vehicle_kill`, `achievement`, `login`, `session_start`.
Filters: `count`, `min_count`, `headshot`, `roadkill`, `weapon_category`,
`vehicle`, `session_count`, `priority`.

Own event IDs: `kill`, `headshot`, `multikill`, `streak`, `streak_ended`, `death`,
`suicide`, `teamkill`, `teamkilled`, `revive`, `revived`, `revenge`, `nemesis`,
`vehicle_kill`, `achievement`, `login`, `session_start`.

### Recursion keys with a trigger (35 of 96)

| Key | Trigger (own definition) |
|---|---|
| `HEAD_SHOT` | kill with a headshot |
| `ROAD_KILL` | kill from a vehicle without a weapon (`attacker_weapon_id` 0, `attacker_vehicle_id` set) – an assumption, to be checked with a real roadkill |
| `KILLING_SPREE` … `GOD_LIKE` | kill streak 5 / 10 / 15 / 20 / 25 (`KILLING_SPREE`, `DOMINATING`, `UNSTOPPABLE`, `WICKED_SICK`, `GOD_LIKE`) |
| `EXCELLENT`, `OUTSTANDING`, `SUPERB` | kill chain 2 / 3 / 4 in the style “Recursion classic” |
| `DOUBLE_KILL` … `HOLY_SHIT_2` | kill chain 2 … 10 and more (style “Unreal”; from 5 in both styles, see the table above) |
| `FIRST_BLOOD` | first kill of the session |
| `KNIFE_FIGHT` | kill with a weapon of the Census category “Knife” |
| `TEAM_KILLER` | teamkill done |
| `WHOSE_SIDE`, `PREVENT_FF` | killed by a teammate (derived from the name) |
| `DEATH_INSULT` | own death (most general pack key for deaths) |
| `REVENGE` | kill of the last own killer |
| `MUTUAL` | mutual kill with the same enemy within 3 s |
| `REPEAT_CUSTOMER` | 2nd kill of the same enemy in the session |
| `DOMINATION` | 4th kill of the same enemy in the session |
| `BEING_FARMED` | 3rd death by the same enemy in the session |
| `DEATH_STREAK` | 5 deaths without an own kill in between |
| `HEAD_HUNTER` | 5 headshot kills in a row (derived from the name, please check) |
| `ROAD_RAGE` | kill chain of 3 or more that ends with a roadkill (derived from the name) |
| `DEATH_FROM_ABOVE` | kill from an aircraft (derived from the name) |
| `ACE` | 5th destroyed aircraft (Mosquito, Reaver, Scythe, Dervish, Liberator, Galaxy, Valkyrie) in the session |

Plus the keys of the binary packs `PER_KILL` (every kill) and `SESSION_STARTED`
(session start). The assignment of the streak and multikill levels follows the
order in the pack and is an own decision.

### Not assigned yet (61 of 96)

These keys are imported and shown in the user interface, but trigger nothing until
a condition is set (not guessed, not read out of Recursion). On the page *Sounds →
Events* every entry has “Choose a condition…” in its menu: choose a template (base
fact), set parameters, save – without TOML. Deliberately open:

- `SUICIDE_BOMBER`: would need the link “own death by the own explosive + kill(s)
  with the same weapon in the same second”. ESS delivers the two Death events
  without a reliable order and without a common identifier; a preset would trigger
  wrongly.
- `VIP_KILL`, `VIP_TEAM_KILL`: neither ESS nor Census know a VIP marker for an
  enemy. (Experience “Priority Kill”/“High Priority Kill”, IDs 278/279, exists, but
  its meaning as “VIP” in Recursion is not established.)
- `QUAD_DAMAGE`: meaning unclear.

`WELCOME_KILL`, `HUMILIATION`, `PIZZA_DELIVERY`, `ONE_PLATOON`, `ONE_MAN_EMPIRE`, `ONE_MAN_ONE_MISSION`, `ONE_MAN_FARM`, `PAIN_REMAINS`, `WATCH_YOUR_STEP`, `ACCURACY`, `IMPRESSIVE`, `SHARP_SHOOTER`, `MARKSMAN`, `LOBOTOMIST`, `STEEL_RAIN`, `PRECIPICE`, `SUICIDE_BOMBER`, `ROCKET_PRIMARY`, `MASTER_JUGGLER`, `COMMISSIONER`, `EXECUTIONER`, `JUGGERNAUGHT`, `MAXIMUM_DAMAGE`, `GUN_BIGGER`, `LAWNMOWER`, `HARVESTER`, `PRESENT`, `ICED`, `FIRE`, `CARE_BEAR`, `QUAD_DAMAGE`, `TOP_GUN`, `SCOURGE_OF_SKIES`, `ARMORED_ASSAULT`, `BLITZKRIEG`, `BOMBARDIER`, `DECIMATION`, `FLYSWATTER`, `NICE_THINGS`, `KAMIKAZE`, `THINK_THEYD_LEARN`, `RECURSION`, `RECURSIVE_RECURSION`, `EXPLODING_FIST`, `EASY_MODE`, `HARD_MODE`, `NIGHTMARE`, `IMPRESS_MYSELF`, `HIGH_ROLLER`, `FATALITY`, `SUPERMAN`, `BATMAN`, `VIP_KILL`, `VIP_TEAM_KILL`, `CHEATER`, `RAGE_QUIT`, `SUBMISSION`, `SHOOT_NEEDLE`, `TERRORISTS`, `EXPLOSIVE_EFFICIENCY`, `FRAGASM`

Example of an own rule in `triggers.toml`:

```toml
[[trigger]]
event = "SHARP_SHOOTER"
on = "headshot_streak"
count = 5
priority = 60
description = "5 headshot kills in a row"
```

## Playback

- Per ESS event only the most important sound is played (highest priority).
- Queue: at most 1–4 sounds at the same time (adjustable); more important sounds
  interrupt less important ones; with mass kills at most two sounds wait, they
  expire after 1.5 s.
- A cooldown per entry prevents constant firing (standard pack: `kill` 150 ms).

## Own sound folders without a manifest

“Map your own sound folder …” searches a folder (up to depth 3) for audio files,
proposes events from the file names (e.g. `double`, `triple`, `headshot`,
`revenge`) and, after manual correction, creates a new pack in the own format. The
files are copied, the original stays unchanged.

## Standard pack

`standard-tones` (CC0) contains sine tones made by the project for all own events
and the assigned Recursion keys – no voice lines. Local TTS lines (e.g. Piper with
a free voice) would be possible, but no TTS was installed on the development
machine; they are therefore not included.
