//! Bounded successful analysis proofs shared by a worker's entry graphs.
//! Avoid redundant error collection and replay exact known failures, never
//! accept an initial compilation without analysis. Keys include full transitive sources, not merely public types.
use crate::{project::Graph, typed_cache::TypeCache};
use std::{cell::RefCell, collections::{BTreeMap, BTreeSet, VecDeque}};

const MAX_PROOFS: usize = 8192;
const MAX_FAILURES: usize = 128;
const MAX_FAILURE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Key { source: [u8; 32], entry: bool }
#[derive(Default)]
struct Store { identity: u64, proofs: BTreeSet<Key>, order: VecDeque<Key>, skipped_passes: usize, skipped_modules: usize, failures: BTreeMap<Key, String>, failure_order: VecDeque<Key>, failure_bytes: usize, reused_failures: usize }
thread_local! { static STORE: RefCell<Store> = RefCell::new(Store::default()); }

pub(super) struct Proofs { identity: u64, keys: BTreeMap<String, Key> }
pub(super) fn prepare(graph: &Graph, cache: &TypeCache) -> Option<Proofs> {
    let identity = crate::session_cache::identity()?;
    let keys = graph.modules.iter().filter(|module| !module.kernel).map(|module| {
        let id = format!("{}:{}", module.owner, module.name);
        let key = Key { source: cache.validation_source_key(&id)?, entry: graph.entries.contains(&id) };
        Some((id, key))
    }).collect::<Option<BTreeMap<_, _>>>()?;
    STORE.with(|store| {
        let mut store = store.borrow_mut();
        if store.identity != identity { *store = Store { identity, ..Store::default() }; }
        // Keep proofs from other entries. Each source key already includes the
        // module identity, path, compiler, manifests, kernels and dependencies.
        // The fixed bound below prevents edits/deletions from growing the store.
    });
    Some(Proofs { identity, keys })
}
impl Proofs {
    /// Call only after the initial full analysis succeeds without deferred
    /// generation errors. A failed build must never create validation proofs.
    pub(super) fn remember(&self) {
        STORE.with(|store| {
            let mut store = store.borrow_mut();
            if store.identity == self.identity {
                for key in self.keys.values() {
                    if store.proofs.insert(*key) {
                        store.order.push_back(*key);
                        if store.order.len() > MAX_PROOFS {
                            let oldest = store.order.pop_front().unwrap();
                            store.proofs.remove(&oldest);
                        }
                    }
                }
            }
        });
    }
    /// Only short-circuit to a failure: preceding modules must have current
    /// successful proofs. The caller still scans syntax and collects independent errors.
    pub(super) fn cached_failure(&self, graph: &Graph) -> Option<String> {
        if !graph.import_errors.is_empty() { return None; }
        STORE.with(|store| {
            let mut store = store.borrow_mut();
            if store.identity != self.identity { return None; }
            for module in graph.modules.iter().filter(|module| !module.kernel) {
                if module.missing_header.is_some() { return None; }
                let id = format!("{}:{}", module.owner, module.name);
                let key = self.keys.get(&id)?;
                if key.entry != graph.entries.contains(&id) { return None; }
                if let Some(error) = store.failures.get(key).cloned() {
                    store.reused_failures += 1;
                    return Some(error);
                }
                if !store.proofs.contains(key) { return None; }
            }
            None
        })
    }
    /// Accept only one unambiguously located module after diagnostic rendering.
    /// Unlocated/project errors and multi-module batches are not memoized.
    pub(super) fn remember_failure(&self, graph: &Graph, error: &str) {
        if !graph.import_errors.is_empty() || error.len() > MAX_FAILURE_BYTES / 4 { return; }
        let paths: BTreeSet<String> = if let Some(report) = crate::docs_diagnostic::report_encoded(error) {
            let Some(errors) = report["errors"].as_array() else { return; };
            let Some(paths) = errors.iter().map(|e| e["path"].as_str().map(str::to_owned)).collect::<Option<BTreeSet<_>>>() else { return; };
            paths
        } else {
            graph.modules.iter().filter_map(|module| {
                let path = module.path.to_string_lossy();
                let detail = error.strip_prefix(&format!("{path}:"))?.trim_start();
                let mut parts = detail.split(':');
                parts.next()?.parse::<usize>().ok()?;
                parts.next()?.parse::<usize>().ok()?;
                Some(path.into_owned())
            }).collect()
        };
        if paths.len() != 1 { return; }
        let path = std::path::Path::new(paths.first().unwrap());
        let Some(module) = graph.modules.iter().find(|m| !m.kernel && m.path == path && m.missing_header.is_none()) else { return; };
        let id = format!("{}:{}", module.owner, module.name);
        let Some(key) = self.keys.get(&id).copied() else { return; };
        if key.entry != graph.entries.contains(&id) { return; }
        STORE.with(|store| {
            let mut store = store.borrow_mut();
            if store.identity != self.identity || store.proofs.contains(&key) || store.failures.contains_key(&key) { return; }
            let bytes = error.len() + 128;
            while store.failures.len() >= MAX_FAILURES || store.failure_bytes + bytes > MAX_FAILURE_BYTES {
                let Some(old) = store.failure_order.pop_front() else { break; };
                if let Some(value) = store.failures.remove(&old) { store.failure_bytes -= value.len() + 128; }
            }
            store.failure_order.push_back(key);
            store.failure_bytes += bytes;
            store.failures.insert(key, error.to_owned());
        });
    }
    pub(super) fn can_skip(&self, remaining: &Graph) -> bool {
        if !remaining.import_errors.is_empty() { return false; }
        STORE.with(|store| {
            let mut store = store.borrow_mut();
            if store.identity != self.identity { return false; }
            let modules = remaining.modules.iter().filter(|module| !module.kernel);
            let valid = modules.clone().all(|module| {
                let id = format!("{}:{}", module.owner, module.name);
                module.missing_header.is_none() && self.keys.get(&id).is_some_and(|key| {
                    key.entry == remaining.entries.contains(&id) && store.proofs.contains(key)
                })
            });
            if valid {
                store.skipped_passes += 1;
                store.skipped_modules += modules.count();
            }
            valid
        })
    }
}
pub(super) fn statistics() -> serde_json::Value {
    STORE.with(|store| {
        let store = store.borrow();
        serde_json::json!({"proofs":store.proofs.len(),"proof_limit":MAX_PROOFS,"skipped_passes":store.skipped_passes,"skipped_modules":store.skipped_modules,"failure_entries":store.failures.len(),"failure_bytes":store.failure_bytes,"reused_failures":store.reused_failures})
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kernel::Mode, project::Module, session_cache};
    fn graph() -> Graph {
        Graph { entries: vec!["app:B".into()], entry: "app:B".into(), manifests: vec![("/elm.json".into(), "manifest".into())], import_errors: vec![], modules: vec![
            module("A", &[]), module("B", &["app:A"]), module("Independent", &[]),
        ] }
    }
    fn module(name: &str, imports: &[&str]) -> Module {
        Module { owner:"app".into(), name:name.into(), path:format!("/src/{name}.elm").into(), source:"original".into(), imports:imports.iter().map(|id|(*id).into()).collect(), kernel:false, missing_header:None, bytes:8, tokens:1 }
    }
    fn cache(graph: &Graph) -> TypeCache { TypeCache::new(std::path::Path::new("/unused"), graph, b"compiler", Mode::Development).unwrap() }
    #[test]
    fn cached_failures_require_current_predecessors_and_keep_later_errors_visible() {
        let _session = session_cache::scope(true);
        let mut original = graph(); original.entries = vec!["app:Independent".into()];
        let mut bad = original.clone(); bad.modules[1].source = "bad".into();
        let failed = prepare(&bad, &cache(&bad)).unwrap();
        failed.remember_failure(&bad, "/src/B.elm:1:1: bad");
        assert!(failed.cached_failure(&bad).is_none(), "cold failure does not prove predecessors valid");
        prepare(&original, &cache(&original)).unwrap().remember();
        assert_eq!(failed.cached_failure(&bad).as_deref(), Some("/src/B.elm:1:1: bad"));
        let mut other = bad.clone(); other.entries.clear();
        assert!(prepare(&other, &cache(&other)).unwrap().cached_failure(&other).is_some());
        other.modules[2].source = "independent bad".into();
        let proofs = prepare(&other, &cache(&other)).unwrap();
        assert!(proofs.cached_failure(&other).is_some());
        other.modules.remove(1);
        assert!(!proofs.can_skip(&other), "independent failure still needs collection");
        for variant in 0..6 {
            let mut changed = bad.clone();
            match variant {
                0 => changed.modules[0].source.push('!'),
                1 => changed.modules[1].source.push('!'),
                2 => changed.modules[1].path = "/moved/B.elm".into(),
                3 => changed.manifests[0].1.push('!'),
                4 => changed.entries.push("app:B".into()),
                _ => changed.modules[0].missing_header = Some("Missing".into()),
            }
            assert!(prepare(&changed, &cache(&changed)).unwrap().cached_failure(&changed).is_none(), "variant {variant}");
        }
        assert!(prepare(&original, &cache(&original)).unwrap().cached_failure(&original).is_none());
        let _new_session = session_cache::scope(true);
        assert!(prepare(&bad, &cache(&bad)).unwrap().cached_failure(&bad).is_none());
    }
    #[test]
    fn failure_storage_bounds_count_and_bytes_without_creating_success_proofs() {
        let _session = session_cache::scope(true);
        let original = graph(); prepare(&original, &cache(&original)).unwrap().remember();
        for index in 0..MAX_FAILURES + 5 {
            let mut bad = original.clone(); bad.modules[1].source = index.to_string().into();
            let proofs = prepare(&bad, &cache(&bad)).unwrap();
            let error = format!("/src/B.elm:1:1: {}", "x".repeat(48 * 1024));
            proofs.remember_failure(&bad, &error);
            assert!(proofs.cached_failure(&bad).is_some());
            assert!(!proofs.can_skip(&bad));
            STORE.with(|store| {
                let store = store.borrow();
                assert!(store.failures.len() <= MAX_FAILURES);
                assert!(store.failure_bytes <= MAX_FAILURE_BYTES);
            });
        }
        let mut oldest = original.clone(); oldest.modules[1].source = "0".into();
        assert!(prepare(&oldest, &cache(&oldest)).unwrap().cached_failure(&oldest).is_none());
    }
    #[test]
    fn unchanged_survivors_skip_but_independent_edits_and_transitive_changes_do_not() {
        let _session = session_cache::scope(true);
        let original = graph();
        let old = prepare(&original, &cache(&original)).unwrap();
        assert!(!old.can_skip(&original)); old.remember();
        let mut changed = original.clone(); changed.modules[0].source.push('!');
        let next = prepare(&changed, &cache(&changed)).unwrap();
        assert!(!next.can_skip(&changed));
        let mut survivor = changed.clone(); survivor.modules.retain(|m|m.name=="B");
        assert!(!next.can_skip(&survivor)); // B itself is unchanged; A is not.
        survivor.modules = vec![changed.modules[2].clone()]; survivor.entries.clear();
        assert!(next.can_skip(&survivor));
        changed.modules[2].source.push('!');
        let next = prepare(&changed, &cache(&changed)).unwrap();
        survivor.modules = vec![changed.modules[2].clone()];
        assert!(!next.can_skip(&survivor)); // A second independent error cannot disappear.
    }
    #[test]
    fn environment_roles_missing_headers_and_import_errors_invalidate() {
        let _session = session_cache::scope(true);
        let original = graph(); prepare(&original, &cache(&original)).unwrap().remember();
        for variant in 0..7 {
            let mut changed = original.clone();
            match variant {
                0 => changed.manifests[0].1.push('!'),
                1 => changed.entries.clear(),
                2 => changed.modules[0].path = "/moved/A.elm".into(),
                3 => changed.modules[2].missing_header = Some("Other".into()),
                4 => changed.import_errors.push(serde_json::json!({"path":"/src/Independent.elm"})),
                5 => { let mut kernel=module("Elm.Kernel.Test", &[]); kernel.kernel=true; changed.modules.push(kernel); },
                _ => changed.modules[2].imports.push("app:A".into()),
            }
            assert!(!prepare(&changed, &cache(&changed)).unwrap().can_skip(&changed), "{variant}");
        }
        let other = TypeCache::new(std::path::Path::new("/unused"), &original, b"different compiler", Mode::Development).unwrap();
        assert!(!prepare(&original, &other).unwrap().can_skip(&original));
        for mode in [Mode::Debug, Mode::Production] {
            assert!(prepare(&original, &TypeCache::new(std::path::Path::new("/unused"), &original, b"compiler", mode).unwrap()).is_none());
        }
        assert!(prepare(&original, &cache(&original).for_diagnostics()).is_none());
    }
    #[test]
    fn proofs_survive_entry_switches_but_not_worker_sessions() {
        let original=graph();
        assert!(prepare(&original, &cache(&original)).is_none());
        let _session=session_cache::scope(true);
        prepare(&original, &cache(&original)).unwrap().remember();
        let mut smaller=original.clone(); smaller.modules.truncate(1); smaller.entries.clear();
        prepare(&smaller, &cache(&smaller)).unwrap().remember();
        assert!(prepare(&original, &cache(&original)).unwrap().can_skip(&original));
        let _new_session=session_cache::scope(true);
        assert!(!prepare(&original, &cache(&original)).unwrap().can_skip(&original));
    }
    #[test]
    fn proof_retention_is_bounded_and_eviction_only_loses_reuse() {
        let _session = session_cache::scope(true);
        let original = graph();
        let initial = prepare(&original, &cache(&original)).unwrap();
        initial.remember();
        let keys = (0..MAX_PROOFS + 1).map(|i| {
            let mut source = [0u8; 32];
            source[..8].copy_from_slice(&(i as u64).to_le_bytes());
            (i.to_string(), Key { source, entry: false })
        }).collect();
        Proofs { identity: initial.identity, keys }.remember();
        assert_eq!(statistics()["proofs"], MAX_PROOFS);
        assert!(!initial.can_skip(&original));
        initial.remember();
        assert!(initial.can_skip(&original));
        assert_eq!(statistics()["proofs"], MAX_PROOFS);
    }

}
