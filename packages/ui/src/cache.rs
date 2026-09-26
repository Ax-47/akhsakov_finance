//! Stale-while-revalidate cache for page data: going back to a page (or a
//! stock, index, filter …) shows the last result at once while fresh data
//! loads in the background. Kept in memory for this session.

use dioxus::prelude::*;
use std::{any::Any, cell::RefCell, collections::HashMap, future::Future, rc::Rc};

/// Entries beyond this are dropped, oldest first.
const MAX_ENTRIES: usize = 300;

thread_local! {
    /// key → value, plus insertion order for eviction.
    static CACHE: RefCell<(HashMap<String, Rc<dyn Any>>, Vec<String>)> = RefCell::default();
}

/// The cached value for `key`, if any (and of type `T`).
pub fn get<T: Clone + 'static>(key: &str) -> Option<T> {
    CACHE.with(|c| c.borrow().0.get(key)?.downcast_ref::<T>().cloned())
}

pub fn put<T: 'static>(key: String, value: T) {
    CACHE.with(|c| {
        let (map, order) = &mut *c.borrow_mut();
        if map.insert(key.clone(), Rc::new(value)).is_none() {
            order.push(key);
            if order.len() > MAX_ENTRIES {
                let oldest = order.remove(0);
                map.remove(&oldest);
            }
        }
    });
}

/// Like `use_resource`, but starts from the cached value for `key()` and
/// caches each result. Both closures may read signals: when they change,
/// the cached value for the new key shows while it refetches. `None` only
/// when nothing is cached and the first load hasn't finished.
pub fn use_cached<T, F, Fut>(key: impl Fn() -> String + 'static, fetch: F) -> Memo<Option<T>>
where
    T: Clone + PartialEq + 'static,
    F: FnMut() -> Fut + 'static,
    Fut: Future<Output = T> + 'static,
{
    let fetch = Rc::new(RefCell::new(fetch));
    let key = Rc::new(key);
    let for_memo = key.clone();
    let latest = use_resource(move || {
        let k = key();
        let pending = (fetch.borrow_mut())();
        async move {
            let value = pending.await;
            put(k.clone(), value.clone());
            (k, value)
        }
    });
    use_memo(move || {
        let k = for_memo();
        match &*latest.read() {
            Some((done, value)) if *done == k => Some(value.clone()),
            _ => get::<T>(&k),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_by_key_and_type() {
        put("a".into(), 5_u32);
        assert_eq!(get::<u32>("a"), Some(5));
        assert_eq!(get::<String>("a"), None, "wrong type");
        assert_eq!(get::<u32>("b"), None);
        for i in 0..MAX_ENTRIES + 5 {
            put(format!("k{i}"), i);
        }
        assert_eq!(get::<u32>("a"), None, "oldest evicted");
        assert_eq!(get::<usize>(&format!("k{}", MAX_ENTRIES + 4)), Some(MAX_ENTRIES + 4));
    }
}
