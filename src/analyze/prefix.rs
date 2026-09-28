//! One immutable analysis checkpoint per development worker. No AST borrows
//! survive a request. Every checkpoint module must still match, including its
//! imports and entry role; their discovery order may differ between applications.
use super::Report;
use crate::{names, project::Graph};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::{BTreeMap, BTreeSet}, rc::Rc};
type Key = [u8; 32];
#[derive(Clone, Default)]
pub(super) struct State {
    pub symbols: names::Symbols,
    pub types: crate::types::Catalog,
    pub engine: Option<crate::unify::Engine>,
    pub global_types: BTreeMap<names::SymbolId, crate::unify::Scheme>,
    pub type_interfaces: BTreeMap<String, Key>,
    pub name_interfaces: BTreeMap<String, Rc<names::Interface>>,
    pub implicit_environment: Option<names::Environment>,
    pub interfaces: BTreeMap<String, crate::fixity::Table>,
    pub report: Report,
    pub layouts: crate::js_names::Layouts,
    pub retained_types: usize,
}
struct Checkpoint { prefix: Vec<Key>, state: State }
#[derive(Default)]
struct Store { identity: u64, global: Key, owner: Vec<String>, last_entry: Vec<String>, graph_size: usize, previous: Vec<Key>, checkpoint: Option<Checkpoint>, resumed: usize, captures: usize }
thread_local! { static STORE: RefCell<Store> = RefCell::new(Store::default()); }
pub(super) struct Plan {
    pub restored: Option<State>,
    pub start: usize,
    reused: BTreeSet<Key>,
    pub reordered: bool,
    pub capture_at: Option<usize>,
    keys: Vec<Key>,
}
fn add(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes()); hash.update(bytes);
}
fn signatures(graph: &Graph) -> (Key, Vec<Key>) {
    let mut global = Sha256::new();
    for (path, source) in &graph.manifests {
        add(&mut global, path.as_os_str().as_encoded_bytes()); add(&mut global, source.as_bytes());
    }
    let mut kernels = Vec::new();
    let keys = graph.modules.iter().map(|module| {
        let mut hash = Sha256::new();
        for text in [&module.owner, &module.name] { add(&mut hash, text.as_bytes()); }
        add(&mut hash, &crate::cache::content_digest(module.source.as_bytes()));
        add(&mut hash, module.path.as_os_str().as_encoded_bytes());
        add(&mut hash, &[module.kernel as u8]);
        add(&mut hash, &serde_json::to_vec(&module.missing_header).unwrap());
        add(&mut hash, &serde_json::to_vec(&module.imports).unwrap());
        // Entry status changes main/flags validation and generated exports.
        add(&mut hash, &[graph.entries.contains(&format!("{}:{}", module.owner, module.name)) as u8]);
        let key: Key = hash.finalize().into();
        if module.kernel { kernels.push(key); }
        key
    }).collect();
    kernels.sort();
    for key in kernels { add(&mut global, &key); }
    (global.finalize().into(), keys)
}
pub(super) fn prepare(graph: &Graph, primary: bool) -> Option<Plan> {
    let identity = crate::session_cache::identity()?;
    let (global, keys) = signatures(graph);
    Some(STORE.with(|store| {
        let mut store = store.borrow_mut();
        if store.identity != identity {
            *store = Store { identity, ..Store::default() };
        }
        // A multi-entry build must not replace a valuable large checkpoint
        // with each smaller app. Keep one state only. Two consecutive requests
        // for another entry allow the cache to follow the developer's focus.
        let same_owner = store.owner == graph.entries;
        let admit = primary && (store.previous.is_empty() || same_owner
            || graph.modules.len() > store.graph_size || store.last_entry == graph.entries);
        if primary { store.last_entry = graph.entries.clone(); }
        if admit && (!same_owner || store.global != global) {
            store.previous.clear();
            // Entry groups can change while their dependency-closed checkpoint
            // stays valid. Preserve it until the complete signature check below;
            // a manifest or runtime change still invalidates it immediately.
            if store.global != global { store.checkpoint = None; }
            store.global = global;
            store.owner = graph.entries.clone();
        }
        let changed = store.previous.iter().zip(&keys).position(|(a,b)| a != b)
            .unwrap_or(store.previous.len().min(keys.len()));
        let capture = if store.previous.is_empty() {
            let paths = crate::session_cache::changed_paths();
            let cwd = std::env::current_dir().unwrap_or_default();
            graph.modules.iter().position(|module| !module.kernel && paths.iter().any(|path| cwd.join(path) == cwd.join(&module.path)))
                .or_else(|| graph.modules.iter().rposition(|module| !module.kernel))
        } else { Some(changed.min(keys.len().saturating_sub(1))) };
        // The captured traversal is dependency-closed. Reuse it in another
        // traversal only when EVERY captured module is present unchanged. No
        // state is projected or partially reused. Entry exports must stay in
        // traversal order, so only export-free states allow this reordering.
        let available: BTreeSet<_> = keys.iter().copied().collect();
        let checkpoint = store.checkpoint.as_ref().filter(|checkpoint| store.global == global && (
            keys.starts_with(&checkpoint.prefix) || (checkpoint.state.report.main_export.is_none()
                && checkpoint.prefix.iter().all(|key| available.contains(key)))
        ));
        let reordered = checkpoint.is_some_and(|checkpoint| !keys.starts_with(&checkpoint.prefix));
        let reused = checkpoint.map_or_else(BTreeSet::new, |checkpoint| checkpoint.prefix.iter().copied().collect());
        let start = keys.iter().take_while(|key| reused.contains(*key)).count();
        let restored = checkpoint.map(|checkpoint| {
            let mut state = checkpoint.state.clone();
            state.report.compiled_modules.clear();
            state.report.module_timings.clear();
            state.report.cached_type_modules = state.report.elm_modules;
            state.report.cached_generated_modules = state.report.generated_modules;
            state.report.resumed_modules = state.report.elm_modules;
            state
        });
        // Keep a valid common checkpoint on an ownership switch. Advancing it
        // into a formerly shared module would make the next group re-import
        // everything; capturing before `start` would mislabel restored state.
        let capture_at = (admit && !reordered && (same_owner || checkpoint.is_none())).then_some(capture).flatten()
            .filter(|index| checkpoint.is_none() || *index > start);
        store.resumed += restored.as_ref().map_or(0, |state| state.report.elm_modules);
        if admit {
            store.graph_size = graph.modules.len();
            if restored.is_none() { store.checkpoint = None; }
            store.previous = keys.clone();
        }
        Plan { restored, start, capture_at, keys, reused, reordered }
    }))
}
impl Plan {
    pub fn skips(&self, index: usize) -> bool { self.reused.contains(&self.keys[index]) }
    pub fn capture(&self, index: usize, state: State) {
        if !state.report.generation_errors.is_empty() { return; }
        STORE.with(|store| {
            let mut store = store.borrow_mut(); store.captures += 1;
            store.checkpoint = Some(Checkpoint { prefix: self.keys[..index].to_vec(), state });
        });
    }
}

