//! Converts the snapshot's borrowed `HICON` handles to reactor image sources.
//!
//! The snapshotter already caches the shell icon handle per window. This cache
//! adds the encoded PNG keyed by window and icon handle so a 200 ms refresh
//! does not re-encode every icon. Entries are dropped when a window disappears
//! or changes its icon.

use std::collections::{HashMap, HashSet};

use windows_reactor::EncodedImage;

use crate::logging;
use crate::windows::{WindowIcon, WindowSnapshot};

#[derive(Default)]
pub(super) struct IconSources {
    entries: HashMap<usize, (WindowIcon, EncodedImage)>,
}

impl IconSources {
    /// Updates the cache for a snapshot, encoding only new or changed icons.
    pub fn refresh(&mut self, snapshot: &WindowSnapshot) {
        let mut active = HashSet::with_capacity(snapshot.windows.len());

        for window in &snapshot.windows {
            let Some(icon) = window.icon else {
                continue;
            };
            let key = window.hwnd.0 as usize;
            active.insert(key);

            let unchanged = self
                .entries
                .get(&key)
                .is_some_and(|(cached, _)| *cached == icon);
            if unchanged {
                continue;
            }

            match icon.png_bytes() {
                Some(bytes) => {
                    self.entries.insert(key, (icon, EncodedImage::new(bytes)));
                }
                None => {
                    self.entries.remove(&key);
                    logging::warn(format!(
                        "icon conversion failed for window {:?}",
                        window.hwnd
                    ));
                }
            }
        }

        self.entries.retain(|key, _| active.contains(key));
    }

    /// Returns the encoded icon for a window, if conversion succeeded.
    pub fn get(&self, hwnd: usize) -> Option<&EncodedImage> {
        self.entries.get(&hwnd).map(|(_, image)| image)
    }
}
