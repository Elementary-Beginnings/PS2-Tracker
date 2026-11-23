//! Checks the Census REST client against real data.
//!
//! PS2_SERVICE_ID=<id> cargo run -p ps2-tracker-core --example census_probe -- <Charaktername>

use ps2_tracker_core::CensusClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sid = std::env::var("PS2_SERVICE_ID").unwrap_or_else(|_| "example".into());
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "SampleSoldier".into());
    let c = CensusClient::new(&sid)?;

    let t = std::time::Instant::now();
    let ch = c.character_by_name(&name).await?;
    println!("character: {ch:?}  ({:.1?})", t.elapsed());

    let t = std::time::Instant::now();
    let l = c.lifetime(ch.character_id).await?;
    println!(
        "lifetime: kills={} deaths={} KD={:.2} KPM={:.2} HSR={:.1}% playtime={}h score/h={:.0} weapons={}  ({:.1?})",
        l.kills,
        l.deaths,
        l.kd(),
        l.kpm(),
        l.hsr() * 100.0,
        l.play_time_secs / 3600,
        l.score_per_hour(),
        l.weapons.len(),
        t.elapsed()
    );

    let mut top: Vec<_> = l.weapons.iter().collect();
    top.sort_by_key(|w| std::cmp::Reverse(w.1.kills));
    let ids: Vec<u32> = top.iter().take(5).map(|(id, _)| **id).collect();
    let t = std::time::Instant::now();
    let names = c.item_names(&ids).await?;
    println!("top weapons ({:.1?}):", t.elapsed());
    for (id, w) in top.iter().take(5) {
        println!(
            "  {:>6}  {:<32} kills={:>6} hs={:>6}",
            id,
            names.get(id).map(String::as_str).unwrap_or("?"),
            w.kills,
            w.headshots
        );
    }
    let v = c.vehicle_names().await?;
    println!("vehicle names: {} (e.g. 1 = {:?})", v.len(), v.get(&1));
    Ok(())
}
