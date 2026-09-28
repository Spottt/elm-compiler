//! Optional, worker-scoped successful outputs. Keys still bind all source bytes,
//! graph resolution, compiler identity and mode. Never used for typed artifacts.
use std::{cell::RefCell, collections::VecDeque, path::{Path, PathBuf}};
const LIMIT: usize = 64 * 1024 * 1024;
const SLOTS: usize = 32;
struct Entry { path: PathBuf, key: [u8;32], trivia: Option<[u8;32]>, bytes: Vec<u8> }
#[derive(Default)]
struct Memory { entries: VecDeque<Entry>, bytes: usize }
thread_local! { static ACTIVE: RefCell<Option<Memory>> = const { RefCell::new(None) }; }
/// Thread-local guard deliberately cannot move to another thread.
pub struct Scope { previous: Option<Memory>, _thread: std::marker::PhantomData<std::rc::Rc<()>> }
pub fn scope(enabled: bool) -> Scope {
    Scope { previous: ACTIVE.with(|s| s.replace(enabled.then(Memory::default))), _thread: std::marker::PhantomData }
}
impl Drop for Scope { fn drop(&mut self) { ACTIVE.with(|s| s.replace(self.previous.take())); } }
pub(super) fn load(path: &Path, key: &[u8;32], trivia: bool) -> Option<Vec<u8>> {
    ACTIVE.with(|s| {
        let mut s = s.borrow_mut(); let memory = s.as_mut()?;
        let index = memory.entries.iter().position(|e| e.path == path && if trivia { e.trivia.as_ref() == Some(key) } else { &e.key == key })?;
        let entry = memory.entries.remove(index)?;
        let bytes = entry.bytes.clone(); memory.entries.push_back(entry); Some(bytes)
    })
}
pub(super) fn invalidate(path: &Path) {
    ACTIVE.with(|s| {
        if let Some(memory) = s.borrow_mut().as_mut()
            && let Some(index) = memory.entries.iter().position(|e| e.path == path) {
            memory.bytes -= memory.entries.remove(index).unwrap().bytes.len();
        }
    });
}
pub(super) fn store(path: &Path, key: [u8;32], trivia: Option<[u8;32]>, bytes: &[u8]) -> bool {
    ACTIVE.with(|s| {
        let mut s = s.borrow_mut(); let Some(memory) = s.as_mut() else { return false; };
        if let Some(index) = memory.entries.iter().position(|e| e.path == path) {
            memory.bytes -= memory.entries.remove(index).unwrap().bytes.len();
        }
        if bytes.len() > LIMIT { return false; }
        while memory.bytes + bytes.len() > LIMIT || memory.entries.len() >= SLOTS {
            memory.bytes -= memory.entries.pop_front().unwrap().bytes.len();
        }
        memory.bytes += bytes.len();
        memory.entries.push_back(Entry { path: path.to_owned(), key, trivia, bytes: bytes.to_vec() }); true
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_and_trivia_keys_are_independent_and_scoped() {
        let path = Path::new("entry");
        let _scope = scope(true);
        assert!(store(path, [1;32], Some([2;32]), b"good"));
        assert_eq!(load(path, &[1;32], false).unwrap(), b"good");
        assert_eq!(load(path, &[2;32], true).unwrap(), b"good");
        assert!(load(path, &[2;32], false).is_none());
        assert!(load(path, &[3;32], true).is_none());
        { let _nested = scope(true); assert!(load(path, &[1;32], false).is_none()); }
        assert_eq!(load(path, &[1;32], false).unwrap(), b"good");
        assert!(store(path, [3;32], None, b"next"));
        assert!(load(path, &[1;32], false).is_none());
        assert!(load(path, &[2;32], true).is_none());
        drop(_scope); assert!(load(path, &[3;32], false).is_none());
    }
    #[test]
    fn eviction_bounds_slots_and_payloads_and_keeps_recent_hits() {
        let _scope = scope(true);
        for n in 0..SLOTS { assert!(store(Path::new(&n.to_string()), [0;32], None, b"x")); }
        assert!(load(Path::new("0"), &[0;32], false).is_some());
        assert!(store(Path::new("extra"), [0;32], None, b"y"));
        assert!(load(Path::new("1"), &[0;32], false).is_none());
        let large = vec![42; LIMIT / 2 + 1];
        assert!(store(Path::new("large1"), [0;32], None, &large));
        assert!(store(Path::new("large2"), [0;32], None, &large));
        assert!(load(Path::new("large1"), &[0;32], false).is_none());
        ACTIVE.with(|s| { let s = s.borrow(); let m = s.as_ref().unwrap(); assert!(m.bytes <= LIMIT); assert_eq!(m.entries.len(), 1); });
    }
}
