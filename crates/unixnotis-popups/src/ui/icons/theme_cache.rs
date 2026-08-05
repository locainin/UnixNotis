//! Main-thread cache for themed popup icons

use std::collections::{HashMap, VecDeque};

use gtk::IconPaintable;

use super::resolver::resolve_icon_paintable_with_scale;

const THEME_ICON_CACHE_MAX_ENTRIES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ThemeIconSizeKey {
    size: i32,
    scale: i32,
}

impl ThemeIconSizeKey {
    const fn new(size: i32, scale: i32) -> Self {
        Self { size, scale }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ThemeIconKey {
    name: String,
    size: i32,
    scale: i32,
}

impl ThemeIconKey {
    fn new(name: &str, size: i32, scale: i32) -> Self {
        Self {
            name: name.to_owned(),
            size,
            scale,
        }
    }

    const fn size_key(&self) -> ThemeIconSizeKey {
        ThemeIconSizeKey::new(self.size, self.scale)
    }

    fn matches(&self, name: &str, size: i32, scale: i32) -> bool {
        self.size == size && self.scale == scale && self.name == name
    }
}

/// Cache storage is generic so miss and eviction behavior can be tested without
/// depending on the host icon theme
#[derive(Debug)]
struct ThemeIconCacheMap<T> {
    entries: HashMap<ThemeIconSizeKey, HashMap<String, T>>,
    order: VecDeque<ThemeIconKey>,
    max_entries: usize,
}

impl<T: Clone> ThemeIconCacheMap<T> {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries,
        }
    }

    fn get_or_resolve_with<F>(&mut self, name: &str, size: i32, scale: i32, resolve: F) -> Option<T>
    where
        F: FnOnce(&str, i32, i32) -> Option<T>,
    {
        let size = size.max(1);
        let scale = scale.max(1);
        let size_key = ThemeIconSizeKey::new(size, scale);

        // Borrow the caller's name on a hit so no lookup key is allocated
        if let Some(value) = self
            .entries
            .get(&size_key)
            .and_then(|bucket| bucket.get(name))
        {
            let value = value.clone();
            self.bump(name, size, scale);
            return Some(value);
        }

        // A miss is deliberately not inserted; the outer negative cache owns retry timing
        let value = resolve(name, size, scale)?;
        self.entries
            .entry(size_key)
            .or_default()
            .insert(name.to_owned(), value.clone());
        self.order.push_back(ThemeIconKey::new(name, size, scale));
        self.enforce_limit();
        Some(value)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    #[cfg(test)]
    fn entry_count(&self) -> usize {
        self.entries.values().map(HashMap::len).sum()
    }

    #[cfg(test)]
    fn contains(&self, name: &str, size: i32, scale: i32) -> bool {
        let size_key = ThemeIconSizeKey::new(size.max(1), scale.max(1));
        self.entries
            .get(&size_key)
            .is_some_and(|bucket| bucket.contains_key(name))
    }

    fn bump(&mut self, name: &str, size: i32, scale: i32) {
        if let Some(position) = self
            .order
            .iter()
            .position(|entry| entry.matches(name, size, scale))
        {
            let key = self.order.remove(position).expect("position was checked");
            self.order.push_back(key);
        }
    }

    fn enforce_limit(&mut self) {
        while self.order.len() > self.max_entries {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            let size_key = evicted.size_key();
            let Some(bucket) = self.entries.get_mut(&size_key) else {
                continue;
            };
            bucket.remove(&evicted.name);
            if bucket.is_empty() {
                self.entries.remove(&size_key);
            }
        }
    }
}

/// GTK objects stay on the GTK thread while repeated successful lookups are avoided
#[derive(Debug)]
pub(in crate::ui) struct ThemeIconCache {
    entries: ThemeIconCacheMap<IconPaintable>,
}

impl ThemeIconCache {
    pub(in crate::ui) fn new_for_popups() -> Self {
        Self {
            entries: ThemeIconCacheMap::new(THEME_ICON_CACHE_MAX_ENTRIES),
        }
    }

    pub(in crate::ui) fn get_or_resolve(
        &mut self,
        name: &str,
        size: i32,
        scale: i32,
    ) -> Option<IconPaintable> {
        self.entries
            .get_or_resolve_with(name, size, scale, resolve_icon_paintable_with_scale)
    }

    pub(in crate::ui) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
#[path = "tests/theme_cache.rs"]
mod tests;
