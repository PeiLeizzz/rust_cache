use std::collections::HashMap;
use std::hash::Hash;
use std::time;
use crate::lru::list::LinkedList;
use crate::lru::arena::Index;
use crate::lru::err::CacheError;

pub struct ListItem<K, V> {
    pub key: K,
    pub value: V,
}

pub struct Cache<K, V>
where
    K: Eq + Hash,
{
    list: LinkedList<ListItem<K, V>>,
    map: HashMap<K, Index>,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Copy,
{
    pub fn new_with_cap(cap: usize) -> Self {
        Cache {
            list: LinkedList::new_with_cap(cap),
            map: HashMap::new(),
        }
    }

    pub fn new_with_cap_timeout(cap: usize, timeout: time::Duration) -> Self {
        Cache {
            list: LinkedList::new_with_cap_timeout(cap, timeout),
            map: HashMap::new(),
        }
    }

    pub fn query(&mut self, key: &K) -> Result<&V, CacheError> {
        let index = self.map.get(key).ok_or(CacheError::CacheMiss)?;
        let index = self.list
            .reposition_to_head(index)
            .map_err(CacheError::CacheBroken)?;
        let node = self.list.get(&index).map_err(CacheError::CacheBroken)?;
        // 更新 map 中的 index
        self.map.insert(*key, index);
        Ok(&node.value.value)
    }

    pub fn remove(&mut self, key: &K) -> Result<V, CacheError> {
        let index = self.map.remove(key).ok_or(CacheError::CacheMiss)?;
        let item = self.list.remove(&index).map_err(CacheError::CacheBroken)?;
        Ok(item.value)
    }

    pub fn insert(&mut self, key: K, value: V) -> Result<(), CacheError> {
        // 每次插入之前都进行自动淘汰
        // TODO: 变为无阻塞操作
        self.retire()?;

        if let Some(index) = self.map.get(&key) {
            let index = self.list
                .reposition_to_head(index)
                .map_err(CacheError::CacheBroken)?;
            let item = self
                .list
                .get_mut(&index)
                .map_err(CacheError::CacheBroken)?;
            // 更新 map 中的 index
            self.map.insert(key, index);
            item.value.value = value;
            return Ok(());
        }

        if self.list.is_full() {
            let item = self.list.pop_back().map_err(CacheError::CacheBroken)?;
            self.map.remove(&item.key);
        }

        let index = self
            .list
            .push_front(ListItem { key, value })
            .map_err(CacheError::CacheBroken)?;
        self.map.insert(key, index);

        Ok(())
    }

    fn retire(&mut self) -> Result<(), CacheError> {
        let retired_items = self.list.retire()
            .map_err(CacheError::CacheBroken)?;
        if let Some(items) = retired_items {
            for item in &items {
                self.map.remove(&item.key).ok_or(CacheError::CacheMiss)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use crate::lru::err::ListError;

    use super::*;

    #[test]
    fn lru_cache_consistency() {
        let mut lru_cache = Cache::<i32, i32>::new_with_cap(0);
        assert_eq!(
            lru_cache.insert(0, 0),
            Err(CacheError::CacheBroken(ListError::ListEmpty))
        );

        let mut lru_cache = Cache::<i32, i32>::new_with_cap(2);
        // [1-1]
        assert!(lru_cache.insert(1, 1).is_ok());
        // [2-2 1-1]
        assert!(lru_cache.insert(2, 2).is_ok());
        // [1-1 2-2]
        assert_eq!(lru_cache.query(&1), Ok(&1));
        // [3-3 1-1]
        assert!(lru_cache.insert(3, 3).is_ok());
        assert_eq!(lru_cache.query(&2), Err(CacheError::CacheMiss));
        // [1,-1 3-3]
        assert!(lru_cache.insert(1, -1).is_ok());
        assert_eq!(lru_cache.query(&1), Ok(&-1));
        // [4-4 1,-1]
        assert!(lru_cache.insert(4, 4).is_ok());
        assert_eq!(lru_cache.query(&3), Err(CacheError::CacheMiss));

        let capacity = 5;

        let mut lru_cache = Cache::<i32, i32>::new_with_cap(capacity);
        assert_eq!(lru_cache.query(&0), Err(CacheError::CacheMiss));

        for ele in 0..capacity {
            let x = ele as i32;
            assert!(lru_cache.insert(x, x).is_ok());
        }

        for ele in 0..capacity {
            let x = ele as i32;
            assert_eq!(lru_cache.query(&x), Ok(&x));
        }

        let x = capacity as i32;
        assert!(lru_cache.insert(x, x).is_ok());

        assert_eq!(lru_cache.query(&x), Ok(&x));

        assert_eq!(lru_cache.query(&0), Err(CacheError::CacheMiss));

        let x = capacity as i32 / 2;
        assert_eq!(lru_cache.remove(&x), Ok(x));

        assert_eq!(lru_cache.query(&x), Err(CacheError::CacheMiss));
        assert_eq!(lru_cache.remove(&x), Err(CacheError::CacheMiss));
    }

    #[test]
    fn lru_cache_timeout() {
        let mut lru_cache = Cache::<i32, i32>::new_with_cap_timeout(5, time::Duration::from_millis(1000));

        // [1-1]
        lru_cache.insert(1, 1);
        assert_eq!(lru_cache.query(&1), Ok(&1));

        // [2-2 1-1]
        lru_cache.insert(2, 2);
        // [3-3 2-2 1-1]
        lru_cache.insert(3, 3);

        thread::sleep(time::Duration::from_millis(500));
        assert_eq!(lru_cache.list.len(), 3);

        // [4-4 3-3 2-2 1-1]
        lru_cache.insert(4, 4);
        // [5-5 4-4 3-3 2-2 1-1]
        lru_cache.insert(5, 5);
        assert_eq!(lru_cache.list.len(), 5);

        thread::sleep(time::Duration::from_millis(500));
        assert_eq!(lru_cache.list.len(), 5);

        // [1-1 5-5 4-4 3-3 2-2]
        assert_eq!(lru_cache.query(&1), Ok(&1));
        // [6-6 1-1 5-5 4-4]
        lru_cache.insert(6, 6);
        assert_eq!(lru_cache.list.len(), 4);
        assert_eq!(lru_cache.query(&2), Err(CacheError::CacheMiss));
        assert_eq!(lru_cache.query(&3), Err(CacheError::CacheMiss));
    }
}
