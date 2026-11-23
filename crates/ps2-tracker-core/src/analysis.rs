//! Evaluation of a stored session for the Analytics page and the Excel export:
//! resolved names, per-minute time series, weapons, vehicles, opponents' teams.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::achievements::Unlocked;
use crate::census::NameCache;
use crate::classes::{Class, ClassStats, class_of_opt, class_stats};
use crate::session::{FeedEntry, Outcome, Rates, Session, SessionStats};

/// Longest time series (minutes) – a forgotten session cannot blow up the UI.
pub const MAX_MINUTES: i64 = 24 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponLine {
    pub id: u32,
    pub name: String,
    pub kills: u32,
    pub headshots: u32,
    pub hsr: f64,
    pub deaths_by: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleLine {
    pub id: u32,
    pub name: String,
    pub kills: u32,
}

/// One kill, death, teamkill or vehicle event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventLine {
    pub t: i64,
    pub kind: String,
    pub other_id: String,
    pub other_name: String,
    pub weapon: String,
    pub headshot: bool,
    pub vehicle: String,
    pub team: Option<u32>,
    /// Class of the other party (data version 4).
    #[serde(default)]
    pub enemy_class: Option<Class>,
    /// Own class at the time (data version 4).
    #[serde(default)]
    pub my_class: Option<Class>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportLine {
    pub t: i64,
    pub kind: String,
    pub other_id: String,
    pub other_name: String,
    pub detail: String,
}

/// Cumulative values at the end of a minute since the session start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub minute: i64,
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub xp: u64,
    pub kd: f64,
    pub hsr: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamLine {
    /// Team/faction ID from ESS (1 VS, 2 NC, 3 TR, 4 NSO); 0 = unknown.
    pub team_id: u32,
    pub kills: u32,
    pub deaths: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub id: Option<i64>,
    pub character_id: String,
    pub character_name: String,
    pub faction_id: Option<u32>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub data_version: u32,
    pub stats: SessionStats,
    pub rates: Rates,
    pub best_streak: u32,
    pub weapons: Vec<WeaponLine>,
    pub vehicles: Vec<VehicleLine>,
    pub kills: Vec<EventLine>,
    pub deaths: Vec<EventLine>,
    pub support: Vec<SupportLine>,
    pub achievements: Vec<Unlocked>,
    pub series: Vec<Point>,
    pub teams: Vec<TeamLine>,
    /// Kills/deaths by enemy class and own class use (data version 4).
    #[serde(default)]
    pub classes: ClassStats,
}

fn char_name(names: &NameCache, id: u64) -> String {
    names
        .characters
        .get(&id)
        .map(|(n, _)| n.clone())
        .unwrap_or_default()
}

fn item(names: &NameCache, id: u32) -> String {
    if id == 0 {
        return String::new();
    }
    names
        .items
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("#{id}"))
}

fn vehicle(names: &NameCache, id: u32) -> String {
    if id == 0 {
        return String::new();
    }
    names
        .vehicles
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("#{id}"))
}

fn event_line(e: &FeedEntry, me: u64, names: &NameCache) -> Option<(bool, EventLine)> {
    // (is_kill_side, line)
    let (kill_side, kind, other, weapon, headshot, veh) = match &e.outcome {
        Outcome::Kill {
            victim,
            weapon_id,
            vehicle_id,
            headshot,
        } => (true, "kill", *victim, *weapon_id, *headshot, *vehicle_id),
        Outcome::Teamkill { victim, weapon_id } => {
            (true, "teamkill", *victim, *weapon_id, false, 0)
        }
        Outcome::VehicleKill {
            vehicle_id,
            owner,
            weapon_id,
        } => (true, "vehicle_kill", *owner, *weapon_id, false, *vehicle_id),
        Outcome::Death {
            killer,
            weapon_id,
            vehicle_id,
            headshot,
        } => (false, "death", *killer, *weapon_id, *headshot, *vehicle_id),
        Outcome::Teamkilled { killer, weapon_id } => {
            (false, "teamkilled", *killer, *weapon_id, false, 0)
        }
        Outcome::Suicide { weapon_id } => (false, "suicide", me, *weapon_id, false, 0),
        Outcome::VehicleLost {
            vehicle_id,
            attacker,
        } => (false, "vehicle_lost", *attacker, 0, false, *vehicle_id),
        _ => return None,
    };
    Some((
        kill_side,
        EventLine {
            t: e.timestamp,
            kind: kind.into(),
            other_id: other.to_string(),
            other_name: char_name(names, other),
            weapon: item(names, weapon),
            headshot,
            vehicle: vehicle(names, veh),
            team: e.team,
            enemy_class: class_of_opt(e.other_loadout),
            my_class: class_of_opt(e.my_loadout),
        },
    ))
}

