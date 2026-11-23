//! Trigger conditions: map base facts (derived from ESS data) to sound events –
//! our own IDs and Recursion keys. Data instead of code so conditions can be
//! defined by the user.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::voice::Trigger;
use crate::voicepack::{PackError, default_priority, is_known_event};

pub const DEFAULT_TRIGGERS: &str = include_str!("../assets/triggers.default.toml");

/// Valid base facts (`on`).
pub const FACTS: &[&str] = &[
    "kill",
    "first_kill",
    "multikill",
    "headshot_streak",
    "repeat_victim",
    "revenge",
    "mutual",
    "streak",
    "streak_ended",
    "death",
    "nemesis",
    "death_streak",
    "suicide",
    "teamkill",
    "teamkilled",
    "revive",
    "revived",
    "vehicle_kill",
    "achievement",
    "login",
    "session_start",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerRule {
    pub event: String,
    pub on: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headshot: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roadkill: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapon_category: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vehicle: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    /// Only for this kill chain style ("classic" or "unreal"); empty = always.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chain_style: String,
}

impl TriggerRule {
    fn matches(&self, f: &Trigger) -> bool {
        if self.on != f.event {
            return false;
        }
        if self.count.is_some_and(|c| f.count != c) || self.min_count.is_some_and(|c| f.count < c) {
            return false;
        }
        if self.headshot.is_some_and(|h| h != f.headshot)
            || self.roadkill.is_some_and(|r| r != f.roadkill)
        {
            return false;
        }
        let any = |list: &[String], v: &Option<String>| {
            v.as_deref()
                .is_some_and(|v| list.iter().any(|x| x.eq_ignore_ascii_case(v)))
        };
        if !self.weapon_category.is_empty() && !any(&self.weapon_category, &f.weapon_category) {
            return false;
        }
        if !self.vehicle.is_empty() && !any(&self.vehicle, &f.vehicle) {
            return false;
        }
        true
    }

    pub fn priority(&self) -> u8 {
        self.priority
            .unwrap_or_else(|| default_priority(&self.event))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerSet {
    #[serde(rename = "trigger", default)]
    pub rules: Vec<TriggerRule>,
}

impl TriggerSet {
    pub fn parse(text: &str) -> Result<Self, PackError> {
        let set: TriggerSet = toml::from_str(text)?;
        Self::validate_rules(&set.rules)?;
        Ok(set)
    }

    /// Most rules per event and longest list filter accepted from the UI.
    pub const MAX_RULES_PER_EVENT: usize = 8;

    /// Checks rules (from a file or edited in the UI).
    pub fn validate_rules(rules: &[TriggerRule]) -> Result<(), PackError> {
        for r in rules {
            if r.weapon_category.len() > 32
                || r.vehicle.len() > 32
                || r.description.len() > 200
                || r.weapon_category
                    .iter()
                    .chain(&r.vehicle)
                    .any(|x| x.len() > 64)
            {
                return Err(PackError::Invalid(format!("{}: list too long", r.event)));
            }
            if !is_known_event(&r.event) {
                return Err(PackError::Invalid(format!("unknown event: {}", r.event)));
            }
            if !FACTS.contains(&r.on.as_str()) {
                return Err(PackError::Invalid(format!(
                    "{}: unknown on = {:?}",
                    r.event, r.on
                )));
            }
            if !matches!(r.chain_style.as_str(), "" | "classic" | "unreal") {
                return Err(PackError::Invalid(format!(
                    "{}: chain_style must be \"classic\" or \"unreal\"",
                    r.event
                )));
            }
            if r.priority.is_some_and(|p| p > 100)
                || r.session_count == Some(0)
                || r.count.is_some_and(|c| c > 1000)
                || r.min_count.is_some_and(|c| c > 1000)
            {
                return Err(PackError::Invalid(format!(
                    "{}: value out of range",
                    r.event
                )));
            }
        }
        Ok(())
    }

    /// Rules edited in the UI replace all rules of their event; an empty list
    /// means "no trigger" (the event never fires). Each list must only contain
    /// rules for its own event.
    pub fn validate_overrides(o: &BTreeMap<String, Vec<TriggerRule>>) -> Result<(), PackError> {
        for (event, rules) in o {
            if rules.len() > Self::MAX_RULES_PER_EVENT {
                return Err(PackError::Invalid(format!("{event}: too many rules")));
            }
            if rules.iter().any(|r| &r.event != event) {
                return Err(PackError::Invalid(format!(
                    "{event}: rule for another event"
                )));
            }
            if !is_known_event(event) {
                return Err(PackError::Invalid(format!("unknown event: {event}")));
            }
            Self::validate_rules(rules)?;
        }
        Ok(())
    }

    /// This set with the rules of the overridden events replaced.
    pub fn with_overrides(
        &self,
        o: &BTreeMap<String, Vec<TriggerRule>>,
    ) -> Result<Self, PackError> {
        Self::validate_overrides(o)?;
        let mut rules: Vec<TriggerRule> = self
            .rules
            .iter()
            .filter(|r| !o.contains_key(&r.event))
            .cloned()
            .collect();
        rules.extend(o.values().flatten().cloned());
        Ok(Self { rules })
    }

    /// All rules of an event.
    pub fn rules_for(&self, event: &str) -> Vec<TriggerRule> {
        self.rules
            .iter()
            .filter(|r| r.event == event)
            .cloned()
            .collect()
    }

    pub fn default_set() -> Self {
        Self::parse(DEFAULT_TRIGGERS).expect("default triggers are valid")
    }

    /// Events with at least one rule.
    pub fn defined(&self) -> HashSet<&str> {
        self.rules.iter().map(|r| r.event.as_str()).collect()
    }

    /// Description(s) of the condition per event, for the UI.
    pub fn describe(&self, event: &str) -> Option<String> {
        let d: Vec<&str> = self
            .rules
            .iter()
            .filter(|r| r.event == event)
            .map(|r| r.description.as_str())
            .collect();
        (!d.is_empty()).then(|| d.join("; "))
    }

    pub fn priority(&self, event: &str) -> u8 {
        self.rules
            .iter()
            .find(|r| r.event == event)
            .map(TriggerRule::priority)
            .unwrap_or_else(|| default_priority(event))
    }
}

/// Which announcements a kill chain uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainStyle {
    /// 2 → EXCELLENT, 3 → OUTSTANDING, 4 → SUPERB, then MULTI_KILL …
    Classic,
    /// 2 → DOUBLE_KILL, 3 → TRIPLE_KILL, 4 → MULTI_KILL …
    Unreal,
}

impl ChainStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Unreal => "unreal",
        }
    }

    /// Setting "classic" | "unreal" | "auto": auto = classic when one of the
    /// active packs has a sound for EXCELLENT.
    pub fn resolve(setting: &str, packs_have_excellent: bool) -> Self {
        match setting {
            "classic" => Self::Classic,
            "unreal" => Self::Unreal,
            _ if packs_have_excellent => Self::Classic,
            _ => Self::Unreal,
        }
    }
}

