mod lru;

use crate::lru::cache::Cache;
use std::time;
use std::thread;

fn main() {
    let mut lru_cache = Cache::<i32, i32>::new_with_cap_timeout(5, time::Duration::from_millis(1000));

    // [1-1]
    lru_cache.insert(1, 1);
    let v = lru_cache.query(&1).unwrap();
    println!("get 1: {v}");
    // [2-2 1-1]
    lru_cache.insert(2, 2);
    let v = lru_cache.query(&2).unwrap();
    println!("get 2: {v}");
    // [3-3 2-2 1-1]
    lru_cache.insert(3, 3);
    let v = lru_cache.query(&3).unwrap();
    println!("get 3: {v}");
    thread::sleep(time::Duration::from_millis(500));

    // [4-4 3-3 2-2 1-1]
    lru_cache.insert(4, 4);
    let v = lru_cache.query(&4).unwrap();
    println!("get 4: {v}");
    // [5-5 4-4 3-3 2-2 1-1]
    lru_cache.insert(5, 5);
    let v = lru_cache.query(&5).unwrap();
    println!("get 5: {v}");

    let len = lru_cache.len();
    println!("current length: {len}");

    let v = lru_cache.query(&6).unwrap_err();
    println!("get 6: {v}");

    // [4-4 3-3 2-2 1-1]
    let v = lru_cache.remove(&5).unwrap();
    println!("remove 5: {v}");
    let v = lru_cache.query(&5).unwrap_err();
    println!("get 5: {v}");

    // [4-4]
    thread::sleep(time::Duration::from_millis(500));
    let len = lru_cache.len();
    println!("current length: {len}");

    // [1-10, 4-4]
    lru_cache.insert(1, 10);
    let len = lru_cache.len();
    println!("current length: {len}");

    let v = lru_cache.query(&4).unwrap();
    println!("get 4: {v}");
    let v = lru_cache.query(&3).unwrap_err();
    println!("get 3: {v}");
    let v = lru_cache.query(&2).unwrap_err();
    println!("get 2: {v}");
    let v = lru_cache.query(&1).unwrap();
    println!("get 1: {v}");
}
