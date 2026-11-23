//! Simple custom achievements. The rule set is data (TOML), so new rules can be
//! added without rebuilding.

use std::collections::{BTreeMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::session::{Outcome, Session};

/// Bundled default rule set.
pub const DEFAULT_RULES: &str = include_str!("../assets/achievements.default.toml");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleKind {
    /// `count` kills within `window_secs` seconds.
    Multikill { count: u32, window_secs: i64 },
    /// Kill streak without dying reaches `count`.
    Streak { count: u32 },
    /// `count` headshot kills in a row.
    HeadshotStreak { count: u32 },
    /// Kill on the character who killed you last.
    Revenge,
    /// Vehicle kills in the session reach `count`.
    VehicleKills { count: u32 },
    /// Kills in the session reach `count`.
    SessionKills { count: u32 },
    /// Revives in the session reach `count`.
    Revives { count: u32 },
    /// Kills with a single weapon in the session reach `count`.
    WeaponKills { count: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(flatten)]
    pub kind: RuleKind,
    /// May fire several times per session (e.g. multikills).
    #[serde(default)]
    pub repeatable: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(rename = "rule", default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("invalid rule set: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("duplicate rule id: {0}")]
    Duplicate(String),
    #[error("rule {0}: value must be greater than 0")]
    Zero(String),
}

impl RuleSet {
    pub fn parse(text: &str) -> Result<Self, RuleError> {
        let set: RuleSet = toml::from_str(text)?;
        let mut seen = HashSet::new();
        for r in &set.rules {
            if !seen.insert(r.id.clone()) {
                return Err(RuleError::Duplicate(r.id.clone()));
            }
            let zero = match r.kind {
                RuleKind::Multikill { count, window_secs } => count == 0 || window_secs <= 0,
                RuleKind::Streak { count }
                | RuleKind::HeadshotStreak { count }
                | RuleKind::VehicleKills { count }
                | RuleKind::SessionKills { count }
                | RuleKind::Revives { count }
                | RuleKind::WeaponKills { count } => count == 0,
                RuleKind::Revenge => false,
            };
            if zero {
                return Err(RuleError::Zero(r.id.clone()));
            }
        }
        Ok(set)
    }

    pub fn default_rules() -> Self {
        Self::parse(DEFAULT_RULES).expect("default rule set is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unlocked {
    pub id: String,
    pub name: String,
    pub description: String,
    pub timestamp: i64,
}

/// Evaluates the rules for one session. One instance per session.
#[derive(Debug, Clone)]
pub struct AchievementEngine {
    rules: Vec<Rule>,
    recent_kills: VecDeque<i64>,
    fired: HashSet<String>,
    /// Multikills: kill counter at the last firing per rule, so a triple kill
    /// does not fire again on every further kill.
    multikill_mark: BTreeMap<String, usize>,
    pub unlocked: Vec<Unlocked>,
}

impl AchievementEngine {
    pub fn new(set: &RuleSet) -> Self {
        Self {
            rules: set.rules.iter().filter(|r| r.enabled).cloned().collect(),
            recent_kills: VecDeque::new(),
            fired: HashSet::new(),
            multikill_mark: BTreeMap::new(),
            unlocked: Vec::new(),
        }
    }

    /// Call after `Session::apply` with the session state *after* the event.
    /// `previous_killer` is `last_killer` *before* the event (for revenge).
    pub fn on_outcomes(
        &mut self,
        session: &Session,
        outcomes: &[Outcome],
        timestamp: i64,
        previous_killer: Option<u64>,
    ) -> Vec<Unlocked> {
        let mut new = Vec::new();
        let max_window = self
            .rules
            .iter()
            .filter_map(|r| match r.kind {
                RuleKind::Multikill { window_secs, .. } => Some(window_secs),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        for o in outcomes {
            match o {
                Outcome::Kill { victim, .. } => {
                    self.recent_kills.push_back(timestamp);
                    while let Some(&t) = self.recent_kills.front() {
                        if timestamp - t > max_window {
                            self.recent_kills.pop_front();
                        } else {
                            break;
                        }
                    }
                    let rules = self.rules.clone();
                    for r in &rules {
                        let hit = match &r.kind {
                            RuleKind::Multikill { count, window_secs } => {
                                let n = self
                                    .recent_kills
                                    .iter()
                                    .filter(|&&t| timestamp - t <= *window_secs)
                                    .count();
                                let mark = self.multikill_mark.get(&r.id).copied().unwrap_or(0);
                                let total = session.stats.kills as usize;
                                // fire again only after enough new kills since the last firing
                                if n >= *count as usize && total >= mark + *count as usize {
                                    self.multikill_mark.insert(r.id.clone(), total);
                                    true
                                } else {
                                    false
                                }
                            }
                            RuleKind::Streak { count } => session.streak == *count,
                            RuleKind::HeadshotStreak { count } => session.headshot_streak == *count,
                            RuleKind::Revenge => previous_killer == Some(*victim),
                            RuleKind::SessionKills { count } => session.stats.kills == *count,
                            RuleKind::WeaponKills { count } => {
                                if let Outcome::Kill { weapon_id, .. } = o {
                                    session.weapons.get(weapon_id).map(|w| w.kills) == Some(*count)
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        };
                        if hit {
                            self.fire(r, timestamp, &mut new);
                        }
                    }
                }
                Outcome::VehicleKill { .. } => {
                    let rules = self.rules.clone();
                    for r in &rules {
                        if let RuleKind::VehicleKills { count } = r.kind
                            && session.stats.vehicle_kills == count
                        {
                            self.fire(r, timestamp, &mut new);
                        }
                    }
                }
                Outcome::Revive { .. } => {
                    let rules = self.rules.clone();
                    for r in &rules {
                        if let RuleKind::Revives { count } = r.kind
                            && session.stats.revives == count
                        {
                            self.fire(r, timestamp, &mut new);
                        }
                    }
                }
                Outcome::Death { .. } | Outcome::Suicide { .. } | Outcome::Teamkilled { .. } => {
                    self.recent_kills.clear();
                }
                _ => {}
            }
        }
        new
    }

    fn fire(&mut self, r: &Rule, timestamp: i64, new: &mut Vec<Unlocked>) {
        if !r.repeatable && !self.fired.insert(r.id.clone()) {
            return;
        }
        let u = Unlocked {
            id: r.id.clone(),
            name: r.name.clone(),
            description: r.description.clone(),
            timestamp,
        };
        self.unlocked.push(u.clone());
        new.push(u);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Death, GameEvent};
    use crate::session::SessionConfig;

    const ME: u64 = 1;

    fn kill(ts: i64, victim: u64, hs: bool) -> GameEvent {
        GameEvent::Death(Death {
            timestamp: ts,
            world_id: 1,
            zone_id: 1,
            character_id: victim,
            attacker_character_id: ME,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 0,
            character_loadout_id: 0,
            is_headshot: hs,
            team_id: Some(1),
            attacker_team_id: Some(3),
        })
    }

    fn died(ts: i64, killer: u64) -> GameEvent {
        GameEvent::Death(Death {
            timestamp: ts,
            world_id: 1,
            zone_id: 1,
            character_id: ME,
            attacker_character_id: killer,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 0,
            character_loadout_id: 0,
            is_headshot: false,
            team_id: Some(3),
            attacker_team_id: Some(1),
        })
    }

    fn run(events: &[GameEvent]) -> Vec<String> {
        let mut s = Session::new(ME, "me", Some(3), 0, SessionConfig::default());
        let mut e = AchievementEngine::new(&RuleSet::default_rules());
        let mut ids = vec![];
        for ev in events {
            let prev = s.last_killer;
            let o = s.apply(ev);
            for u in e.on_outcomes(&s, &o, ev.timestamp(), prev) {
                ids.push(u.id);
            }
        }
        ids
    }

    #[test]
    fn default_rules_parse() {
        let set = RuleSet::default_rules();
        assert!(set.rules.len() >= 8);
    }

    #[test]
    fn double_and_triple_kill() {
        let ids = run(&[kill(10, 2, false), kill(12, 3, false), kill(13, 4, false)]);
        assert_eq!(ids, vec!["double_kill", "triple_kill"]);
    }

    #[test]
    fn multikill_window_expires() {
        let ids = run(&[kill(10, 2, false), kill(30, 3, false)]);
        assert!(ids.is_empty());
    }

    #[test]
    fn revenge() {
        let ids = run(&[died(5, 77), kill(20, 77, false)]);
        assert_eq!(ids, vec!["revenge"]);
    }

    #[test]
    fn headshot_streak_and_streak_once() {
        let mut evs = vec![];
        for i in 0..5 {
            evs.push(kill(100 * i, 10 + i as u64, true));
        }
        let ids = run(&evs);
        assert!(ids.contains(&"headshot_streak_3".to_string()));
        assert!(ids.contains(&"streak_5".to_string()));
        assert_eq!(ids.iter().filter(|i| *i == "streak_5").count(), 1);
    }

    #[test]
    fn rejects_bad_rules() {
        let dup = "[[rule]]\nid='a'\nname='A'\nkind='revenge'\n[[rule]]\nid='a'\nname='B'\nkind='revenge'\n";
        assert!(matches!(RuleSet::parse(dup), Err(RuleError::Duplicate(_))));
        let zero = "[[rule]]\nid='a'\nname='A'\nkind='streak'\ncount=0\n";
        assert!(matches!(RuleSet::parse(zero), Err(RuleError::Zero(_))));
    }
}
