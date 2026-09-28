//! Download verified sources into an explicitly supplied ELM_HOME.
use planexpo_elm::{
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_solver::Version,
};
fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [home, name, version] = args.as_slice() else {
        return Err("usage: package_fetch ELM_HOME PACKAGE VERSION".into());
    };
    let root = PackageNetwork::new(PACKAGE_SERVER)?.package(
        std::path::Path::new(home),
        name,
        Version::parse(version)?,
    )?;
    println!("{}", root.display());
    Ok(())
}
