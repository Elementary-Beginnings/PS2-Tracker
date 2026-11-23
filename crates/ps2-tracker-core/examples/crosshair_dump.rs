//! Writes the built-in crosshairs (generated in `crosshair.rs`, CC0) as PNG
//! files into a folder, e.g. for a preview or to use them elsewhere.
//!
//! cargo run -p ps2-tracker-core --example crosshair_dump -- <folder>

fn main() {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "crosshairs".into()),
    );
    std::fs::create_dir_all(&dir).expect("create folder");
    for (name, bytes) in ps2_tracker_core::crosshair::standard_crosshairs().expect("generate") {
        std::fs::write(dir.join(&name), bytes).expect("write");
        println!("{name}");
    }
}
