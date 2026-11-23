//! Core of the PS2 Session Tracker – no GUI.
//!
//! All data comes exclusively from the public Daybreak Census API (REST) and the
//! Event Streaming Service (WebSocket). There is no access whatsoever to the game
//! process, its memory or its graphics output.

pub mod achievements;
pub mod analysis;
pub mod audiofmt;
pub mod census;
pub mod class_icons;
pub mod classes;
pub mod config;
pub mod crosshair;
pub mod ess;
pub mod events;
pub mod hitfx;
pub mod nrbf;
pub mod online_sync;
pub mod overlay;
pub mod packloader;
pub mod pakdoc;
pub mod presence;
pub mod protowire;
pub mod ratelimit;
pub mod recursion;
pub mod recursion_mapping;
pub mod recursion_pak;
pub mod service_id;
pub mod session;
pub mod sessionfile;
pub mod sharpser;
pub mod store;
pub mod tones;
pub mod tracker;
pub mod triggers;
pub mod voice;
pub mod voicepack;

pub use achievements::{AchievementEngine, RuleSet, Unlocked};
pub use census::{CensusClient, CharacterInfo, LifetimeStats, NameCache};
pub use config::{Config, Paths};
pub use events::{CharacterId, GameEvent};
pub use session::{Outcome, Session, SessionConfig};
pub use store::Store;
pub use tracker::{SessionSnapshot, Tracker, TrackerOutput};
