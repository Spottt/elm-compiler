//! Scoped download events, emitted only when package sources are acquired.
use crate::package_solver::Version;
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Started {
        name: String,
        version: Version,
    },
    Finished {
        name: String,
        version: Version,
        success: bool,
    },
}
type Listener = Rc<dyn Fn(Event)>;
thread_local! { static LISTENER: RefCell<Option<Listener>> = RefCell::new(None); }
pub struct Subscription {
    previous: Option<Listener>,
    _thread: PhantomData<Rc<()>>,
}
pub fn subscribe(listener: impl Fn(Event) + 'static) -> Subscription {
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
pub(crate) struct Download {
    listener: Option<Listener>,
    name: String,
    version: Version,
    success: bool,
}
impl Download {
    pub(crate) fn start(name: &str, version: Version) -> Self {
        let listener = LISTENER.with(|slot| slot.borrow().clone());
        if let Some(listener) = &listener {
            listener(Event::Started {
                name: name.into(),
                version,
            });
        }
        Self {
            listener,
            name: name.into(),
            version,
            success: false,
        }
    }
    pub(crate) fn succeeded(&mut self) {
        self.success = true;
    }
}
impl Drop for Download {
    fn drop(&mut self) {
        if let Some(listener) = &self.listener {
            listener(Event::Finished {
                name: self.name.clone(),
                version: self.version,
                success: self.success,
            });
        }
    }
}
