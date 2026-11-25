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
    /// Client ID of the user's own Discord application (Developer Portal),
    /// optional. Empty = the ELME Discord application built into official
    /// builds (the same one the online sync signs in with); builds without it
    /// need an own ID, otherwise the feature stays off. The built-in ID is
    /// never written here.
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

/// Discord application IDs are snowflakes: 17–20 digits.
pub fn is_app_id(id: &str) -> bool {
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

/// The default application ID: the ELME Discord application of official
/// builds, embedded at compile time for the online sync
/// (`PS2_SYNC_DISCORD_CLIENT_ID`). A public client ID, but the app never shows
/// it, logs it or writes it to the configuration.
pub fn built_in_app_id() -> Option<&'static str> {
    crate::online_sync::built_in_client_id()
}

/// This build has a default application (what the user interface may know).
pub fn has_default_app_id() -> bool {
    built_in_app_id().is_some()
}

impl DiscordSettings {
    /// The user's own ID is valid.
    pub fn app_id_valid(&self) -> bool {
        is_app_id(&self.app_id)
    }

    /// Own ID, otherwise the built-in one of official builds.
    pub fn effective_app_id(&self) -> Option<String> {
        self.app_id_with(built_in_app_id())
    }

    fn app_id_with(&self, default: Option<&str>) -> Option<String> {
        if self.app_id_valid() {
            Some(self.app_id.clone())
        } else if self.app_id.is_empty() {
            default.map(str::to_owned)
        } else {
            None
        }
    }

    /// A stored ID equal to the default is cleared, so the configuration
    /// never holds the default and follows it when it changes. Returns
    /// whether something changed.
    pub fn migrate(&mut self) -> bool {
        self.migrate_with(built_in_app_id())
    }

    fn migrate_with(&mut self, default: Option<&str>) -> bool {
        let trimmed = self.app_id.trim();
        if trimmed.len() != self.app_id.len() && (trimmed.is_empty() || is_app_id(trimmed)) {
            self.app_id = trimmed.to_owned();
        }
        if default.is_some_and(|d| !self.app_id.is_empty() && self.app_id == d) {
            self.app_id.clear();
            return true;
        }
        false
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.app_id.is_empty() && !self.app_id_valid() {
            return Err("Discord application ID must be 17–20 digits".into());
        }
        Ok(())
    }

    /// Active only when switched on and an ID is available (own or built in).
    pub fn active(&self) -> bool {
        self.active_with(built_in_app_id())
    }

    fn active_with(&self, default: Option<&str>) -> bool {
        self.enabled && self.app_id_with(default).is_some()
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
    use super::*;
    use crate::census::NameCache;
    use crate::session::{Session, SessionConfig};
    use crate::tracker::snapshot;

    /// A syntactically valid application ID, made at run time (no ID literal
    /// in the repository; secret scanners flag those).
    fn snowflake(seed: u64) -> String {
        (1_000_000_000_000_000_000u64 + seed * 7_919).to_string()
    }

    #[test]
    fn empty_uses_the_default_own_id_wins() {
        let default = snowflake(1);
        let own = snowflake(2);
        let mut d = DiscordSettings::default();
        assert_eq!(d.app_id_with(Some(&default)), Some(default.clone()));
        assert_eq!(d.app_id_with(None), None, "no default built in: off");
        d.app_id = own.clone();
        assert_eq!(d.app_id_with(Some(&default)), Some(own.clone()));
        assert_eq!(d.app_id_with(None), Some(own));
        // an invalid own ID never falls back to the default
        d.app_id = "123".into();
        assert_eq!(d.app_id_with(Some(&default)), None);
        d.app_id.clear();
        assert_eq!(d.effective_app_id().as_deref(), built_in_app_id());
        assert_eq!(has_default_app_id(), built_in_app_id().is_some());
    }

    #[test]
    fn invalid_ids_are_rejected() {
        let mut d = DiscordSettings::default();
        for bad in [
            "123",
            "abc",
            "12345678901234567x",
            "123456789012345678901",
            " 1",
            "-100000000000000000",
        ] {
            d.app_id = bad.into();
            assert!(d.validate().is_err(), "{bad:?} accepted");
            assert!(!d.app_id_valid());
        }
        d.app_id = snowflake(3);
        assert!(d.validate().is_ok());
        d.app_id.clear();
        assert!(d.validate().is_ok(), "empty = default");
    }

    #[test]
    fn active_with_and_without_a_default() {
        let default = snowflake(4);
        let mut d = DiscordSettings::default();
        assert!(!d.active_with(Some(&default)), "off by default");
        d.enabled = true;
        assert!(d.active_with(Some(&default)));
        assert!(!d.active_with(None), "local build without own ID: off");
        d.app_id = snowflake(5);
        assert!(d.active_with(None));
    }

    #[test]
    fn stored_default_is_migrated_to_empty_own_id_kept() {
        let default = snowflake(6);
        let own = snowflake(7);
        let mut d = DiscordSettings {
            app_id: default.clone(),
            ..Default::default()
        };
        assert!(d.migrate_with(Some(&default)));
        assert_eq!(d.app_id, "");
        d.app_id = own.clone();
        assert!(!d.migrate_with(Some(&default)));
        assert_eq!(d.app_id, own);
        d.app_id = format!(" {default} ");
        assert!(
            d.migrate_with(Some(&default)),
            "whitespace around the default"
        );
        assert_eq!(d.app_id, "");
        d.app_id.clear();
        assert!(!d.migrate_with(None));
        assert!(!d.migrate_with(Some(&default)));
    }

    #[test]
    fn default_never_in_the_serialized_settings() {
        let default = snowflake(8);
        let mut d = DiscordSettings {
            enabled: true,
            ..Default::default()
        };
        assert!(d.active_with(Some(&default)));
        let json = serde_json::to_string(&d).unwrap();
        let toml = toml::to_string(&d).unwrap();
        assert!(!json.contains(&default) && !toml.contains(&default));
        assert!(json.contains("\"app_id\":\"\""));
        // the same with the real default of this build, if it has one
        if let Some(b) = built_in_app_id() {
            d.app_id = b.to_owned();
            d.migrate();
            let cfg = crate::Config {
                discord: d,
                ..Default::default()
            };
            assert!(!serde_json::to_string(&cfg).unwrap().contains(b));
            assert!(!toml::to_string(&cfg).unwrap().contains(b));
        }
    }
    #[test]
    fn app_id_rules() {
        let mut s = DiscordSettings::default();
        assert!(!s.active(), "off by default");
        s.enabled = true;
        assert_eq!(
            s.active(),
            has_default_app_id(),
            "empty: only with a default"
        );
        s.app_id = "123".into();
        assert!(s.validate().is_err());
        s.app_id = snowflake(9);
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
