//! Keys of the Recursion voice pack format (`<NAME>` in `.rtst_vpk`).
//!
//! Source: only a real voice pack owned by the user (as of 2014) and its read-me –
//! interoperability with user data. The Recursion installer and program files were
//! neither unpacked nor decompiled.
//!
//! The keys are adopted as our own event IDs. **What** triggers a key is not in the
//! pack; trigger conditions are our own, openly documented definitions in
//! `assets/triggers.default.toml` and `docs/voicepacks.md`.

/// File extension of the definition file.
pub const VPK_EXTENSION: &str = "rtst_vpk";

/// Format confirmed with a real pack (structure, fields, path rules).
pub const CONFIRMED: bool = true;

/// All 96 keys of the sample pack, in its order (used for export).
pub const KEYS: [&str; 96] = [
    "EXCELLENT",
    "OUTSTANDING",
    "SUPERB",
    "HEAD_SHOT",
    "WELCOME_KILL",
    "HUMILIATION",
    "ROAD_KILL",
    "PIZZA_DELIVERY",
    "KILLING_SPREE",
    "DOMINATING",
    "UNSTOPPABLE",
    "WICKED_SICK",
    "GOD_LIKE",
    "ONE_PLATOON",
    "ONE_MAN_EMPIRE",
    "ONE_MAN_ONE_MISSION",
    "ONE_MAN_FARM",
    "DEATH_STREAK",
    "BEING_FARMED",
    "DEATH_INSULT",
    "PAIN_REMAINS",
    "WATCH_YOUR_STEP",
    "TEAM_KILLER",
    "PREVENT_FF",
    "WHOSE_SIDE",
    "ACCURACY",
    "IMPRESSIVE",
    "SHARP_SHOOTER",
    "MARKSMAN",
    "HEAD_HUNTER",
    "LOBOTOMIST",
    "ROAD_RAGE",
    "STEEL_RAIN",
    "PRECIPICE",
    "SUICIDE_BOMBER",
    "ROCKET_PRIMARY",
    "MASTER_JUGGLER",
    "COMMISSIONER",
    "EXECUTIONER",
    "JUGGERNAUGHT",
    "MAXIMUM_DAMAGE",
    "GUN_BIGGER",
    "LAWNMOWER",
    "HARVESTER",
    "PRESENT",
    "ICED",
    "FIRE",
    "CARE_BEAR",
    "QUAD_DAMAGE",
    "ACE",
    "TOP_GUN",
    "SCOURGE_OF_SKIES",
    "DEATH_FROM_ABOVE",
    "ARMORED_ASSAULT",
    "BLITZKRIEG",
    "BOMBARDIER",
    "DECIMATION",
    "FLYSWATTER",
    "NICE_THINGS",
    "KAMIKAZE",
    "REPEAT_CUSTOMER",
    "THINK_THEYD_LEARN",
    "DOMINATION",
    "RECURSION",
    "RECURSIVE_RECURSION",
    "REVENGE",
    "EXPLODING_FIST",
    "KNIFE_FIGHT",
    "EASY_MODE",
    "HARD_MODE",
    "NIGHTMARE",
    "IMPRESS_MYSELF",
    "HIGH_ROLLER",
    "FATALITY",
    "SUPERMAN",
    "BATMAN",
    "VIP_KILL",
    "VIP_TEAM_KILL",
    "CHEATER",
    "RAGE_QUIT",
    "FIRST_BLOOD",
    "SUBMISSION",
    "SHOOT_NEEDLE",
    "MUTUAL",
    "TERRORISTS",
    "EXPLOSIVE_EFFICIENCY",
    "FRAGASM",
    "DOUBLE_KILL",
    "TRIPLE_KILL",
    "MULTI_KILL",
    "MEGA_KILL",
    "ULTRA_KILL",
    "MONSTER_KILL",
    "LUDICROUS_KILL",
    "HOLY_SHIT",
    "HOLY_SHIT_2",
];

pub fn is_key(name: &str) -> bool {
    KEYS.contains(&name)
}

/// Readable label: "HEAD_SHOT" → "Head Shot".
pub fn label(key: &str) -> String {
    key.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let lower = w.to_lowercase();
            let mut c = lower.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_labelled() {
        let mut k = KEYS.to_vec();
        k.sort_unstable();
        k.dedup();
        assert_eq!(k.len(), 96);
        assert_eq!(label("HEAD_SHOT"), "Head Shot");
        assert_eq!(label("HOLY_SHIT_2"), "Holy Shit 2");
    }
}
