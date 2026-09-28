pub mod analyze;
pub mod type_localizer;
pub mod repl_type;
pub mod repl_session;
pub mod repl_compile;
pub mod repl_input;
mod repl_doc;
pub mod ast;
pub mod cache;
pub mod fields;
pub mod fixity;
pub mod lexer;
pub mod module;
pub mod names;
pub mod parser;
pub mod project;
pub mod installation;
mod install_diagnostic;
pub mod types;
pub mod unify;

pub mod infer;

pub mod coverage;

pub mod entry;

pub mod effects;

pub mod kernel;

pub mod literal;

pub mod codegen;

pub mod pattern_codegen;

pub mod js_names;

pub mod module_codegen;

pub mod port_codegen;

pub mod linker;

pub mod tail_codegen;

pub mod typed_artifact;

pub mod type_interface;
pub mod typed_cache;

pub mod html;

pub mod source_error;

mod unicode;

pub mod shader;

pub mod package_solver;

pub mod registry;

pub mod package_network;

mod package_archive;

pub mod dependencies;

pub mod package_resolution;

pub mod outline;

pub mod dependency_error;

pub mod docs;

pub mod docs_diagnostic;

pub mod declaration_diagnostic;
pub mod header_diagnostic;
pub mod space_diagnostic;

pub mod pattern_diagnostic;

pub mod effect_diagnostic;

pub mod number_diagnostic;

pub mod literal_diagnostic;

pub mod control_diagnostic;

pub mod name_diagnostic;