/// Evaluates the rules per session (counts `session_count`).
#[derive(Debug, Clone, Default)]
pub struct TriggerEngine {
    counts: HashMap<usize, u32>,
}

impl TriggerEngine {
    /// Returns the triggered events (with priority) for a fact, kill chain in
    /// the "unreal" style.
    pub fn evaluate(&mut self, set: &TriggerSet, fact: &Trigger) -> Vec<(Trigger, u8)> {
        self.evaluate_style(set, fact, ChainStyle::Unreal)
    }

    /// Same with an explicit kill chain style.
    pub fn evaluate_style(
        &mut self,
        set: &TriggerSet,
        fact: &Trigger,
        style: ChainStyle,
    ) -> Vec<(Trigger, u8)> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for (i, r) in set.rules.iter().enumerate() {
            if !r.matches(fact) || !(r.chain_style.is_empty() || r.chain_style == style.name()) {
                continue;
            }
            if let Some(n) = r.session_count {
                let c = self.counts.entry(i).or_default();
                *c += 1;
                if *c != n {
                    continue;
                }
            }
            if seen.insert(r.event.clone()) {
                out.push((
                    Trigger {
                        event: r.event.clone(),
                        ..fact.clone()
                    },
                    r.priority(),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recursion_mapping::KEYS;

    fn fact(on: &str, count: u32) -> Trigger {
        let mut t = Trigger::new(on);
        t.count = count;
        t
    }

    fn events(set: &TriggerSet, e: &mut TriggerEngine, f: &Trigger) -> Vec<String> {
        e.evaluate(set, f)
            .into_iter()
            .map(|(t, _)| t.event)
            .collect()
    }

    #[test]
    fn default_set_maps_multikill_and_streaks() {
        let s = TriggerSet::default_set();
        let mut e = TriggerEngine::default();
        assert_eq!(
            events(&s, &mut e, &fact("multikill", 3)),
            vec!["multikill", "TRIPLE_KILL"]
        );
        assert_eq!(
            events(&s, &mut e, &fact("multikill", 12)),
            vec!["multikill", "HOLY_SHIT_2"]
        );
        assert_eq!(
            events(&s, &mut e, &fact("streak", 10)),
            vec!["streak", "DOMINATING"]
        );
        let mut hs = fact("kill", 1);
        hs.headshot = true;
        assert_eq!(
            events(&s, &mut e, &hs),
            vec!["headshot", "HEAD_SHOT", "PER_KILL"]
        );
        let mut knife = fact("kill", 1);
        knife.weapon_category = Some("knife".into());
        assert!(events(&s, &mut e, &knife).contains(&"KNIFE_FIGHT".to_string()));
        let mut rk = fact("kill", 1);
        rk.roadkill = true;
        assert!(events(&s, &mut e, &rk).contains(&"ROAD_KILL".to_string()));
    }

    #[test]
    fn kill_chain_styles() {
        let s = TriggerSet::default_set();
        let mut e = TriggerEngine::default();
        let mut chain = |n: u32, st: ChainStyle| -> Vec<String> {
            e.evaluate_style(&s, &fact("multikill", n), st)
                .into_iter()
                .map(|(t, _)| t.event)
                .collect()
        };
        use ChainStyle::*;
        assert_eq!(chain(2, Classic), vec!["multikill", "EXCELLENT"]);
        assert_eq!(chain(3, Classic), vec!["multikill", "OUTSTANDING"]);
        assert_eq!(chain(4, Classic), vec!["multikill", "SUPERB"]);
        assert_eq!(chain(5, Classic), vec!["multikill", "MULTI_KILL"]);
        assert_eq!(chain(12, Classic), vec!["multikill", "HOLY_SHIT_2"]);
        assert_eq!(chain(2, Unreal), vec!["multikill", "DOUBLE_KILL"]);
        assert_eq!(chain(4, Unreal), vec!["multikill", "MULTI_KILL"]);
        assert_eq!(ChainStyle::resolve("auto", true), Classic);
        assert_eq!(ChainStyle::resolve("auto", false), Unreal);
        assert_eq!(ChainStyle::resolve("unreal", true), Unreal);
        assert!(
            TriggerSet::parse("[[trigger]]\nevent='X_Y'\non='kill'\nchain_style='odd'\n").is_err()
        );
    }

    #[test]
    fn derived_keys() {
        let s = TriggerSet::default_set();
        let mut e = TriggerEngine::default();
        assert!(
            events(&s, &mut e, &fact("headshot_streak", 5)).contains(&"HEAD_HUNTER".to_string())
        );
        let mut rr = fact("multikill", 3);
        rr.roadkill = true;
        assert!(events(&s, &mut e, &rr).contains(&"ROAD_RAGE".to_string()));
        let mut air = fact("kill", 1);
        air.vehicle = Some("Reaver".into());
        assert!(events(&s, &mut e, &air).contains(&"DEATH_FROM_ABOVE".to_string()));
        let ground = fact("kill", 1);
        assert!(!events(&s, &mut e, &ground).contains(&"DEATH_FROM_ABOVE".to_string()));
        let d = s.defined();
        for k in ["SUICIDE_BOMBER", "VIP_KILL", "VIP_TEAM_KILL", "QUAD_DAMAGE"] {
            assert!(!d.contains(k), "{k}");
        }
    }

    #[test]
    fn session_count_fires_once_at_n() {
        let s = TriggerSet::default_set();
        let mut e = TriggerEngine::default();
        let mut air = fact("vehicle_kill", 0);
        air.vehicle = Some("Reaver".into());
        let mut fired = 0;
        for _ in 0..7 {
            if events(&s, &mut e, &air).contains(&"ACE".to_string()) {
                fired += 1;
            }
        }
        assert_eq!(fired, 1);
    }

    #[test]
    fn every_key_is_listed_and_unclear_ones_stay_open() {
        let s = TriggerSet::default_set();
        let defined = s.defined();
        for k in ["PIZZA_DELIVERY", "CARE_BEAR", "HIGH_ROLLER", "SUPERMAN"] {
            assert!(!defined.contains(k), "{k} must not have a trigger");
        }
        assert!(defined.iter().filter(|d| KEYS.contains(d)).count() >= 25);
    }

    #[test]
    fn rejects_bad_rules() {
        assert!(TriggerSet::parse("[[trigger]]\nevent='kill'\non='nope'\n").is_err());
        assert!(TriggerSet::parse("[[trigger]]\nevent='lowercase-x'\non='kill'\n").is_err());
    }

    #[test]
    fn ui_overrides_replace_the_rules_of_their_event() {
        let base = TriggerSet::default_set();
        let mut rule = base.rules_for("HEAD_SHOT")[0].clone();
        rule.min_count = Some(1);
        rule.weapon_category = vec!["Sniper Rifle".into()];
        let mut o = BTreeMap::new();
        o.insert("HEAD_SHOT".to_string(), vec![rule.clone()]);
        o.insert("ACE".to_string(), vec![]);
        let s = base.with_overrides(&o).unwrap();
        assert_eq!(s.rules_for("HEAD_SHOT"), vec![rule]);
        assert!(!s.defined().contains("ACE"), "empty list = no trigger");
        assert_eq!(s.rules_for("kill"), base.rules_for("kill"));

        // a rule for another event, an unknown fact or too many rules are refused
        let mut bad = BTreeMap::new();
        bad.insert("ACE".to_string(), base.rules_for("kill"));
        assert!(base.with_overrides(&bad).is_err());
        let mut r = base.rules_for("kill")[0].clone();
        r.on = "nope".into();
        bad.insert("ACE".to_string(), vec![]);
        bad.insert("kill".to_string(), vec![r]);
        assert!(base.with_overrides(&bad).is_err());
        let k = base.rules_for("kill")[0].clone();
        let mut many = BTreeMap::new();
        many.insert(
            "kill".to_string(),
            vec![k; TriggerSet::MAX_RULES_PER_EVENT + 1],
        );
        assert!(TriggerSet::validate_overrides(&many).is_err());
    }
}
