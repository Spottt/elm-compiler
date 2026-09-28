//! Internal probe for comparing extracted metadata to Elm's --debug output.
use planexpo_elm::{
    debug_metadata,
    names::{self, Environment, Symbols},
    parser::parse,
    types::{self, Catalog},
    unify::{Engine, Term},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected declarations file")?;
    let source = std::fs::read_to_string(path)?;
    let ast = parse(&source)?;
    let mut symbols = Symbols::default();
    let mut engine = Engine::new(types::builtins(&mut symbols));
    engine.track_debug_types();
    let mut environment = Environment::default();
    environment.builtin_list(&mut symbols);
    let (interface, resolved) =
        names::resolve(&ast, "author/project:Main", environment, &mut symbols)?;
    let mut catalog = Catalog::default();
    catalog.register_type_definitions(
        &ast,
        &resolved,
        &symbols,
        "author/project:Main",
        &mut engine,
    )?;
    catalog.compact(&mut engine, &mut Default::default());
    let message = engine.term(Term::Named(interface.types["Msg"], vec![]));
    println!(
        "{}",
        debug_metadata::extract(&mut engine, &catalog, &symbols, message)?
    );
    Ok(())
}
