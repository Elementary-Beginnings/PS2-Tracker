//! Event model for the Daybreak Event Streaming Service (ESS).
//!
//! ESS sends every value as a JSON string (numbers and booleans as `"0"`/`"1"` too).
//! They are translated into typed structs here. Unknown events are kept as
//! [`GameEvent::Other`] so new event types do not break anything.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Character ID (public game data, a 19-digit number).
pub type CharacterId = u64;

/// Serializes IDs as strings (JavaScript loses digits above 2^53),
/// accepts a string or a number when reading.
pub mod id_string {
    use serde::{Deserialize, Deserializer, Serializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        S(String),
        N(u64),
    }

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        match Raw::deserialize(d)? {
            Raw::N(n) => Ok(n),
            Raw::S(s) => s.parse().map_err(serde::de::Error::custom),
        }
    }

    pub mod option {
        use super::Raw;
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
            match v {
                Some(x) => s.serialize_str(&x.to_string()),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
            match Option::<Raw>::deserialize(d)? {
                None => Ok(None),
                Some(Raw::N(n)) => Ok(Some(n)),
                Some(Raw::S(s)) if s.is_empty() => Ok(None),
                Some(Raw::S(s)) => s.parse().map(Some).map_err(serde::de::Error::custom),
            }
        }
    }
}

/// Known experience IDs (Census collection `experience`).
pub mod xp {
    pub const KILL_PLAYER: u32 = 1;
    pub const KILL_ASSIST: u32 = 2;
    pub const REVIVE: u32 = 7;
    pub const SQUAD_REVIVE: u32 = 53;
    pub const PRIORITY_ASSIST: u32 = 371;
    pub const HIGH_PRIORITY_ASSIST: u32 = 372;