pub fn analyse(s: &Session, names: &NameCache) -> Analysis {
    let me = s.character_id;
    let end = s.ended_at.unwrap_or(s.last_event_at).max(s.started_at);
    let mut kills = Vec::new();
    let mut deaths = Vec::new();
    for e in &s.timeline {
        if let Some((k, line)) = event_line(e, me, names) {
            if k {
                kills.push(line)
            } else {
                deaths.push(line)
            }
        }
    }

    let support = s
        .events
        .iter()
        .filter_map(|e| {
            let (kind, other, detail) = match &e.outcome {
                Outcome::Revive { target } => ("revive", *target, String::new()),
                Outcome::Revived { by } => ("revived", *by, String::new()),
                Outcome::Assist { victim } => ("assist", *victim, String::new()),
                Outcome::FacilityCapture { facility_id } => ("capture", 0, facility_id.to_string()),
                Outcome::FacilityDefend { facility_id } => ("defend", 0, facility_id.to_string()),
                Outcome::BattleRankUp { battle_rank } => ("rank_up", 0, battle_rank.to_string()),
                Outcome::Streak { count } => ("streak", 0, count.to_string()),
                _ => return None,
            };
            Some(SupportLine {
                t: e.timestamp,
                kind: kind.into(),
                other_id: if other == 0 {
                    String::new()
                } else {
                    other.to_string()
                },
                other_name: if other == 0 {
                    String::new()
                } else {
                    char_name(names, other)
                },
                detail,
            })
        })
        .collect();

    // per-minute cumulative series
    let minutes = ((end - s.started_at) / 60).clamp(0, MAX_MINUTES);
    let minute_of = |t: i64| ((t - s.started_at).max(0) / 60).min(minutes);
    let mut k_at = vec![0u32; minutes as usize + 1];
    let mut d_at = vec![0u32; minutes as usize + 1];
    let mut h_at = vec![0u32; minutes as usize + 1];
    for e in &s.timeline {
        let m = minute_of(e.timestamp) as usize;
        match e.outcome {
            Outcome::Kill { headshot, .. } => {
                k_at[m] += 1;
                h_at[m] += u32::from(headshot);
            }
            Outcome::Death { .. } | Outcome::Teamkilled { .. } | Outcome::Suicide { .. } => {
                d_at[m] += 1
            }
            _ => {}
        }
    }
    let (mut k, mut d, mut h, mut xp) = (0u32, 0u32, 0u32, 0u64);
    let series = (0..=minutes)
        .map(|m| {
            let i = m as usize;
            k += k_at[i];
            d += d_at[i];
            h += h_at[i];
            xp += s.xp_by_minute.get(&m).copied().unwrap_or(0);
            Point {
                minute: m,
                kills: k,
                deaths: d,
                headshots: h,
                xp,
                kd: f64::from(k) / f64::from(d.max(1)),
                hsr: if k > 0 {
                    f64::from(h) / f64::from(k)
                } else {
                    0.0
                },
            }
        })
        .collect();

    let mut teams: BTreeMap<u32, TeamLine> = BTreeMap::new();
    for l in &kills {
        if l.kind == "kill" {
            let id = l.team.unwrap_or(0);
            teams
                .entry(id)
                .or_insert(TeamLine {
                    team_id: id,
                    kills: 0,
                    deaths: 0,
                })
                .kills += 1;
        }
    }
    for l in &deaths {
        if l.kind == "death" {
            let id = l.team.unwrap_or(0);
            teams
                .entry(id)
                .or_insert(TeamLine {
                    team_id: id,
                    kills: 0,
                    deaths: 0,
                })
                .deaths += 1;
        }
    }

    let mut weapons: Vec<WeaponLine> = s
        .weapons
        .iter()
        .map(|(id, w)| WeaponLine {
            id: *id,
            name: item(names, *id),
            kills: w.kills,
            headshots: w.headshots,
            hsr: if w.kills > 0 {
                f64::from(w.headshots) / f64::from(w.kills)
            } else {
                0.0
            },
            deaths_by: w.deaths_by,
        })
        .collect();
    weapons.sort_by(|a, b| b.kills.cmp(&a.kills).then(b.deaths_by.cmp(&a.deaths_by)));
    let mut vehicles: Vec<VehicleLine> = s
        .vehicle_kills_by_type
        .iter()
        .map(|(id, n)| VehicleLine {
            id: *id,
            name: vehicle(names, *id),
            kills: *n,
        })
        .collect();
    vehicles.sort_by_key(|v| std::cmp::Reverse(v.kills));

    Analysis {
        id: s.id,
        character_id: s.character_id.to_string(),
        character_name: s.character_name.clone(),
        faction_id: s.faction_id,
        started_at: s.started_at,
        ended_at: s.ended_at,
        data_version: s.version(),
        stats: s.stats.clone(),
        rates: s.rates(end),
        best_streak: s.best_streak,
        weapons,
        vehicles,
        kills,
        deaths,
        support,
        achievements: s.achievements.clone(),
        series,
        teams: teams.into_values().collect(),
        classes: class_stats(&s.timeline),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Death, GainExperience, GameEvent, xp};
    use crate::session::SessionConfig;

    const ME: u64 = 5428000000000000042;
    const FOE: u64 = 5429000000000000001;

    fn death(ts: i64, victim: u64, attacker: u64, hs: bool, vt: u32, at: u32) -> GameEvent {
        GameEvent::Death(Death {
            timestamp: ts,
            world_id: 10,
            zone_id: 2,
            character_id: victim,
            attacker_character_id: attacker,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 0,
            character_loadout_id: 0,
            is_headshot: hs,
            team_id: Some(vt),
            attacker_team_id: Some(at),
        })
    }

    #[test]
    fn builds_series_teams_and_names() {
        let mut s = Session::new(ME, "SampleSoldier", Some(3), 1000, SessionConfig::default());
        s.apply(&death(1010, FOE, ME, true, 1, 3)); // minute 0, VS
        s.apply(&death(1070, FOE, ME, false, 2, 3)); // minute 1, NC
        s.apply(&death(1130, ME, FOE, false, 3, 2)); // minute 2, death by NC
        s.apply(&GameEvent::GainExperience(GainExperience {
            timestamp: 1075,
            world_id: 10,
            zone_id: 2,
            character_id: ME,
            other_id: FOE,
            experience_id: xp::KILL_ASSIST,
            amount: 50,
            loadout_id: 0,
        }));
        s.end(1200);
        let mut names = NameCache::default();
        names.items.insert(80, "Gauss SAW".into());
        names.characters.insert(FOE, ("Enemy".into(), 1));
        let a = analyse(&s, &names);
        assert_eq!(a.kills.len(), 2);
        assert_eq!(a.kills[0].weapon, "Gauss SAW");
        assert_eq!(a.kills[0].other_name, "Enemy");
        assert_eq!(a.deaths.len(), 1);
        assert_eq!(a.support.len(), 1);
        assert_eq!(a.series.len(), 4); // minutes 0..=3
        let last = a.series.last().unwrap();
        assert_eq!(
            (last.kills, last.deaths, last.headshots, last.xp),
            (2, 1, 1, 50)
        );
        assert!((a.series[1].kd - 2.0).abs() < 1e-9);
        let nc = a.teams.iter().find(|t| t.team_id == 2).unwrap();
        assert_eq!((nc.kills, nc.deaths), (1, 1));
        assert_eq!(a.weapons[0].kills, 2);
    }

    #[test]
    fn caps_the_series_of_a_forgotten_session() {
        let mut s = Session::new(ME, "X", None, 0, SessionConfig::default());
        s.end(10 * 24 * 3600);
        assert_eq!(
            analyse(&s, &NameCache::default()).series.len(),
            MAX_MINUTES as usize + 1
        );
    }
}
