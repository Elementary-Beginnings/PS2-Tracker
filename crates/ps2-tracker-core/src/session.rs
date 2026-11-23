//! Session logic: processes a character's events and keeps count.
//!
//! Pure and deterministic (no clock, no I/O): timestamps come from the events or
//! are passed as `now`, so everything can be tested with fixtures.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::events::{CharacterId, GameEvent, xp};

/// What an event means for the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outcome {
    Kill {
        victim: CharacterId,
        weapon_id: u32,
        vehicle_id: u32,
        headshot: bool,
    },
    Death {
        killer: CharacterId,
        weapon_id: u32,
        vehicle_id: u32,
        headshot: bool,
    },
    Teamkill {
        victim: CharacterId,
        weapon_id: u32,
    },
    Teamkilled {
        killer: CharacterId,
        weapon_id: u32,
    },
    Suicide {
        weapon_id: u32,
    },
    VehicleKill {
        vehicle_id: u32,
        owner: CharacterId,
        weapon_id: u32,
    },
    VehicleLost {
        vehicle_id: u32,
        attacker: CharacterId,
    },
    Revive {
        target: u64,
    },
    Revived {
        by: CharacterId,
    },
    Assist {
        victim: u64,
    },
    /// Kill streak reached a milestone (e.g. 5, 10, …).
    Streak {
        count: u32,
    },
    StreakEnded {
        count: u32,
    },
    FacilityCapture {
        facility_id: u32,
    },
    FacilityDefend {
        facility_id: u32,
    },
    BattleRankUp {
        battle_rank: u32,
    },
    Login,
    Logout,
}

impl Outcome {
    /// The other character and the weapon of a kill-feed outcome, for name
    /// lookups. `None` for outcomes without another player.
    pub fn other_and_weapon(&self) -> Option<(CharacterId, u32)> {
        match *self {
            Outcome::Kill {
                victim, weapon_id, ..
            }
            | Outcome::Teamkill { victim, weapon_id } => Some((victim, weapon_id)),
            Outcome::Death {
                killer, weapon_id, ..
            }
            | Outcome::Teamkilled { killer, weapon_id } => Some((killer, weapon_id)),
            Outcome::VehicleKill {
                owner, weapon_id, ..
            } => Some((owner, weapon_id)),
            Outcome::VehicleLost { attacker, .. } => Some((attacker, 0)),
            _ => None,
        }
    }
}

/// Entry in the kill feed or the timeline of a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedEntry {
    pub timestamp: i64,
    #[serde(flatten)]
    pub outcome: Outcome,
    /// Team (faction) of the other party of a kill/death/vehicle event, from
    /// the ESS payload; `None` in sessions recorded before data version 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<u32>,
    /// Loadout of the other party at the time (kill: the victim, death: the
    /// attacker), see [`crate::classes`]; data version 4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_loadout: Option<u32>,
    /// Own loadout at the time; data version 4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub my_loadout: Option<u32>,
}

/// Version of the session data written by this build. 1 = up to 0.2.0 betas
/// (only kills/deaths/vehicles in the timeline), 2 = plus support events,
/// XP per minute, achievements and the opponent's team, 3 = plus the last
/// server (`world_id`) and continent (`zone_id`), 4 = plus the loadouts
/// (classes) of both sides of a kill/death and the last own loadout.
pub const SESSION_DATA_VERSION: u32 = 4;