    pub fn is_revive(id: u32) -> bool {
        matches!(id, REVIVE | SQUAD_REVIVE)
    }
    pub fn is_assist(id: u32) -> bool {
        matches!(id, KILL_ASSIST | PRIORITY_ASSIST | HIGH_PRIORITY_ASSIST)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Death {
    pub timestamp: i64,
    pub world_id: u32,
    pub zone_id: u32,
    pub character_id: CharacterId,
    pub attacker_character_id: CharacterId,
    pub attacker_weapon_id: u32,
    pub attacker_vehicle_id: u32,
    pub attacker_loadout_id: u32,
    pub character_loadout_id: u32,
    pub is_headshot: bool,
    /// Team (faction or NSO assignment) of the victim; missing in old payloads.
    pub team_id: Option<u32>,
    pub attacker_team_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleDestroy {
    pub timestamp: i64,
    pub world_id: u32,
    pub zone_id: u32,
    pub character_id: CharacterId,
    pub attacker_character_id: CharacterId,
    pub attacker_weapon_id: u32,
    pub attacker_vehicle_id: u32,
    pub vehicle_id: u32,
    pub faction_id: Option<u32>,
    pub team_id: Option<u32>,
    pub attacker_team_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GainExperience {
    pub timestamp: i64,
    pub world_id: u32,
    pub zone_id: u32,
    pub character_id: CharacterId,
    pub other_id: u64,
    pub experience_id: u32,
    pub amount: u32,
    pub loadout_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSession {
    pub timestamp: i64,
    pub world_id: u32,
    pub character_id: CharacterId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FacilityControl {
    pub timestamp: i64,
    pub world_id: u32,
    pub zone_id: u32,
    pub facility_id: u32,
    pub new_faction_id: u32,
    pub old_faction_id: u32,
    pub outfit_id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerFacility {
    pub timestamp: i64,
    pub world_id: u32,
    pub zone_id: u32,
    pub character_id: CharacterId,
    pub facility_id: u32,
    pub outfit_id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemAdded {
    pub timestamp: i64,
    pub world_id: u32,
    pub character_id: CharacterId,
    pub item_id: u32,
    pub item_count: u32,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AchievementEarned {
    pub timestamp: i64,
    pub world_id: u32,
    pub character_id: CharacterId,
    pub achievement_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleRankUp {
    pub timestamp: i64,
    pub world_id: u32,
    pub character_id: CharacterId,
    pub battle_rank: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GameEvent {
    Death(Death),
    VehicleDestroy(VehicleDestroy),
    GainExperience(GainExperience),
    PlayerLogin(PlayerSession),
    PlayerLogout(PlayerSession),
    FacilityControl(FacilityControl),
    PlayerFacilityCapture(PlayerFacility),
    PlayerFacilityDefend(PlayerFacility),
    ItemAdded(ItemAdded),
    AchievementEarned(AchievementEarned),
    BattleRankUp(BattleRankUp),
    Other { event_name: String, timestamp: i64 },
}

impl GameEvent {
    pub fn timestamp(&self) -> i64 {
        match self {
            GameEvent::Death(e) => e.timestamp,
            GameEvent::VehicleDestroy(e) => e.timestamp,
            GameEvent::GainExperience(e) => e.timestamp,
            GameEvent::PlayerLogin(e) | GameEvent::PlayerLogout(e) => e.timestamp,
            GameEvent::FacilityControl(e) => e.timestamp,
            GameEvent::PlayerFacilityCapture(e) | GameEvent::PlayerFacilityDefend(e) => e.timestamp,
            GameEvent::ItemAdded(e) => e.timestamp,
            GameEvent::AchievementEarned(e) => e.timestamp,
            GameEvent::BattleRankUp(e) => e.timestamp,
            GameEvent::Other { timestamp, .. } => *timestamp,
        }
    }
}

/// Messages arriving over the ESS connection.
#[derive(Debug, Clone, PartialEq)]
pub enum EssMessage {
    Event(GameEvent),
    Heartbeat,
    ConnectionState {
        connected: bool,
    },
    Subscription(Value),
    /// Help texts, `serviceStateChanged` etc. – irrelevant for the logic.
    Ignored,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("field `{0}` missing or invalid")]
    Field(&'static str),
}

struct Fields<'a>(&'a serde_json::Map<String, Value>);

impl Fields<'_> {
    fn raw(&self, key: &'static str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str)
    }
    fn num<T: std::str::FromStr>(&self, key: &'static str) -> Result<T, ParseError> {
        self.raw(key)
            .and_then(|s| s.parse().ok())
            .ok_or(ParseError::Field(key))
    }
    /// Optional number; missing or empty fields yield `default`.
    fn num_or<T: std::str::FromStr>(&self, key: &'static str, default: T) -> T {
        self.raw(key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    }
    fn opt<T: std::str::FromStr>(&self, key: &'static str) -> Option<T> {
        self.raw(key).and_then(|s| s.parse().ok())
    }
    fn flag(&self, key: &'static str) -> bool {
        self.raw(key) == Some("1")
    }
}

/// Translates an ESS `payload` into a [`GameEvent`].
pub fn parse_payload(payload: &Value) -> Result<GameEvent, ParseError> {
    let obj = payload.as_object().ok_or(ParseError::Field("payload"))?;
    let f = Fields(obj);
    let name = f.raw("event_name").ok_or(ParseError::Field("event_name"))?;
    let timestamp: i64 = f.num("timestamp")?;
    let ev = match name {
        "Death" => GameEvent::Death(Death {
            timestamp,
            world_id: f.num_or("world_id", 0),
            zone_id: f.num_or("zone_id", 0),
            character_id: f.num("character_id")?,
            attacker_character_id: f.num_or("attacker_character_id", 0),
            attacker_weapon_id: f.num_or("attacker_weapon_id", 0),
            attacker_vehicle_id: f.num_or("attacker_vehicle_id", 0),
            attacker_loadout_id: f.num_or("attacker_loadout_id", 0),
            character_loadout_id: f.num_or("character_loadout_id", 0),
            is_headshot: f.flag("is_headshot"),
            team_id: f.opt("team_id"),
            attacker_team_id: f.opt("attacker_team_id"),
        }),
        "VehicleDestroy" => GameEvent::VehicleDestroy(VehicleDestroy {
            timestamp,
            world_id: f.num_or("world_id", 0),
            zone_id: f.num_or("zone_id", 0),
            character_id: f.num_or("character_id", 0),
            attacker_character_id: f.num_or("attacker_character_id", 0),
            attacker_weapon_id: f.num_or("attacker_weapon_id", 0),
            attacker_vehicle_id: f.num_or("attacker_vehicle_id", 0),
            vehicle_id: f.num_or("vehicle_id", 0),
            faction_id: f.opt("faction_id"),
            team_id: f.opt("team_id"),
            attacker_team_id: f.opt("attacker_team_id"),
        }),
        "GainExperience" => GameEvent::GainExperience(GainExperience {
            timestamp,
            world_id: f.num_or("world_id", 0),
            zone_id: f.num_or("zone_id", 0),
            character_id: f.num("character_id")?,
            other_id: f.num_or("other_id", 0),
            experience_id: f.num("experience_id")?,
            amount: f.num_or("amount", 0),
            loadout_id: f.num_or("loadout_id", 0),
        }),
        "PlayerLogin" | "PlayerLogout" => {
            let s = PlayerSession {
                timestamp,
                world_id: f.num_or("world_id", 0),
                character_id: f.num("character_id")?,
            };
            if name == "PlayerLogin" {
                GameEvent::PlayerLogin(s)
            } else {
                GameEvent::PlayerLogout(s)
            }
        }
        "FacilityControl" => GameEvent::FacilityControl(FacilityControl {
            timestamp,
            world_id: f.num_or("world_id", 0),
            zone_id: f.num_or("zone_id", 0),
            facility_id: f.num_or("facility_id", 0),
            new_faction_id: f.num_or("new_faction_id", 0),
            old_faction_id: f.num_or("old_faction_id", 0),
            outfit_id: f.num_or("outfit_id", 0),
        }),
        "PlayerFacilityCapture" | "PlayerFacilityDefend" => {
            let p = PlayerFacility {
                timestamp,
                world_id: f.num_or("world_id", 0),
                zone_id: f.num_or("zone_id", 0),
                character_id: f.num("character_id")?,
                facility_id: f.num_or("facility_id", 0),
                outfit_id: f.num_or("outfit_id", 0),
            };
            if name == "PlayerFacilityCapture" {
                GameEvent::PlayerFacilityCapture(p)
            } else {
                GameEvent::PlayerFacilityDefend(p)
            }
        }
        "ItemAdded" => GameEvent::ItemAdded(ItemAdded {
            timestamp,
            world_id: f.num_or("world_id", 0),
            character_id: f.num("character_id")?,
            item_id: f.num_or("item_id", 0),
            item_count: f.num_or("item_count", 1),
            context: f.raw("context").unwrap_or_default().to_owned(),
        }),
        "AchievementEarned" => GameEvent::AchievementEarned(AchievementEarned {
            timestamp,
            world_id: f.num_or("world_id", 0),
            character_id: f.num("character_id")?,
            achievement_id: f.num_or("achievement_id", 0),
        }),
        "BattleRankUp" => GameEvent::BattleRankUp(BattleRankUp {
            timestamp,
            world_id: f.num_or("world_id", 0),
            character_id: f.num("character_id")?,
            battle_rank: f.num_or("battle_rank", 0),
        }),
        other => GameEvent::Other {
            event_name: other.to_owned(),
            timestamp,
        },
    };
    Ok(ev)
}

/// Translates a complete ESS message (text frame).
pub fn parse_message(text: &str) -> Result<EssMessage, ParseError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ParseError::Json(e.to_string()))?;
    if v.get("subscription").is_some() {
        return Ok(EssMessage::Subscription(v["subscription"].clone()));
    }
    match v.get("type").and_then(Value::as_str) {
        Some("serviceMessage") => {
            let payload = v.get("payload").ok_or(ParseError::Field("payload"))?;
            parse_payload(payload).map(EssMessage::Event)
        }
        Some("heartbeat") => Ok(EssMessage::Heartbeat),
        Some("connectionStateChanged") => {
            let connected = matches!(
                v.get("connected"),
                Some(Value::String(s)) if s == "true"
            ) || v.get("connected") == Some(&Value::Bool(true));
            Ok(EssMessage::ConnectionState { connected })
        }
        _ => Ok(EssMessage::Ignored),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_death_with_team_ids() {
        let m = r#"{"payload":{"attacker_character_id":"5428000000000000001","attacker_fire_mode_id":"1","attacker_loadout_id":"15","attacker_vehicle_id":"0","attacker_weapon_id":"7214","attacker_team_id":"3","character_id":"5428000000000000002","character_loadout_id":"8","event_name":"Death","is_critical":"0","is_headshot":"1","team_id":"1","timestamp":"1795200000","world_id":"10","zone_id":"2"},"service":"event","type":"serviceMessage"}"#;
        let EssMessage::Event(GameEvent::Death(d)) = parse_message(m).unwrap() else {
            panic!("not a Death")
        };
        assert!(d.is_headshot);
        assert_eq!(d.attacker_weapon_id, 7214);
        assert_eq!(d.team_id, Some(1));
        assert_eq!(d.attacker_team_id, Some(3));
    }

    #[test]
    fn heartbeat_and_unknown() {
        assert_eq!(
            parse_message(r#"{"online":{},"service":"event","type":"heartbeat"}"#).unwrap(),
            EssMessage::Heartbeat
        );
        assert_eq!(
            parse_message(r#"{"send this for help":{"service":"event","action":"help"}}"#).unwrap(),
            EssMessage::Ignored
        );
        let m = r#"{"payload":{"event_name":"NewThing","timestamp":"5"},"type":"serviceMessage"}"#;
        assert!(matches!(
            parse_message(m).unwrap(),
            EssMessage::Event(GameEvent::Other { .. })
        ));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_message("{nope").is_err());
        let m = r#"{"payload":{"event_name":"Death","timestamp":"x"},"type":"serviceMessage"}"#;
        assert_eq!(parse_message(m), Err(ParseError::Field("timestamp")));
    }

    #[test]
    fn connection_state() {
        let m = r#"{"connected":"true","service":"push","type":"connectionStateChanged"}"#;
        assert_eq!(
            parse_message(m).unwrap(),
            EssMessage::ConnectionState { connected: true }
        );
    }
}
