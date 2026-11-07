//! Reactor About and Settings windows.
//!
//! Both dialogs are secondary WinUI windows opened from the pager's context
//! menu. They are ordinary reactor components; no native dialog runtime is
//! involved.
//!
//! The settings dialog edits a working copy and reports its progress through a
//! [`DialogLink`]. The link is a small callback wrapper rather than a direct
//! dependency on the pager module, so a dialog can also run standalone in an
//! example.

pub mod about;
pub(crate) mod placement;
pub mod settings;

use std::rc::Rc;

use crate::settings::Settings;

/// Messages sent from a dialog back to the pager.
#[derive(Clone, Debug)]
pub enum DialogEvent {
    /// A live preview of the working settings. It is not persisted.
    Preview(Settings),
    /// The user pressed OK. The settings must be persisted.
    Applied(Settings),
    /// The dialog was cancelled or closed. Restore these settings without
    /// persisting them.
    Cancelled(Settings),
    /// The dialog window is gone; the pager may open another dialog.
    Closed,
}

/// A shared, identity-comparable callback into the pager's message queue.
#[derive(Clone)]
pub struct DialogLink {
    sink: Rc<dyn Fn(DialogEvent)>,
}

impl DialogLink {
    pub fn new(sink: impl Fn(DialogEvent) + 'static) -> Self {
        Self {
            sink: Rc::new(sink),
        }
    }

    pub fn send(&self, event: DialogEvent) {
        (self.sink)(event);
    }
}

impl PartialEq for DialogLink {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.sink, &other.sink)
    }
}
