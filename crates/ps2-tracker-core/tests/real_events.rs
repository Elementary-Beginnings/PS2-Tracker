//! Tests with real ESS messages (recorded 2026-11-20, servers Wainwright/Osprey).
//! Structure and values as recorded; the character IDs are replaced by
//! pseudonyms (same prefix and length, consistent within the file).

use ps2_tracker_core::events::{EssMessage, GameEvent, parse_message};
use ps2_tracker_core::session::{Outcome, Session, SessionConfig};

const FIXTURE: &str = include_str!("fixtures/ess_real_2026-11-20.jsonl");

fn messages() -> Vec<EssMessage> {
    FIXTURE
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| parse_message(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

#[test]
fn every_real_message_parses() {
    let msgs = messages();
    assert_eq!(msgs.len(), FIXTURE.lines().count());
    let events: Vec<&GameEvent> = msgs
        .iter()
        .filter_map(|m| match m {
            EssMessage::Event(e) => Some(e),
            _ => None,
        })
        .collect();
    assert!(events.iter().any(|e| matches!(e, GameEvent::Death(_))));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::VehicleDestroy(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::GainExperience(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::PlayerLogin(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::PlayerLogout(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::FacilityControl(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::PlayerFacilityCapture(_)))
    );
    assert!(!events.iter().any(|e| matches!(e, GameEvent::Other { .. })));
    assert!(
        msgs.iter()
            .any(|m| matches!(m, EssMessage::ConnectionState { connected: true }))
    );
}

#[test]
fn real_deaths_have_team_ids() {
    for m in messages() {
        if let EssMessage::Event(GameEvent::Death(d)) = m {
            assert!(d.team_id.is_some() && d.attacker_team_id.is_some(), "{d:?}");
            assert!(d.timestamp > 1_700_000_000);
        }
    }
}

#[test]
fn session_from_real_attacker_perspective() {
    let msgs = messages();
    let first_kill = msgs
        .iter()
        .find_map(|m| match m {
            EssMessage::Event(GameEvent::Death(d))
                if d.attacker_character_id != d.character_id && d.attacker_team_id != d.team_id =>
            {
                Some(d.clone())
            }
            _ => None,
        })
        .expect("fixture contains a kill");
    let me = first_kill.attacker_character_id;
    let mut s = Session::new(
        me,
        "Fixture",
        None,
        first_kill.timestamp,
        SessionConfig::default(),
    );
    let mut kills = 0;
    for m in &msgs {
        if let EssMessage::Event(e) = m {
            for o in s.apply(e) {
                if matches!(o, Outcome::Kill { .. }) {
                    kills += 1;
                }
            }
        }
    }
    assert!(kills >= 1);
    assert_eq!(s.stats.kills, kills);
    // The attacker's kill XP event (experience_id 1) is in the fixture as well.
    assert!(s.stats.xp > 0);
}
