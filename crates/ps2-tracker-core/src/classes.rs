//! Infantry classes from the ESS loadout IDs (`attacker_loadout_id`,
//! `character_loadout_id`, `loadout_id`).
//!
//! The table `data/loadouts.json` comes from the Census collection
//! `ps2:v2/loadout` (24 loadouts: six classes for NC, TR, VS and NSO). The
//! frontend gets the class (`light_assault` …), not the loadout ID. Unknown
//! IDs (0 in old data, new loadouts) have no class – shown as "–", never
//! guessed.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Infiltrator,
    LightAssault,
    Medic,
    Engineer,
    HeavyAssault,
    Max,
}

impl Class {
    pub const ALL: [Class; 6] = [
        Class::Infiltrator,
        Class::LightAssault,
        Class::Medic,
        Class::Engineer,
        Class::HeavyAssault,
        Class::Max,
    ];

    /// Display name (English, as in game).
    pub fn name(self) -> &'static str {
        match self {
            Class::Infiltrator => "Infiltrator",
            Class::LightAssault => "Light Assault",
            Class::Medic => "Combat Medic",
            Class::Engineer => "Engineer",
            Class::HeavyAssault => "Heavy Assault",
            Class::Max => "MAX",
        }
    }

    /// Short form for narrow places and exports.
    pub fn short(self) -> &'static str {
        match self {
            Class::Infiltrator => "INF",
            Class::LightAssault => "LA",
            Class::Medic => "MED",
            Class::Engineer => "ENG",
            Class::HeavyAssault => "HA",
            Class::Max => "MAX",
        }
    }
}

#[derive(Deserialize)]
struct Entry {
    class: Class,
    #[allow(dead_code)]
    faction_id: u32,
}

#[derive(Deserialize)]
struct Table {
    loadouts: BTreeMap<u32, Entry>,
    /// Census `profile` image IDs (`/files/ps2/images/static/<id>.png`).
    #[serde(default)]
    class_images: BTreeMap<Class, u32>,
}

struct Parsed {
    loadouts: BTreeMap<u32, Class>,
    images: BTreeMap<Class, u32>,
}

fn parsed() -> &'static Parsed {
    static T: OnceLock<Parsed> = OnceLock::new();
    T.get_or_init(|| {
        let t: Table = serde_json::from_str(include_str!("../data/loadouts.json"))
            .expect("data/loadouts.json is valid (checked by a test)");
        Parsed {
            loadouts: t
                .loadouts
                .into_iter()
                .map(|(id, e)| (id, e.class))
                .collect(),
            images: t.class_images,
        }
    })
}

fn table() -> &'static BTreeMap<u32, Class> {
    &parsed().loadouts
}

/// Census image ID of the official class icon (only the number is built in;
/// the image is loaded at runtime, see `class_icons`).
pub fn image_id(class: Class) -> Option<u32> {
    parsed().images.get(&class).copied()
}

/// Class of a loadout ID; `None` for 0 and unknown IDs.
pub fn class_of(loadout_id: u32) -> Option<Class> {
    table().get(&loadout_id).copied()
}

/// Class of an optional loadout (as stored in a session).
pub fn class_of_opt(loadout_id: Option<u32>) -> Option<Class> {
    loadout_id.and_then(class_of)
}

/// Kills and deaths against one enemy class (`class: None` = unknown, e.g.
/// sessions recorded before data version 4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyClassRow {
    pub class: Option<Class>,
    /// Kills of enemies playing this class.
    pub kills: u32,
    /// Deaths by enemies playing this class.
    pub deaths: u32,
}

/// Own class use in a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnClassRow {
    pub class: Option<Class>,
    pub kills: u32,
    pub deaths: u32,
    /// Approximate time in this class: gaps between consecutive kills/deaths
    /// played in the same class (a class change in between is not counted).
    pub secs: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassStats {
    /// Sorted by kills + deaths, unknown last.
    pub enemy: Vec<EnemyClassRow>,
    pub own: Vec<OwnClassRow>,
}

