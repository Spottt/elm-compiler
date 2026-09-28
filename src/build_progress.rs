//! Scoped observations of freshly checked modules. Library callers are silent.
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

type Listener = Rc<dyn Fn(&str)>;
thread_local! {
    static LISTENER: RefCell<Option<Listener>> = RefCell::new(None);
}

pub struct Subscription {
    previous: Option<Listener>,
    // A subscription must be dropped on the thread where it was installed.
    _thread: PhantomData<Rc<()>>,
}

pub fn subscribe(listener: impl Fn(&str) + 'static) -> Subscription {
    Subscription {
        previous: LISTENER.with(|slot| slot.replace(Some(Rc::new(listener)))),
        _thread: PhantomData,
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        LISTENER.with(|slot| slot.replace(self.previous.take()));
    }
}

pub(crate) fn checked(module: &str) {
    let listener = LISTENER.with(|slot| slot.borrow().clone());
    if let Some(listener) = listener {
        listener(module);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_subscription_restores_previous_listener_even_on_unwind() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let outer_seen = seen.clone();
        let outer = subscribe(move |name| outer_seen.borrow_mut().push(name.to_owned()));
        checked("first");
        let _ = std::panic::catch_unwind(|| {
            let _inner = subscribe(|_| {});
            checked("hidden");
            panic!("aborted build");
        });
        checked("second");
        drop(outer);
        checked("silent");
        assert_eq!(*seen.borrow(), ["first", "second"]);
    }
}
