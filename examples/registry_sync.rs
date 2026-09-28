//! Exercise registry acquisition in an explicitly supplied, isolated ELM_HOME.
use planexpo_elm::package_network::{PACKAGE_SERVER, PackageNetwork};
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let home = args.next().ok_or("usage: registry_sync ELM_HOME")?;
    if args.next().is_some() {
        return Err("usage: registry_sync ELM_HOME".into());
    }
    let state = PackageNetwork::new(PACKAGE_SERVER)?.registry(std::path::Path::new(&home))?;
    println!(
        "{}",
        serde_json::json!({"online":state.online,
        "packages":state.registry.package_count(),"versions":state.registry.count()})
    );
    Ok(())
}
