pub mod analyze;
pub mod build_progress;
pub mod dependency_build;
pub mod package_progress;
pub mod make_output;
pub mod ast;
pub mod edition;
pub mod cache;
pub mod fields;
pub mod fixity;
mod install_diagnostic;
pub mod installation;
pub mod lexer;
pub mod module;
pub mod names;
pub mod parser;
pub mod project;
mod import_diagnostic;
pub mod import_history;
pub mod repl_compile;
mod repl_doc;
pub mod repl_input;
pub mod repl_session;
pub mod repl_type;
pub mod type_localizer;
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
pub mod port_diagnostic;

pub mod linker;

pub mod tail_codegen;

pub mod typed_artifact;

pub mod type_interface;
pub mod typed_cache;

pub mod html;

pub mod source_error;

mod unicode;
mod unicode_15_1;

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

pub mod type_variable_diagnostic;

pub mod api_diff;

pub mod api_diff_render;

pub mod diff_diagnostic;

pub mod package_bump;

pub mod bump_diagnostic;

pub mod debug_metadata;

pub mod package_publish;

pub mod publish_diagnostic;

pub mod publish_git;

pub mod publish_network;

pub mod main_diagnostic;

pub mod coverage_diagnostic;

pub mod local_cycle_diagnostic;

pub mod annotation_diagnostic;

pub mod type_comparison;

mod record_access_diagnostic;

mod infinite_type_diagnostic;

mod outline_diagnostic;
mod outline_json;

mod proxy_environment;

pub mod generated_cache;

pub mod type_graph_compact;

pub mod session_cache;
