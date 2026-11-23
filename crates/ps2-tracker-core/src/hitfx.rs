//! Crosshair feedback ("hit markers" as in Recursion), derived from ESS events.
//!
//! The Census API / ESS has **no hit or damage events** for infantry – only
//! deaths, vehicle destructions and experience. Single hits would only be
//! visible inside the game, which is ruled out (BattlEye, no access to the game
//! process). So the feedback reacts to what ESS reports: own kills, headshot
//! kills, kill assists, destroyed vehicles and damage XP against vehicles, with
//! the ESS delay (typically a few hundred milliseconds).

use serde::{Deserialize, Serialize};

use crate::events::{CharacterId, GameEvent};

/// Kill assist experience IDs (Census `experience`): 2 "Kill Player Assist",
/// 371 "Kill Player Priority Assist", 372 "Kill Player High Priority Assist".
pub use crate::events::xp::is_assist;

/// Experience IDs for damaging an enemy vehicle (Census `experience`,
/// description "… Damage" / "… Damage (Infantry vs Vehicle)"; HIVE entries and
/// component damage left out). These are the only ESS events close to "hits".
pub const VEHICLE_DAMAGE_XP: &[u32] = &[
    280,  // Lighting Damage (Infantry vs Vehicle)
    281,  // Prowler Damage (Infantry vs Vehicle)
    283,  // Galaxy Damage
    284,  // Liberator Damage
    285,  // Magrider Damage (Infantry vs Vehicle)
    286,  // Mosquito Damage
    287,  // Reaver Damage
    288,  // Scythe Damage
    289,  // Sunderer Damage (Infantry vs Vehicle)
    290,  // Vanguard Damage (Infantry vs Vehicle)
    508,  // Valkyrie Damage
    665,  // ANT Damage (Infantry vs Vehicle)
    1395, // Flash Damage (Infantry vs Vehicle)
    1396, // Harasser Damage (Infantry vs Vehicle)
    1458, // Colossus Damage (Infantry vs Vehicle)
    1502, // Javelin Damage (Infantry vs Vehicle)
    1557, // Chimera Damage (Infantry vs Vehicle)
    1596, // Dervish Damage
    1647, // Dervish Damage
    1752, // Prototype Sunderer Damage (Infantry vs Vehicle)
    1818, // Prototype Galaxy Damage
    1876, // Prototype Valkyrie Damage
    1923, // Lodestar Damage
    1924, // Wasp Damage
    2002, // Corsair Damage (Infantry vs Vehicle)
];

/// What triggered the feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxKind {
    Kill,
    Headshot,
    Assist,
    VehicleKill,
    VehicleDamage,
}

impl FxKind {
    pub const ALL: [FxKind; 5] = [
        FxKind::Kill,
        FxKind::Headshot,
        FxKind::Assist,
        FxKind::VehicleKill,
        FxKind::VehicleDamage,
    ];
    pub fn name(self) -> &'static str {
        match self {
            FxKind::Kill => "kill",
            FxKind::Headshot => "headshot",
            FxKind::Assist => "assist",
            FxKind::VehicleKill => "vehicle_kill",
            FxKind::VehicleDamage => "vehicle_damage",
        }
    }
}

/// One effect: flash the crosshair in a colour or swap it for another PNG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxEffect {
    pub enabled: bool,
    /// "tint" | "swap"
    pub mode: String,
    /// "#rrggbb" for "tint".
    pub color: String,
    /// File in the crosshair library for "swap".
    pub file: String,
    /// Length of the effect in milliseconds (40–2000).
    pub duration_ms: u32,
    /// Fade out instead of switching back at once.
    pub fade: bool,
    /// Opacity at the peak of the effect (0.1–1).
    pub peak: f32,
}

impl FxEffect {
    fn tint(enabled: bool, color: &str) -> Self {
        Self {
            enabled,
            mode: "tint".into(),
            color: color.into(),
            file: String::new(),
            duration_ms: 150,
            fade: true,
            peak: 1.0,
        }
    }
}

