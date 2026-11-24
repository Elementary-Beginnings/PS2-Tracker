//! Core of the PS2 Session Tracker – no GUI.
//!
//! All data comes exclusively from the public Daybreak Census API (REST) and the
//! Event Streaming Service (WebSocket). There is no access whatsoever to the game
//! process, its memory or its graphics output.
//!
//! Feature `app` (default): everything the desktop app needs (Census and ESS
//! clients, SQLite store, configuration, voice packs, overlay, online sync).
//! Without it (`default-features = false`) only the session file remains:
//! reading and checking it ([`sessionfile`], `SessionFile::validate`), the
//! session model and its summary ([`session`], [`analysis`]) and the Excel
//! export (`sessionfile::to_xlsx`), with the data types they use.

pub mod achievements;
pub mod analysis;
#[cfg(feature = "app")]
pub mod audiofmt;
pub mod census;
#[cfg(feature = "app")]
pub mod class_icons;
pub mod classes;
#[cfg(feature = "app")]
pub mod config;
#[cfg(feature = "app")]
pub mod crosshair;
#[cfg(feature = "app")]
pub mod ess;
pub mod events;
#[cfg(feature = "app")]
pub mod hitfx;
#[cfg(feature = "app")]
pub mod nrbf;
#[cfg(feature = "app")]
pub mod online_sync;
#[cfg(feature = "app")]
pub mod overlay;
#[cfg(feature = "app")]
pub mod packloader;
#[cfg(feature = "app")]
pub mod pakdoc;
#[cfg(feature = "app")]
pub mod presence;
#[cfg(feature = "app")]
pub mod protowire;
#[cfg(feature = "app")]
pub mod ratelimit;
#[cfg(feature = "app")]
pub mod recursion;
#[cfg(feature = "app")]
pub mod recursion_mapping;
#[cfg(feature = "app")]
pub mod recursion_pak;
#[cfg(feature = "app")]
pub mod service_id;
pub mod session;
pub mod sessionfile;
#[cfg(feature = "app")]
pub mod sharpser;
#[cfg(feature = "app")]
pub mod store;
#[cfg(feature = "app")]
pub mod tones;
#[cfg(feature = "app")]
pub mod tracker;
#[cfg(feature = "app")]
pub mod triggers;
#[cfg(feature = "app")]
pub mod voice;
#[cfg(feature = "app")]
pub mod voicepack;

pub use achievements::{AchievementEngine, RuleSet, Unlocked};
#[cfg(feature = "app")]
pub use census::CensusClient;
pub use census::{CharacterInfo, LifetimeStats, NameCache};
#[cfg(feature = "app")]
pub use config::{Config, Paths};
pub use events::{CharacterId, GameEvent};
pub use session::{Outcome, Session, SessionConfig};
#[cfg(feature = "app")]
pub use store::Store;
#[cfg(feature = "app")]
pub use tracker::{SessionSnapshot, Tracker, TrackerOutput};
