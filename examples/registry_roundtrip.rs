//! Read an official cache without modifying it; write its round trip elsewhere.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: registry_roundtrip INPUT OUTPUT".into());
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err("output must not exist".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let registry = planexpo_elm::registry::Registry::decode(&bytes)?;
    let output = registry.encode()?;
    if output != bytes {
        return Err("registry bytes differ after round trip".into());
    }
    std::fs::write(&args[1], output)?;
    println!(
        "{}",
        serde_json::json!({"packages":registry.package_count(),"versions":registry.count(),"identical":true})
    );
    Ok(())
}
