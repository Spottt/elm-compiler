//! Internal differential-test probe; the public elm diff command is separate.
use planexpo_elm::api_diff::{Magnitude, diff};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    if paths.len() != 2 {
        return Err("expected old.json new.json".into());
    }
    let old = serde_json::from_slice(&std::fs::read(&paths[0])?)?;
    let new = serde_json::from_slice(&std::fs::read(&paths[1])?)?;
    let changes = diff(&old, &new)?;
    println!(
        "{}",
        match changes.magnitude() {
            Magnitude::Patch => "PATCH",
            Magnitude::Minor => "MINOR",
            Magnitude::Major => "MAJOR",
        }
    );
    Ok(())
}