impl Default for FxEffect {
    fn default() -> Self {
        Self::tint(false, "#ffffff")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HitFeedback {
    pub enabled: bool,
    pub kill: FxEffect,
    pub headshot: FxEffect,
    pub assist: FxEffect,
    pub vehicle_kill: FxEffect,
    pub vehicle_damage: FxEffect,
}

impl Default for HitFeedback {
    fn default() -> Self {
        Self {
            enabled: false,
            kill: FxEffect::tint(true, "#ffffff"),
            headshot: FxEffect::tint(true, "#ff3b30"),
            assist: FxEffect::tint(false, "#62b4ff"),
            vehicle_kill: FxEffect::tint(false, "#ffd24a"),
            vehicle_damage: FxEffect {
                duration_ms: 80,
                peak: 0.7,
                ..FxEffect::tint(false, "#ffd24a")
            },
        }
    }
}

impl HitFeedback {
    pub fn effect(&self, kind: FxKind) -> &FxEffect {
        match kind {
            FxKind::Kill => &self.kill,
            FxKind::Headshot => &self.headshot,
            FxKind::Assist => &self.assist,
            FxKind::VehicleKill => &self.vehicle_kill,
            FxKind::VehicleDamage => &self.vehicle_damage,
        }
    }

    /// The effect to show for an event of one of the tracked characters, if any.
    pub fn for_event(&self, ev: &GameEvent, me: CharacterId) -> Option<(FxKind, &FxEffect)> {
        if !self.enabled {
            return None;
        }
        let kind = classify(ev, me)?;
        let fx = self.effect(kind);
        fx.enabled.then_some((kind, fx))
    }

    pub fn validate(&self) -> Result<(), String> {
        for k in FxKind::ALL {
            let e = self.effect(k);
            let hex = e.color.len() == 7
                && e.color.starts_with('#')
                && e.color[1..].chars().all(|c| c.is_ascii_hexdigit());
            if !matches!(e.mode.as_str(), "tint" | "swap") {
                return Err(format!("{}: mode must be tint or swap", k.name()));
            }
            if e.mode == "tint" && !hex {
                return Err(format!("{}: colour must be #rrggbb", k.name()));
            }
            if e.file.contains(['/', '\\']) || e.file.contains("..") {
                return Err(format!("{}: invalid crosshair file", k.name()));
            }
            if !(40..=2000).contains(&e.duration_ms) || !(0.1..=1.0).contains(&e.peak) {
                return Err(format!("{}: duration or peak out of range", k.name()));
            }
        }
        Ok(())
    }
}

/// Which feedback an ESS event means for character `me`. Own deaths, suicides
/// and teamkills give none.
pub fn classify(ev: &GameEvent, me: CharacterId) -> Option<FxKind> {
    match ev {
        GameEvent::Death(d) => {
            let own_kill = d.attacker_character_id == me && d.character_id != me;
            let teamkill = matches!((d.team_id, d.attacker_team_id), (Some(a), Some(b)) if a == b);
            (own_kill && !teamkill).then_some(if d.is_headshot {
                FxKind::Headshot
            } else {
                FxKind::Kill
            })
        }
        GameEvent::VehicleDestroy(v) => {
            let own = v.attacker_character_id == me && v.character_id != me;
            let friendly = matches!((v.team_id, v.attacker_team_id), (Some(a), Some(b)) if a == b);
            (own && !friendly).then_some(FxKind::VehicleKill)
        }
        GameEvent::GainExperience(g) if g.character_id == me => {
            if is_assist(g.experience_id) {
                Some(FxKind::Assist)
            } else if VEHICLE_DAMAGE_XP.contains(&g.experience_id) {
                Some(FxKind::VehicleDamage)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Death, GainExperience, VehicleDestroy};

    const ME: CharacterId = 5428000000000000042;
    const FOE: CharacterId = 5428000000000000099;

    fn death(victim: CharacterId, attacker: CharacterId, hs: bool, teams: (u32, u32)) -> GameEvent {
        GameEvent::Death(Death {
            timestamp: 1,
            world_id: 10,
            zone_id: 2,
            character_id: victim,
            attacker_character_id: attacker,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 0,
            character_loadout_id: 0,
            is_headshot: hs,
            team_id: Some(teams.0),
            attacker_team_id: Some(teams.1),
        })
    }
    fn xp(who: CharacterId, id: u32) -> GameEvent {
        GameEvent::GainExperience(GainExperience {
            timestamp: 1,
            world_id: 10,
            zone_id: 2,
            character_id: who,
            other_id: FOE,
            experience_id: id,
            amount: 100,
            loadout_id: 0,
        })
    }

    #[test]
    fn maps_events_to_effects() {
        assert_eq!(
            classify(&death(FOE, ME, false, (1, 2)), ME),
            Some(FxKind::Kill)
        );
        assert_eq!(
            classify(&death(FOE, ME, true, (1, 2)), ME),
            Some(FxKind::Headshot)
        );
        // own death, suicide and teamkill: no effect
        assert_eq!(classify(&death(ME, FOE, true, (2, 1)), ME), None);
        assert_eq!(classify(&death(ME, ME, false, (2, 2)), ME), None);
        assert_eq!(classify(&death(FOE, ME, false, (2, 2)), ME), None);
        assert_eq!(classify(&xp(ME, 2), ME), Some(FxKind::Assist));
        assert_eq!(classify(&xp(ME, 372), ME), Some(FxKind::Assist));
        assert_eq!(classify(&xp(ME, 287), ME), Some(FxKind::VehicleDamage));
        assert_eq!(classify(&xp(ME, 7), ME), None, "revive is no hit");
        assert_eq!(classify(&xp(FOE, 2), ME), None, "someone else's assist");
        let v = GameEvent::VehicleDestroy(VehicleDestroy {
            timestamp: 1,
            world_id: 10,
            zone_id: 2,
            character_id: FOE,
            attacker_character_id: ME,
            attacker_weapon_id: 0,
            attacker_vehicle_id: 0,
            vehicle_id: 4,
            faction_id: Some(1),
            team_id: Some(1),
            attacker_team_id: Some(2),
        });
        assert_eq!(classify(&v, ME), Some(FxKind::VehicleKill));
    }

    #[test]
    fn settings_gate_the_effects() {
        let mut h = HitFeedback::default();
        let kill = death(FOE, ME, false, (1, 2));
        assert!(h.for_event(&kill, ME).is_none(), "off by default");
        h.enabled = true;
        let (k, fx) = h.for_event(&kill, ME).unwrap();
        assert_eq!(
            (k, fx.duration_ms, fx.color.as_str()),
            (FxKind::Kill, 150, "#ffffff")
        );
        let (_, hs) = h.for_event(&death(FOE, ME, true, (1, 2)), ME).unwrap();
        assert_eq!(hs.color, "#ff3b30");
        assert!(
            h.for_event(&xp(ME, 2), ME).is_none(),
            "assist off by default"
        );
        assert!(h.validate().is_ok());
        h.headshot.mode = "swap".into();
        h.headshot.file = "../x.png".into();
        assert!(h.validate().is_err());
        h.headshot.file = "dot.png".into();
        h.headshot.duration_ms = 5;
        assert!(h.validate().is_err());
    }
}