/// Class evaluation of a timeline (kills and deaths only).
pub fn class_stats(timeline: &[crate::session::FeedEntry]) -> ClassStats {
    use crate::session::Outcome;
    let mut enemy: BTreeMap<Option<Class>, EnemyClassRow> = BTreeMap::new();
    let mut own: BTreeMap<Option<Class>, OwnClassRow> = BTreeMap::new();
    let mut prev: Option<(i64, Class)> = None;
    for e in timeline {
        let kill = matches!(e.outcome, Outcome::Kill { .. });
        let death = matches!(e.outcome, Outcome::Death { .. });
        if !kill && !death {
            continue;
        }
        let other = class_of_opt(e.other_loadout);
        let mine = class_of_opt(e.my_loadout);
        let r = enemy.entry(other).or_insert_with(|| EnemyClassRow {
            class: other,
            ..Default::default()
        });
        let o = own.entry(mine).or_insert_with(|| OwnClassRow {
            class: mine,
            ..Default::default()
        });
        if kill {
            r.kills += 1;
            o.kills += 1;
        } else {
            r.deaths += 1;
            o.deaths += 1;
        }
        if let (Some((t, c)), Some(m)) = (prev, mine)
            && c == m
        {
            o.secs += (e.timestamp - t).max(0);
        }
        prev = mine.map(|m| (e.timestamp, m));
    }
    let order = |c: &Option<Class>, n: u32| (c.is_none(), std::cmp::Reverse(n));
    let mut enemy: Vec<_> = enemy.into_values().collect();
    enemy.sort_by_key(|r| (order(&r.class, r.kills + r.deaths), r.class));
    let mut own: Vec<_> = own.into_values().collect();
    own.sort_by_key(|r| (order(&r.class, r.kills + r.deaths), r.class));
    ClassStats { enemy, own }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_image_ids() {
        let ids: Vec<Option<u32>> = Class::ALL.iter().map(|c| image_id(*c)).collect();
        assert_eq!(
            ids,
            [
                Some(204),
                Some(62),
                Some(65),
                Some(201),
                Some(59),
                Some(207)
            ]
        );
    }

    #[test]
    fn table_from_census() {
        // all 24 loadouts of the Census collection (Nov 2026)
        let expect = [
            (1, Class::Infiltrator),
            (3, Class::LightAssault),
            (4, Class::Medic),
            (5, Class::Engineer),
            (6, Class::HeavyAssault),
            (7, Class::Max),
            (8, Class::Infiltrator),
            (10, Class::LightAssault),
            (11, Class::Medic),
            (12, Class::Engineer),
            (13, Class::HeavyAssault),
            (14, Class::Max),
            (15, Class::Infiltrator),
            (17, Class::LightAssault),
            (18, Class::Medic),
            (19, Class::Engineer),
            (20, Class::HeavyAssault),
            (21, Class::Max),
            (28, Class::Infiltrator),
            (29, Class::LightAssault),
            (30, Class::Medic),
            (31, Class::Engineer),
            (32, Class::HeavyAssault),
            (45, Class::Max),
        ];
        assert_eq!(table().len(), expect.len());
        for (id, c) in expect {
            assert_eq!(class_of(id), Some(c), "loadout {id}");
        }
        assert_eq!(class_of(0), None);
        assert_eq!(class_of(2), None); // gaps in the Census IDs
        assert_eq!(class_of(9999), None);
        assert_eq!(Class::Medic.short(), "MED");
        assert_eq!(Class::Max.name(), "MAX");
    }

    #[test]
    fn stats_from_a_timeline() {
        use crate::session::{FeedEntry, Outcome};
        let kill = |t, other, mine| FeedEntry {
            timestamp: t,
            outcome: Outcome::Kill {
                victim: 1,
                weapon_id: 0,
                vehicle_id: 0,
                headshot: false,
            },
            team: None,
            other_loadout: other,
            my_loadout: mine,
        };
        let death = |t, other, mine| FeedEntry {
            outcome: Outcome::Death {
                killer: 1,
                weapon_id: 0,
                vehicle_id: 0,
                headshot: false,
            },
            ..kill(t, other, mine)
        };
        // own: HA (6) kills two medics (18, 11) and an INF (15), dies to a MAX (21);
        // then as medic (4) kills an LA (29); an old entry without loadouts
        let tl = vec![
            kill(100, Some(18), Some(6)),
            kill(130, Some(11), Some(6)),
            kill(150, Some(15), Some(6)),
            death(200, Some(21), Some(6)),
            kill(400, Some(29), Some(4)),
            kill(500, None, None),
        ];
        let s = class_stats(&tl);
        let e = |c| s.enemy.iter().find(|r| r.class == c).unwrap().clone();
        assert_eq!(s.enemy[0].class, Some(Class::Medic));
        assert_eq!(
            (e(Some(Class::Medic)).kills, e(Some(Class::Medic)).deaths),
            (2, 0)
        );
        assert_eq!(e(Some(Class::Max)).deaths, 1);
        assert_eq!(s.enemy.last().unwrap().class, None);
        let o = |c| s.own.iter().find(|r| r.class == c).unwrap().clone();
        let ha = o(Some(Class::HeavyAssault));
        assert_eq!((ha.kills, ha.deaths, ha.secs), (3, 1, 100));
        let med = o(Some(Class::Medic));
        assert_eq!((med.kills, med.secs), (1, 0)); // class change: no time
        assert_eq!(s.own[0].class, Some(Class::HeavyAssault));
    }

    /// Compares the table with the live Census collection (network; run with
    /// `cargo test -p ps2-tracker-core -- --ignored loadouts_match_census`).
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn loadouts_match_census() {
        let url = "https://census.daybreakgames.com/s:example/get/ps2:v2/loadout?c:limit=100";
        let v: serde_json::Value = reqwest::get(url).await.unwrap().json().await.unwrap();
        let list = v["loadout_list"].as_array().unwrap();
        assert_eq!(list.len(), table().len());
        for l in list {
            let id: u32 = l["loadout_id"].as_str().unwrap().parse().unwrap();
            let name = l["code_name"].as_str().unwrap();
            let class = class_of(id).unwrap_or_else(|| panic!("loadout {id} ({name}) missing"));
            let suffix = match class {
                Class::Medic => "Medic",
                c => c.name(),
            };
            assert!(name.ends_with(suffix), "{id}: {name} vs {class:?}");
        }
    }
}