pub(super) fn statistics() -> serde_json::Value { STORE.with(|store| { let store = store.borrow(); serde_json::json!({"resumed_modules":store.resumed,"captures":store.captures,"checkpoint_entry":store.owner,"checkpoint_modules":store.checkpoint.as_ref().map_or(0, |checkpoint| checkpoint.prefix.len())}) }) }

#[cfg(test)]
mod tests {
    use super::*;
    fn graph() -> Graph {
        Graph { import_errors: vec![], entry: "application:B".into(), entries: vec!["application:B".into()], manifests: vec![("elm.json".into(), "manifest".into())], modules: ["A", "B"].into_iter().map(|name| crate::project::Module {
            missing_header: None, owner: "application".into(), name: name.into(), path: format!("src/{name}.elm").into(), imports: vec![], bytes: 0, tokens: 0, kernel: false, source: name.into(),
        }).collect() }
    }
    fn checkpoint(graph: &Graph) {
        let plan = prepare(graph, true).unwrap();
        assert!(plan.restored.is_none());
        let mut state = State::default(); state.report.elm_modules = 1;
        plan.capture(1, state);
    }
    #[test]
    fn reordered_complete_checkpoint_reuses_only_identical_members() {
        let _session = crate::session_cache::scope(true);
        let mut original = graph();
        let mut extra = original.modules[0].clone(); extra.name = "C".into(); extra.source = "C".into(); extra.path = "src/C.elm".into();
        original.modules.insert(1, extra);
        let plan = prepare(&original, true).unwrap();
        plan.capture(2, State::default());
        let mut other = original.clone(); other.modules.swap(0, 1);
        other.entries = vec!["application:Other".into()];
        let plan = prepare(&other, true).unwrap();
        assert!(plan.restored.is_some()); assert!(plan.reordered);
        assert!(plan.skips(0)); assert!(plan.skips(1)); assert!(!plan.skips(2));
        assert!(plan.capture_at.is_none(), "a reordered state is not a prefix of the new traversal");
        other.modules[0].source.push('!');
        assert!(prepare(&other, false).unwrap().restored.is_none());
        other.modules.remove(0);
        assert!(prepare(&other, false).unwrap().restored.is_none());
    }
    #[test]
    fn reordered_checkpoint_skips_interleaved_members_but_not_new_modules() {
        let _session = crate::session_cache::scope(true);
        let mut original = graph();
        let mut extra = original.modules[0].clone(); extra.name = "C".into(); extra.source = "C".into(); extra.path = "src/C.elm".into();
        original.modules.insert(1, extra.clone());
        let plan = prepare(&original, true).unwrap(); plan.capture(2, State::default());
        extra.name = "New".into(); extra.source = "New".into(); extra.path = "src/New.elm".into();
        let mut other = original.clone(); other.modules.swap(0, 1); other.modules.insert(1, extra);
        let plan = prepare(&other, false).unwrap();
        assert!(plan.reordered); assert_eq!(plan.start, 1);
        assert!(!plan.skips(1)); assert!(plan.skips(2)); assert!(!plan.skips(3));
    }
    #[test]
    fn reordered_checkpoint_never_replays_entry_exports() {
        let _session = crate::session_cache::scope(true);
        let original = graph();
        let plan = prepare(&original, true).unwrap();
        let mut state = State::default(); state.report.main_export = Some("export".into());
        plan.capture(2, state);
        let mut other = original.clone(); other.modules.swap(0, 1);
        assert!(prepare(&other, false).unwrap().restored.is_none());
    }
    #[test]
    fn only_unchanged_prefixes_resume_including_filtered_error_passes() {
        let _session = crate::session_cache::scope(true);
        let mut graph = graph(); checkpoint(&graph);
        graph.modules[1].source = "changed later".into();
        let plan = prepare(&graph, true).unwrap();
        assert_eq!(plan.start, 1); assert!(plan.restored.is_some());
        graph.modules.pop(); graph.entries.clear();
        let plan = prepare(&graph, false).unwrap();
        assert_eq!(plan.start, 1); assert!(plan.restored.is_some());
        graph.modules[0].source = "changed earlier".into();
        assert!(prepare(&graph, true).unwrap().restored.is_none());
    }
    #[test]
    fn watch_hint_places_first_checkpoint_but_never_overrides_source_validation() {
        let _session = crate::session_cache::scope(true);
        let mut graph = graph();
        crate::session_cache::set_changed_paths(vec![graph.modules[0].path.clone()]);
        assert_eq!(prepare(&graph, true).unwrap().capture_at, Some(0));
        // Even a misleading hint cannot reuse a checkpoint with changed input.
        let plan = prepare(&graph, true).unwrap(); plan.capture(1, State::default());
        crate::session_cache::set_changed_paths(vec![graph.modules[1].path.clone()]);
        graph.modules[0].source.push('!');
        assert!(prepare(&graph, true).unwrap().restored.is_none());
    }
    #[test]
    fn manifests_imports_entry_roles_paths_and_kernels_invalidate() {
        for change in 0..5 {
            let _session = crate::session_cache::scope(true);
            let mut graph = graph(); checkpoint(&graph);
            match change {
                0 => graph.manifests[0].1.push('!'),
                1 => graph.modules[0].imports.push("application:C".into()),
                2 => graph.entries.push("application:A".into()),
                3 => graph.modules[0].path = "other/A.elm".into(),
                _ => { let mut kernel = graph.modules[1].clone(); kernel.kernel = true; graph.modules.push(kernel); }
            }
            assert!(prepare(&graph, true).unwrap().restored.is_none(), "change {change}");
        }
    }
    #[test]
    fn smaller_entry_does_not_evict_larger_checkpoint_and_focus_can_change() {
        let _session = crate::session_cache::scope(true);
        let original = graph(); checkpoint(&original);
        let mut smaller = original.clone(); smaller.modules.remove(0);
        smaller.modules[0].name = "Small".into();
        // The entry name distinguishes the real application, even when its
        // imported modules overlap with another app's graph.
        smaller.entry = "application:Small".into();
        smaller.entries = vec![smaller.entry.clone()];
        assert!(prepare(&smaller, true).unwrap().restored.is_none());
        assert_eq!(prepare(&original, true).unwrap().start, 1);
        assert!(prepare(&smaller, true).unwrap().capture_at.is_none());
        assert!(prepare(&smaller, true).unwrap().capture_at.is_some());
    }
    #[test]
    fn preserved_checkpoint_still_rejects_changed_source_after_an_entry_switch() {
        let _session = crate::session_cache::scope(true);
        let mut original = graph(); checkpoint(&original);
        let mut smaller = original.clone(); smaller.modules.remove(0);
        smaller.modules[0].name = "Small".into();
        smaller.entries = vec!["application:Small".into()];
        prepare(&smaller, true).unwrap();
        original.modules[0].source.push('!');
        assert!(prepare(&original, true).unwrap().restored.is_none());
    }

