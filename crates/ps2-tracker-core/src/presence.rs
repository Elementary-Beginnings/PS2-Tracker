//! Discord Rich Presence: settings and the text shown (pure, testable). The
//! IPC connection itself lives in the app (src-tauri/src/discord.rs).

use serde::{Deserialize, Serialize};

use crate::tracker::SessionSnapshot;

/// Discord limits updates; the app never sends more often than this.
pub const MIN_UPDATE_SECS: u64 = 15;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordSettings {
    pub enabled: bool,
    /// Client ID of the user's own Discord application (Developer Portal);
    /// empty = the application of official builds (`PS2_DEFAULT_DISCORD_APP_ID`
    /// at compile time), otherwise the feature stays off.
    pub app_id: String,
    pub show_character: bool,
    pub show_kd: bool,
    pub show_kills: bool,
    pub show_duration: bool,
    /// Continent and server, e.g. "Indar · Miller" (off by default).
    pub show_location: bool,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            app_id: String::new(),
            show_character: true,
            show_kd: true,
            show_kills: true,
            show_duration: true,
            show_location: false,
        }
    }
}

/// Discord application of official builds (a public client ID, not a secret);
/// unset or empty in builds without it.
const BUILT_IN_APP_ID: Option<&str> = option_env!("PS2_DEFAULT_DISCORD_APP_ID");

/// Discord application IDs are snowflakes: 17–20 digits.
pub fn is_app_id(id: &str) -> bool {
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

/// The built-in application ID, if this build has a valid one.
pub fn built_in_app_id() -> Option<&'static str> {
    BUILT_IN_APP_ID.filter(|id| is_app_id(id))
}

impl DiscordSettings {
    /// The user's own ID is valid.
    pub fn app_id_valid(&self) -> bool {
        is_app_id(&self.app_id)
    }

    /// Own ID, otherwise the built-in one of official builds.
    pub fn effective_app_id(&self) -> Option<String> {
        if self.app_id_valid() {
            Some(self.app_id.clone())
        } else if self.app_id.is_empty() {
            built_in_app_id().map(str::to_owned)
        } else {
            None
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.app_id.is_empty() && !self.app_id_valid() {
            return Err("Discord application ID must be 17–20 digits".into());
        }
        Ok(())
    }

    /// Active only when switched on and an ID is available (own or built in).
    pub fn active(&self) -> bool {
        self.enabled && self.effective_app_id().is_some()
    }
}

/// What Rich Presence shows: (details, state, start timestamp).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    pub details: String,
    pub state: String,
    pub start: Option<i64>,
}

fn faction_short(id: Option<u32>) -> &'static str {
    match id {
        Some(1) => "VS",
        Some(2) => "NC",
        Some(3) => "TR",
        Some(4) => "NSO",
        _ => "",
    }
}

pub fn presence(snap: Option<&SessionSnapshot>, s: &DiscordSettings) -> Presence {
    let Some(snap) = snap else {
        return Presence {
            details: "PlanetSide 2".into(),
            state: "No session".into(),
            start: None,
        };
    };
    let details = if s.show_character {
        let f = faction_short(snap.faction_id);
        if f.is_empty() {
            snap.character_name.clone()
        } else {
            format!("{} ({f})", snap.character_name)
        }
    } else {
        "PlanetSide 2".into()
    };
    let mut parts = Vec::new();
    if s.show_location {
        let loc: Vec<&str> = [snap.zone_name.as_deref(), snap.world_name.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        if !loc.is_empty() {
            parts.push(loc.join(" · "));
        }
    }
    if s.show_kd {
        parts.push(format!("K/D {:.2}", snap.rates.kd));
    }
    if s.show_kills {
        parts.push(format!("{} kills", snap.stats.kills));
    }
    let state = if parts.is_empty() {
        "In a session".into()
    } else {
        parts.join(" · ")
    };
    Presence {
        details: details.chars().take(120).collect(),
        state: state.chars().take(120).collect(),
        start: s.show_duration.then_some(snap.started_at),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn effective_app_id() {
        let mut d = DiscordSettings {
            app_id: "112233445566778899".into(),
            ..Default::default()
        };
        assert_eq!(d.effective_app_id().as_deref(), Some("112233445566778899"));
        // an invalid own ID never falls back to the built-in one
        d.app_id = "123".into();
        assert_eq!(d.effective_app_id(), None);
        d.app_id.clear();
        assert_eq!(d.effective_app_id().as_deref(), built_in_app_id());
    }

    use super::*;
    use crate::census::NameCache;
    use crate::session::{Session, SessionConfig};
    use crate::tracker::snapshot;

    #[test]
    fn app_id_rules() {
        let mut s = DiscordSettings::default();
        assert!(!s.active(), "off by default");
        s.enabled = true;
        assert!(!s.active(), "no ID, no presence");
        s.app_id = "123".into();
        assert!(s.validate().is_err());
        s.app_id = "112233445566778899".into();
        assert!(s.validate().is_ok() && s.active());
    }

    #[test]
    fn text_follows_the_settings() {
        let mut sess = Session::new(1, "DemoSoldierTR", Some(3), 1000, SessionConfig::default());
        sess.stats.kills = 12;
        sess.stats.deaths = 4;
        let snap = snapshot(&sess, true, 1600, &NameCache::default(), None, &[]);
        let mut s = DiscordSettings::default();
        let p = presence(Some(&snap), &s);
        assert_eq!(p.details, "DemoSoldierTR (TR)");
        assert_eq!(p.state, "K/D 3.00 · 12 kills");
        assert_eq!(p.start, Some(1000));
        s.show_character = false;
        s.show_kd = false;
        s.show_kills = false;
        s.show_duration = false;
        let p = presence(Some(&snap), &s);
        assert_eq!(
            (p.details.as_str(), p.state.as_str(), p.start),
            ("PlanetSide 2", "In a session", None)
        );
        assert_eq!(presence(None, &s).state, "No session");
    }

    #[test]
    fn location_only_when_wanted_and_known() {
        let mut sess = Session::new(1, "DemoSoldierTR", Some(3), 1000, SessionConfig::default());
        sess.world_id = Some(10);
        sess.zone_id = Some(2);
        let mut names = NameCache::default();
        let mut s = DiscordSettings {
            show_kd: false,
            show_kills: false,
            ..Default::default()
        };
        let snap = snapshot(&sess, true, 1600, &names, None, &[]);
        assert_eq!(presence(Some(&snap), &s).state, "In a session");
        s.show_location = true;
        // names not resolved yet: nothing shown instead of raw IDs
        assert_eq!(presence(Some(&snap), &s).state, "In a session");
        names.worlds.insert(10, "Miller".into());
        names.zones.insert(2, "Indar".into());
        let snap = snapshot(&sess, true, 1600, &names, None, &[]);
        assert_eq!(presence(Some(&snap), &s).state, "Indar · Miller");
    }
}
