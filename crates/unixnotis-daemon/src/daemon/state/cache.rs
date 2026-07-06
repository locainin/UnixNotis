use std::sync::Mutex as StdMutex;

pub(in crate::daemon::state) fn should_emit_cached<T: Clone + PartialEq>(
    cache: &StdMutex<Option<T>>,
    value: &T,
) -> bool {
    // Sync mutex is enough here because this cache is tiny and never held across await points
    let mut last_value = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if last_value
        .as_ref()
        .is_some_and(|previous| previous == value)
    {
        // Identical state would only burn CPU in zbus and the listeners
        return false;
    }
    // Clone once on change so later comparisons stay allocation-free for equal values
    *last_value = Some(value.clone());
    true
}