    #[test]
    fn entry_group_switches_keep_a_verified_common_checkpoint_without_advancing_it() {
        let _session = crate::session_cache::scope(true);
        let mut full = graph();
        for name in ["C", "D"] {
            let mut module = full.modules[1].clone();
            module.name = name.into(); module.path = format!("src/{name}.elm").into(); module.source = name.into();
            full.modules.push(module); full.entries.push(format!("application:{name}"));
        }
        checkpoint(&full);
        let mut smaller = full.clone(); smaller.modules.remove(1);
        smaller.entries.remove(0); smaller.entry = smaller.entries[0].clone();
        crate::session_cache::set_changed_paths(vec![smaller.modules.last().unwrap().path.clone()]);
        assert!(prepare(&smaller, true).unwrap().restored.is_some());
        let changed_owner = prepare(&smaller, true).unwrap();
        assert!(changed_owner.restored.is_some(), "switching owners must not discard a valid dependency-closed state");
        assert!(changed_owner.capture_at.is_none(), "do not move the common checkpoint forward merely because ownership changed");
        let restored = prepare(&full, true).unwrap();
        assert!(restored.restored.is_some()); assert!(restored.capture_at.is_none());
        full.modules[0].source.push('!');
        assert!(prepare(&full, true).unwrap().restored.is_none(), "a changed checkpoint member still invalidates the state");
    }

}