/// Server and continent of an event of character `me` (`None`: not part of
/// the event, or 0). Dynamic zones carry an instance in the upper 16 bits of
/// `zone_id`; the lower 16 bits are the continent.
pub fn location(ev: &GameEvent, me: CharacterId) -> (Option<u32>, Option<u32>) {
    let nz = |v: u32| (v != 0).then_some(v);
    let zone = |z: u32| nz(z & 0xFFFF);
    match ev {
        GameEvent::Death(d) if d.character_id == me || d.attacker_character_id == me => {
            (nz(d.world_id), zone(d.zone_id))
        }
        GameEvent::VehicleDestroy(v) if v.character_id == me || v.attacker_character_id == me => {
            (nz(v.world_id), zone(v.zone_id))
        }
        GameEvent::GainExperience(g) if g.character_id == me || g.other_id == me => {
            (nz(g.world_id), zone(g.zone_id))
        }
        GameEvent::PlayerFacilityCapture(p) | GameEvent::PlayerFacilityDefend(p)
            if p.character_id == me =>
        {
            (nz(p.world_id), zone(p.zone_id))
        }
        GameEvent::PlayerLogin(p) | GameEvent::PlayerLogout(p) if p.character_id == me => {
            (nz(p.world_id), None)
        }
        _ => (None, None),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStats {
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub teamkills: u32,
    pub teamkilled: u32,
    pub suicides: u32,
    pub revives: u32,
    pub revived: u32,
    /// Deaths undone by a revive (a revive counts only against an earlier death
    /// of the same session, never more than the deaths).
    #[serde(default)]
    pub revived_deaths: u32,
    pub assists: u32,
    pub xp: u64,
    pub vehicle_kills: u32,
    pub vehicles_lost: u32,
    pub facility_captures: u32,
    pub facility_defends: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponStats {
    pub kills: u32,
    pub headshots: u32,
    /// Deaths by this weapon (used by the enemy).
    pub deaths_by: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Interval of streak milestones (5 → 5, 10, 15 …).
    pub streak_step: u32,
    /// Length of the kill feed in the overlay.
    pub killfeed_len: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            streak_step: 5,
            killfeed_len: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Database ID once saved.
    pub id: Option<i64>,
    pub character_id: CharacterId,
    pub character_name: String,
    pub faction_id: Option<u32>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub last_event_at: i64,
    pub stats: SessionStats,
    pub weapons: BTreeMap<u32, WeaponStats>,
    pub vehicle_kills_by_type: BTreeMap<u32, u32>,
    pub streak: u32,
    pub best_streak: u32,
    pub headshot_streak: u32,
    pub last_killer: Option<CharacterId>,
    /// All kills/deaths/vehicle events in chronological order.
    pub timeline: Vec<FeedEntry>,
    /// Data version (see [`SESSION_DATA_VERSION`]); 0 in old data = 1.
    #[serde(default)]
    pub data_version: u32,
    /// Revives, assists, captures, rank-ups and streak milestones (data version 2).
    #[serde(default)]
    pub events: Vec<FeedEntry>,
    /// XP per minute since the session start (data version 2).
    #[serde(default)]
    pub xp_by_minute: BTreeMap<i64, u64>,
    /// Achievements unlocked in this session (data version 2).
    #[serde(default)]
    pub achievements: Vec<crate::achievements::Unlocked>,
    /// Last server seen in an event of this character (data version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_id: Option<u32>,
    /// Last continent (zone definition) seen (data version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<u32>,
    /// Last own loadout seen in an event (class, data version 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loadout_id: Option<u32>,
    /// Logged out, waiting for a relogin within the resume window: time of
    /// the logout. The session continues if the character comes back in time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_at: Option<i64>,
    /// Seconds spent logged out between logout and relogin (not counted as
    /// playtime in the rates).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub paused_secs: i64,
    #[serde(skip)]
    killfeed: VecDeque<FeedEntry>,
    #[serde(skip, default)]
    config: SessionConfig,
}

/// Derived rates for display and comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rates {
    pub duration_secs: i64,
    pub kd: f64,
    /// K/D as PlanetSide 2 shows it in game: a death you were revived from
    /// does not count. kills / max(1, deaths - revived_deaths)
    #[serde(default)]
    pub kd_revived: f64,
    pub kpm: f64,
    pub hsr: f64,
    pub xp_per_hour: f64,
}

impl Session {
    pub fn new(
        character_id: CharacterId,
        character_name: impl Into<String>,
        faction_id: Option<u32>,
        started_at: i64,
        config: SessionConfig,
    ) -> Self {
        Self {
            id: None,
            character_id,
            character_name: character_name.into(),
            faction_id,
            started_at,
            ended_at: None,
            last_event_at: started_at,
            stats: SessionStats::default(),
            weapons: BTreeMap::new(),
            vehicle_kills_by_type: BTreeMap::new(),
            streak: 0,
            best_streak: 0,
            headshot_streak: 0,
            last_killer: None,
            timeline: Vec::new(),
            data_version: SESSION_DATA_VERSION,
            events: Vec::new(),
            xp_by_minute: BTreeMap::new(),
            achievements: Vec::new(),
            world_id: None,
            zone_id: None,
            loadout_id: None,
            paused_at: None,
            paused_secs: 0,
            killfeed: VecDeque::new(),
            config,
        }
    }

    pub fn killfeed(&self) -> impl DoubleEndedIterator<Item = &FeedEntry> {
        self.killfeed.iter()
    }

    /// Restores configuration and kill feed after loading from the database.
    pub fn restore(&mut self, config: SessionConfig) {
        self.config = config;
        self.killfeed = self
            .timeline
            .iter()
            .rev()
            .take(config.killfeed_len)
            .rev()
            .cloned()
            .collect();
    }

    pub fn end(&mut self, at: i64) {
        self.ended_at = Some(at.max(self.started_at));
        self.paused_at = None;
    }

    /// Logout with a resume window: the clock stops at `at`.
    pub fn pause(&mut self, at: i64) {
        if self.paused_at.is_none() {
            self.paused_at = Some(at.max(self.started_at));
        }
    }

    /// Relogin within the window: the time logged out is not playtime.
    pub fn resume(&mut self, at: i64) {
        if let Some(p) = self.paused_at.take() {
            self.paused_secs += (at - p).max(0);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub fn rates(&self, now: i64) -> Rates {
        let end = self
            .ended_at
            .or(self.paused_at)
            .unwrap_or(now)
            .max(self.started_at);
        let duration_secs = (end - self.started_at - self.paused_secs).max(0);
        let s = &self.stats;
        let minutes = duration_secs as f64 / 60.0;
        Rates {
            duration_secs,
            kd: ratio(s.kills as f64, s.deaths as f64),
            kd_revived: s.kills as f64
                / f64::from(s.deaths.saturating_sub(s.revived_deaths).max(1)),
            kpm: if minutes >= 1.0 {
                s.kills as f64 / minutes
            } else {
                0.0
            },
            hsr: if s.kills > 0 {
                s.headshots as f64 / s.kills as f64
            } else {
                0.0
            },
            xp_per_hour: if minutes >= 1.0 {
                s.xp as f64 / (minutes / 60.0)
            } else {
                0.0
            },
        }
    }

    fn push(
        &mut self,
        timestamp: i64,
        outcome: Outcome,
        feed: bool,
        team: Option<u32>,
        loadouts: (Option<u32>, Option<u32>),
    ) {
        let keep = matches!(
            outcome,
            Outcome::Revive { .. }
                | Outcome::Revived { .. }
                | Outcome::Assist { .. }
                | Outcome::FacilityCapture { .. }
                | Outcome::FacilityDefend { .. }
                | Outcome::BattleRankUp { .. }
                | Outcome::Streak { .. }
        );
        let entry = FeedEntry {
            timestamp,
            outcome,
            team: if feed { team } else { None },
            other_loadout: if feed { loadouts.0 } else { None },
            my_loadout: if feed { loadouts.1 } else { None },
        };
        if feed {
            self.killfeed.push_back(entry.clone());
            while self.killfeed.len() > self.config.killfeed_len.max(1) {
                self.killfeed.pop_front();
            }
            self.timeline.push(entry);
        } else if keep {
            self.events.push(entry);
        }
    }

    /// Data version of this session (old data without the field = 1).
    pub fn version(&self) -> u32 {
        self.data_version.max(1)
    }

    fn same_team(a: Option<u32>, b: Option<u32>) -> bool {
        matches!((a, b), (Some(x), Some(y)) if x == y)
    }

    fn end_streak(&mut self, out: &mut Vec<Outcome>) {
        if self.streak >= self.config.streak_step && self.config.streak_step > 0 {
            out.push(Outcome::StreakEnded { count: self.streak });
        }
        self.streak = 0;
        self.headshot_streak = 0;
    }

    /// Processes an event and returns what it means for this session.
    /// Events of other characters are ignored (empty list).
    pub fn apply(&mut self, event: &GameEvent) -> Vec<Outcome> {
        let me = self.character_id;
        let mut out = Vec::new();
        let ts = event.timestamp();
        let (world, zone) = location(event, me);
        if world.is_some() {
            self.world_id = world;
        }
        if zone.is_some() {
            self.zone_id = zone;
        }
        // team of the other party (kill/death/vehicle) for the timeline
        let other_team = match event {
            GameEvent::Death(d) if d.character_id == me => d.attacker_team_id,
            GameEvent::Death(d) => d.team_id,
            GameEvent::VehicleDestroy(v) if v.character_id == me => v.attacker_team_id,
            GameEvent::VehicleDestroy(v) => v.team_id.or(v.faction_id),
            _ => None,
        };
        // loadouts (classes): (other party, me); 0 = unknown
        let nz = |v: u32| (v != 0).then_some(v);
        let loadouts = match event {
            GameEvent::Death(d) if d.character_id == me => {
                let other = d.attacker_character_id;
                let other = (other != me && other != 0)
                    .then(|| nz(d.attacker_loadout_id))
                    .flatten();
                (other, nz(d.character_loadout_id))
            }
            GameEvent::Death(d) if d.attacker_character_id == me => {
                (nz(d.character_loadout_id), nz(d.attacker_loadout_id))
            }
            GameEvent::GainExperience(g) if g.character_id == me => (None, nz(g.loadout_id)),
            _ => (None, None),
        };
        if loadouts.1.is_some() {
            self.loadout_id = loadouts.1;
        }
        match event {
            GameEvent::Death(d) => {
                let attacker = d.attacker_character_id;
                if d.character_id == me {
                    self.stats.deaths += 1;
                    if attacker == me || attacker == 0 {
                        self.stats.suicides += 1;
                        out.push(Outcome::Suicide {
                            weapon_id: d.attacker_weapon_id,
                        });
                    } else if Self::same_team(d.team_id, d.attacker_team_id) {
                        self.stats.teamkilled += 1;
                        out.push(Outcome::Teamkilled {
                            killer: attacker,
                            weapon_id: d.attacker_weapon_id,
                        });
                    } else {
                        self.last_killer = Some(attacker);
                        self.weapons
                            .entry(d.attacker_weapon_id)
                            .or_default()
                            .deaths_by += 1;
                        out.push(Outcome::Death {
                            killer: attacker,
                            weapon_id: d.attacker_weapon_id,
                            vehicle_id: d.attacker_vehicle_id,
                            headshot: d.is_headshot,
                        });
                    }
                    self.end_streak(&mut out);
                } else if attacker == me {
                    if Self::same_team(d.team_id, d.attacker_team_id) {
                        self.stats.teamkills += 1;
                        out.push(Outcome::Teamkill {
                            victim: d.character_id,
                            weapon_id: d.attacker_weapon_id,
                        });
                    } else {
                        self.stats.kills += 1;
                        let w = self.weapons.entry(d.attacker_weapon_id).or_default();
                        w.kills += 1;
                        if d.is_headshot {
                            w.headshots += 1;
                            self.stats.headshots += 1;
                            self.headshot_streak += 1;
                        } else {
                            self.headshot_streak = 0;
                        }
                        self.streak += 1;
                        self.best_streak = self.best_streak.max(self.streak);
                        out.push(Outcome::Kill {
                            victim: d.character_id,
                            weapon_id: d.attacker_weapon_id,
                            vehicle_id: d.attacker_vehicle_id,
                            headshot: d.is_headshot,
                        });
                        let step = self.config.streak_step;
                        if step > 0 && self.streak.is_multiple_of(step) {
                            out.push(Outcome::Streak { count: self.streak });
                        }
                    }
                }
            }
            GameEvent::VehicleDestroy(v) => {
                if v.attacker_character_id == me && v.character_id != me {
                    if !Self::same_team(v.team_id, v.attacker_team_id) {
                        self.stats.vehicle_kills += 1;
                        *self.vehicle_kills_by_type.entry(v.vehicle_id).or_default() += 1;
                        out.push(Outcome::VehicleKill {
                            vehicle_id: v.vehicle_id,
                            owner: v.character_id,
                            weapon_id: v.attacker_weapon_id,
                        });
                    }
                } else if v.character_id == me {
                    self.stats.vehicles_lost += 1;
                    out.push(Outcome::VehicleLost {
                        vehicle_id: v.vehicle_id,
                        attacker: v.attacker_character_id,
                    });
                }
            }
            GameEvent::GainExperience(g) => {
                if g.character_id == me {
                    self.stats.xp += u64::from(g.amount);
                    let minute = (ts - self.started_at).max(0) / 60;
                    *self.xp_by_minute.entry(minute).or_default() += u64::from(g.amount);
                    if xp::is_revive(g.experience_id) {
                        self.stats.revives += 1;
                        out.push(Outcome::Revive { target: g.other_id });
                    } else if xp::is_assist(g.experience_id) {
                        self.stats.assists += 1;
                        out.push(Outcome::Assist { victim: g.other_id });
                    }
                } else if g.other_id == me && xp::is_revive(g.experience_id) {
                    self.stats.revived += 1;
                    if self.stats.revived_deaths < self.stats.deaths {
                        self.stats.revived_deaths += 1;
                    }
                    out.push(Outcome::Revived { by: g.character_id });
                }
            }
            GameEvent::PlayerFacilityCapture(p) if p.character_id == me => {
                self.stats.facility_captures += 1;
                out.push(Outcome::FacilityCapture {
                    facility_id: p.facility_id,
                });
            }
            GameEvent::PlayerFacilityDefend(p) if p.character_id == me => {
                self.stats.facility_defends += 1;
                out.push(Outcome::FacilityDefend {
                    facility_id: p.facility_id,
                });
            }
            GameEvent::BattleRankUp(b) if b.character_id == me => {
                out.push(Outcome::BattleRankUp {
                    battle_rank: b.battle_rank,
                });
            }
            GameEvent::PlayerLogin(p) if p.character_id == me => out.push(Outcome::Login),
            GameEvent::PlayerLogout(p) if p.character_id == me => out.push(Outcome::Logout),
            _ => {}
        }
        if !out.is_empty() {
            self.last_event_at = self.last_event_at.max(ts);
        }
        for o in &out {
            let feed = matches!(
                o,
                Outcome::Kill { .. }
                    | Outcome::Death { .. }
                    | Outcome::Teamkill { .. }
                    | Outcome::Teamkilled { .. }
                    | Outcome::Suicide { .. }
                    | Outcome::VehicleKill { .. }
                    | Outcome::VehicleLost { .. }
            );
            self.push(ts, o.clone(), feed, other_team, loadouts);
        }
        out
    }
}

fn ratio(a: f64, b: f64) -> f64 {
    if b > 0.0 { a / b } else { a }
}

fn is_zero(v: &i64) -> bool {
    *v == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 kills, 3 deaths, 2 revives received: true K/D 1.33, with revives 4.00.
    /// Revive experience IDs 7 (Revive) and 53 (Squad Revive), checked against
    /// Census `experience`.
    #[test]
    fn kd_with_revives() {
        let mut s = session();
        let ge = |c: u64, o: u64, id: u32| {
            GameEvent::GainExperience(GainExperience {
                timestamp: 5,
                world_id: 10,
                zone_id: 2,
                character_id: c,
                other_id: o,
                experience_id: id,
                amount: 75,
                loadout_id: 0,
            })
        };
        // a revive before any death does not count
        s.apply(&ge(MATE, ME, xp::REVIVE));
        for i in 0..3 {
            s.apply(&death(1100 + i, ME, FOE, false, 3, 1));
        }
        for i in 0..4 {
            s.apply(&death(1200 + i, FOE, ME, false, 1, 3));
        }
        s.apply(&ge(MATE, ME, xp::REVIVE));
        s.apply(&ge(MATE, ME, xp::SQUAD_REVIVE));
        let r = s.rates(2000);
        assert_eq!((s.stats.kills, s.stats.deaths, s.stats.revived), (4, 3, 3));
        assert_eq!(s.stats.revived_deaths, 2);
        assert!((r.kd - 4.0 / 3.0).abs() < 1e-9);
        assert!((r.kd_revived - 4.0).abs() < 1e-9);
        // never more revives than deaths
        s.apply(&ge(MATE, ME, xp::REVIVE));
        s.apply(&ge(MATE, ME, xp::REVIVE));
        assert_eq!(s.stats.revived_deaths, 3);
        assert!((s.rates(2000).kd_revived - 4.0).abs() < 1e-9);
    }
    use crate::events::{Death, GainExperience, VehicleDestroy};

    const ME: u64 = 5428000000000000042;
    const FOE: u64 = 5429000000000000001;
    const MATE: u64 = 5428000000000000099;

    fn death(ts: i64, victim: u64, attacker: u64, hs: bool, vt: u32, at: u32) -> GameEvent {
        GameEvent::Death(Death {
            timestamp: ts,
            world_id: 10,
            zone_id: 2,
            character_id: victim,
            attacker_character_id: attacker,
            attacker_weapon_id: 7214,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 15,
            character_loadout_id: 1,
            is_headshot: hs,
            team_id: Some(vt),
            attacker_team_id: Some(at),
        })
    }

    fn session() -> Session {
        Session::new(ME, "SampleSoldier", Some(3), 1000, SessionConfig::default())
    }

    #[test]
    fn data_version_3_keeps_server_and_continent() {
        let mut s = session();
        assert_eq!((s.world_id, s.zone_id), (None, None));
        s.apply(&GameEvent::PlayerLogin(crate::events::PlayerSession {
            timestamp: 1000,
            world_id: 10,
            character_id: ME,
        }));
        assert_eq!((s.world_id, s.zone_id), (Some(10), None));
        s.apply(&death(1010, FOE, ME, false, 1, 3));
        assert_eq!((s.world_id, s.zone_id), (Some(10), Some(2)));
        // dynamic zone: instance bits are dropped, the continent stays
        let mut ev = death(1020, FOE, ME, false, 1, 3);
        if let GameEvent::Death(d) = &mut ev {
            d.zone_id = (7 << 16) | 344;
        }
        s.apply(&ev);
        assert_eq!(s.zone_id, Some(344));
        // events of others do not move the location
        let mut other = death(1030, FOE, 77, false, 1, 3);
        if let GameEvent::Death(d) = &mut other {
            d.world_id = 13;
            d.zone_id = 4;
        }
        s.apply(&other);
        assert_eq!((s.world_id, s.zone_id), (Some(10), Some(344)));
        // sessions of data version 2 have no location and stay readable
        let mut old = serde_json::to_value(&s).unwrap();
        let o = old.as_object_mut().unwrap();
        o.remove("world_id");
        o.remove("zone_id");
        o.insert("data_version".into(), 2.into());
        let back: Session = serde_json::from_value(old).unwrap();
        assert_eq!(
            (back.version(), back.world_id, back.zone_id),
            (2, None, None)
        );
    }

    /// Synthetic ESS deaths with loadouts: VS Heavy Assault (20) kills a TR
    /// Medic (11), then dies to an NC MAX (7); a revive tick as Medic (18).
    #[test]
    fn data_version_4_keeps_classes() {
        use crate::classes::{Class, class_of_opt};
        assert_eq!(SESSION_DATA_VERSION, 4);
        let mut s = session();
        let with = |mut ev: GameEvent, attacker_lo: u32, victim_lo: u32| {
            if let GameEvent::Death(d) = &mut ev {
                d.attacker_loadout_id = attacker_lo;
                d.character_loadout_id = victim_lo;
            }
            ev
        };
        s.apply(&with(death(1010, FOE, ME, false, 1, 3), 20, 11));
        s.apply(&with(death(1020, ME, FOE, false, 3, 2), 7, 20));
        let k = &s.timeline[0];
        assert_eq!(class_of_opt(k.other_loadout), Some(Class::Medic));
        assert_eq!(class_of_opt(k.my_loadout), Some(Class::HeavyAssault));
        let d = &s.timeline[1];
        assert_eq!(class_of_opt(d.other_loadout), Some(Class::Max));
        assert_eq!(class_of_opt(d.my_loadout), Some(Class::HeavyAssault));
        assert_eq!(class_of_opt(s.loadout_id), Some(Class::HeavyAssault));
        s.apply(&GameEvent::GainExperience(crate::events::GainExperience {
            timestamp: 1030,
            world_id: 10,
            zone_id: 2,
            character_id: ME,
            other_id: 0,
            experience_id: 1,
            amount: 10,
            loadout_id: 18,
        }));
        assert_eq!(class_of_opt(s.loadout_id), Some(Class::Medic));
        // a suicide has no other party
        s.apply(&with(death(1040, ME, ME, false, 3, 3), 18, 18));
        assert_eq!(s.timeline[2].other_loadout, None);
        // the loadouts survive a save; old entries without them stay readable
        let json = serde_json::to_string(&s.timeline[0]).unwrap();
        assert!(json.contains("\"other_loadout\":11") && json.contains("\"my_loadout\":20"));
        let old: FeedEntry =
            serde_json::from_str(r#"{"timestamp":1,"type":"suicide","weapon_id":0}"#).unwrap();
        assert_eq!((old.other_loadout, old.my_loadout), (None, None));
    }

    #[test]
    fn counts_kills_deaths_headshots_and_streaks() {
        let mut s = session();
        for i in 0..5 {
            let o = s.apply(&death(1000 + i, FOE, ME, i % 2 == 0, 1, 3));
            assert!(matches!(o[0], Outcome::Kill { .. }));
        }
        // 5th kill reaches a milestone
        assert_eq!(s.streak, 5);
        assert_eq!(s.stats.headshots, 3);
        let o = s.apply(&death(1010, ME, FOE, false, 3, 1));
        assert!(matches!(o[0], Outcome::Death { .. }));
        assert_eq!(o[1], Outcome::StreakEnded { count: 5 });
        assert_eq!(s.streak, 0);
        assert_eq!(s.best_streak, 5);
        assert_eq!(s.last_killer, Some(FOE));
        assert_eq!(s.weapons[&7214].kills, 5);
        assert_eq!(s.weapons[&7214].deaths_by, 1);
        let r = s.rates(1000 + 600);
        assert!((r.kd - 5.0).abs() < 1e-9);
        assert!((r.kpm - 0.5).abs() < 1e-9);
        assert!((r.hsr - 0.6).abs() < 1e-9);
    }

    #[test]
    fn data_version_2_keeps_support_events_xp_and_teams() {
        let mut s = session();
        assert_eq!(s.version(), SESSION_DATA_VERSION);
        s.apply(&death(1010, FOE, ME, true, 1, 3));
        s.apply(&death(1070, ME, FOE, false, 3, 2));
        let xp = |ts: i64, who: u64, other: u64, id: u32, amount: u32| {
            GameEvent::GainExperience(crate::events::GainExperience {
                timestamp: ts,
                world_id: 10,
                zone_id: 2,
                character_id: who,
                other_id: other,
                experience_id: id,
                amount,
                loadout_id: 0,
            })
        };
        s.apply(&xp(1010, ME, FOE, 1, 100));
        s.apply(&xp(1075, ME, MATE, xp::REVIVE, 75));
        s.apply(&xp(1080, MATE, ME, xp::REVIVE, 75));
        s.apply(&xp(1130, ME, FOE, xp::KILL_ASSIST, 50));
        // the opponent's team is kept for kills and deaths
        let teams: Vec<_> = s.timeline.iter().map(|e| e.team).collect();
        assert_eq!(teams, vec![Some(1), Some(2)]);
        // support events in their own list, not in the kill feed
        assert_eq!(s.events.len(), 3);
        assert!(matches!(s.events[0].outcome, Outcome::Revive { .. }));
        assert!(matches!(s.events[1].outcome, Outcome::Revived { .. }));
        assert!(matches!(s.events[2].outcome, Outcome::Assist { .. }));
        assert_eq!(s.timeline.len(), 2);
        // XP per minute since the start (1000): minute 0 = 100, 1 = 75, 2 = 50
        assert_eq!(s.xp_by_minute.get(&0), Some(&100));
        assert_eq!(s.xp_by_minute.get(&1), Some(&75));
        assert_eq!(s.xp_by_minute.get(&2), Some(&50));
        // survives the JSON roundtrip of the database
        let back: Session = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.events, s.events);
        assert_eq!(back.xp_by_minute, s.xp_by_minute);
    }

    #[test]
    fn streak_milestone_emitted() {
        let mut s = session();
        let mut milestones = vec![];
        for i in 0..10 {
            for o in s.apply(&death(1000 + i, FOE, ME, false, 1, 3)) {
                if let Outcome::Streak { count } = o {
                    milestones.push(count);
                }
            }
        }
        assert_eq!(milestones, vec![5, 10]);
    }

    #[test]
    fn teamkills_and_suicides() {
        let mut s = session();
        let o = s.apply(&death(1, MATE, ME, false, 3, 3));
        assert!(matches!(o[0], Outcome::Teamkill { .. }));
        let o = s.apply(&death(2, ME, MATE, false, 3, 3));
        assert!(matches!(o[0], Outcome::Teamkilled { .. }));
        let o = s.apply(&death(3, ME, ME, false, 3, 3));
        assert!(matches!(o[0], Outcome::Suicide { .. }));
        let o = s.apply(&death(4, ME, 0, false, 3, 0));
        assert!(matches!(o[0], Outcome::Suicide { .. }));
        assert_eq!(s.stats.kills, 0);
        assert_eq!(s.stats.teamkills, 1);
        assert_eq!(s.stats.teamkilled, 1);
        assert_eq!(s.stats.suicides, 2);
        assert_eq!(s.stats.deaths, 3);
        assert_eq!(s.last_killer, None);
    }

    #[test]
    fn nso_without_team_ids_counts_as_kill() {
        let mut s = session();
        let mut d = death(1, FOE, ME, false, 1, 3);
        if let GameEvent::Death(ref mut x) = d {
            x.team_id = None;
            x.attacker_team_id = None;
        }
        assert!(matches!(s.apply(&d)[0], Outcome::Kill { .. }));
    }

    #[test]
    fn experience_revives_assists_xp() {
        let mut s = session();
        let ge = |c: u64, o: u64, id: u32, amount: u32| {
            GameEvent::GainExperience(GainExperience {
                timestamp: 5,
                world_id: 10,
                zone_id: 2,
                character_id: c,
                other_id: o,
                experience_id: id,
                amount,
                loadout_id: 4,
            })
        };
        s.apply(&ge(ME, FOE, xp::KILL_PLAYER, 100));
        s.apply(&ge(ME, MATE, xp::REVIVE, 75));
        s.apply(&ge(ME, FOE, xp::HIGH_PRIORITY_ASSIST, 50));
        s.apply(&ge(MATE, ME, xp::SQUAD_REVIVE, 100));
        s.apply(&ge(MATE, FOE, xp::REVIVE, 100));
        assert_eq!(s.stats.xp, 225);
        assert_eq!(s.stats.revives, 1);
        assert_eq!(s.stats.assists, 1);
        assert_eq!(s.stats.revived, 1);
    }

    #[test]
    fn vehicle_kills() {
        let mut s = session();
        let vd = |victim: u64, attacker: u64, vt: u32| {
            GameEvent::VehicleDestroy(VehicleDestroy {
                timestamp: 9,
                world_id: 10,
                zone_id: 2,
                character_id: victim,
                attacker_character_id: attacker,
                attacker_weapon_id: 1,
                attacker_vehicle_id: 0,
                vehicle_id: 4,
                faction_id: Some(vt),
                team_id: Some(vt),
                attacker_team_id: Some(3),
            })
        };
        s.apply(&vd(FOE, ME, 1));
        s.apply(&vd(MATE, ME, 3)); // own team: does not count
        s.apply(&vd(ME, FOE, 3));
        assert_eq!(s.stats.vehicle_kills, 1);
        assert_eq!(s.vehicle_kills_by_type[&4], 1);
        assert_eq!(s.stats.vehicles_lost, 1);
        assert_eq!(s.timeline.len(), 2);
    }

    #[test]
    fn killfeed_is_capped_and_restorable() {
        let mut s = Session::new(
            ME,
            "x",
            None,
            0,
            SessionConfig {
                streak_step: 0,
                killfeed_len: 3,
            },
        );
        for i in 0..6 {
            s.apply(&death(i, FOE, ME, false, 1, 3));
        }
        assert_eq!(s.killfeed().count(), 3);
        assert_eq!(s.timeline.len(), 6);
        let json = serde_json::to_string(&s).unwrap();
        let mut back: Session = serde_json::from_str(&json).unwrap();
        back.restore(SessionConfig {
            streak_step: 0,
            killfeed_len: 3,
        });
        assert_eq!(back.killfeed().next().unwrap().timestamp, 3);
    }

    #[test]
    fn ignores_foreign_events() {
        let mut s = session();
        assert!(s.apply(&death(1, FOE, MATE, false, 1, 3)).is_empty());
        assert_eq!(s.stats, SessionStats::default());
    }
}
