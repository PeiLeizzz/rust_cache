use super::{
    arena::{Arena, Index},
    err::ListError,
};

use std::time;

pub struct Node<T> {
    value: T,
    // 淘汰时间
    // 为空说明没有开启自动超时淘汰机制
    expire_time: Option<time::Instant>,
    next: Option<Index>,
    prev: Option<Index>,
}

pub struct LinkedList<T> {
    arena: Arena<Node<T>>,
    head: Option<Index>,
    tail: Option<Index>,
    len: usize,
    // 每个节点 timeout 的时间
    // timeout 为 None 说明没有开启自动超时淘汰机制
    timeout: Option<time::Duration>,
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        LinkedList {
            arena: Arena::new(),
            head: None,
            tail: None,
            len: 0,
            timeout: None,
        }
    }

    pub fn new_with_cap_timeout(cap: usize, timeout: time::Duration) -> Self {
        let mut list = LinkedList::new();
        list.reserve(cap);
        list.timeout = Some(timeout);
        list
    }

    pub fn new_with_cap(cap: usize) -> Self {
        let mut list = LinkedList::new();
        list.reserve(cap);
        list
    }

    pub fn reserve(&mut self, cap: usize) {
        self.arena.reserve(cap)
    }

    // 头插并返回新节点的索引
    pub fn push_front(&mut self, value: T) -> Result<Index, ListError> {
        let mut cur_head_node = Node {
            value,
            expire_time: None,
            next: self.head,
            prev: None,
        };
        // 设置节点淘汰时间
        if let Some(timeout) = self.timeout {
            cur_head_node.expire_time = Some(time::Instant::now() + timeout);
        }

        // 先找块空闲区域插入数据
        let cur_head_index = self
            .arena
            .insert(cur_head_node)
            .map_err(ListError::ListOOM)?;

        if let Some(old_head_index) = self.head {
            // 如果头节点不为空，则将其 prev 指向当前节点
            let old_head_node = self.get_mut(&old_head_index)?;
            old_head_node.prev = Some(cur_head_index);
        } else {
            // 如果头节点为空，说明此时是空链表
            // 头尾节点需要都指派为 node
            self.tail = Some(cur_head_index);
        }

        // 转移头节点至 node
        self.head = Some(cur_head_index);
        self.len += 1;
        Ok(cur_head_index)
    }

    // 尾部删除节点
    pub fn pop_back(&mut self) -> Result<T, ListError> {
        if let Some(tail_index) = self.tail {
            self.remove(&tail_index)
        } else {
            Err(ListError::ListEmpty)
        }

        // let old_tail_index = self.tail.ok_or(ListError::ListEmpty)?;
        // let old_tail_node = self.arena.remove(&old_tail_index).ok_or(ListError::LinkBroken)?;

        // // 尾转移到原尾的前一个节点
        // self.tail = old_tail_node.prev;
        // if let Some(cur_tail_index) = self.tail {
        //     // 将新尾的 next 置为空
        //     let cur_tail_node = self.get_mut(&cur_tail_index)?;
        //     cur_tail_node.next = None;
        // } else {
        //     // 如果尾节点为空，说明此时是空链表
        //     // 头节点也需要指派为空
        //     self.head = None;
        // }

        // self.len -= 1;
        // Ok(old_tail_node.value)
    }

    // 根据节点索引删除该节点
    pub fn remove(&mut self, index: &Index) -> Result<T, ListError> {
        if self.is_empty() {
            return Err(ListError::ListEmpty);
        }

        let node = self.arena.remove(index).ok_or(ListError::LinkBroken)?;

        match (node.prev, node.next) {
            (Some(prev_index), Some(next_index)) => {
                let prev = self.get_mut(&prev_index)?;
                prev.next = Some(next_index);
                let next = self.get_mut(&next_index)?;
                next.prev = Some(prev_index);
            }
            (None, Some(next_index)) => {
                let next = self.get_mut(&next_index)?;
                next.prev = None;
                self.head = Some(next_index);
            }
            (Some(prev_index), None) => {
                let prev = self.get_mut(&prev_index)?;
                prev.next = None;
                self.tail = Some(prev_index);
            }
            (None, None) => {
                // node 是唯一节点，链表现在为空
                self.head = None;
                self.tail = None;
            }
        }

        self.len -= 1;
        Ok(node.value)
    }

    pub fn retire(&mut self) -> Result<Option<Vec<T>>, ListError> {
        if let Some(_) = self.timeout {
            let now = time::Instant::now();
            let mut values = vec![];
            while !self.is_empty() {
                let tail_index = self.tail.unwrap();
                let expire_time = self.get(&tail_index)?.expire_time.unwrap();
                if now >= expire_time {
                    values.push(self.remove(&tail_index)?);
                } else {
                    break;
                }
            }
            if values.len() > 0 {
                return Ok(Some(values));
            }
            // 如果没有一个被淘汰，返回 None，而不是 vec![]
        }
        Ok(None)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn is_full(&self) -> bool {
        self.len == self.arena.cap()
    }

    pub fn get(&self, index: &Index) -> Result<&Node<T>, ListError> {
        self.arena.get(index).ok_or(ListError::LinkBroken)
    }

    pub fn get_mut(&mut self, index: &Index) -> Result<&mut Node<T>, ListError> {
        self.arena.get_mut(index).ok_or(ListError::LinkBroken)
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    impl<T> LinkedList<T> {
        fn iter(&self) -> Iter<T> {
            Iter {
                list: self,
                current: self.head,
            }
        }
    }

    struct Iter<'a, T: 'a> {
        list: &'a LinkedList<T>,
        current: Option<Index>,
    }
    impl<'a, T: 'a> Iterator for Iter<'a, T> {
        type Item = &'a T;

        fn next(&mut self) -> Option<Self::Item> {
            if let Some(index) = self.current {
                if let Ok(node) = self.list.get(&index) {
                    self.current = node.next;
                    return Some(&node.value);
                }
            }

            None
        }
    }

    #[test]
    fn list_new() {
        let list = LinkedList::<i32>::new();
        assert!(list.is_empty());
        assert!(list.is_full());
    }

    #[test]
    fn list_push_front() {
        let capacity = 10;
        let mut list = LinkedList::<i32>::new_with_cap(capacity);
        for ele in 0..capacity {
            assert!(list.push_front(ele as i32).is_ok());
        }

        let mut i = capacity as i32 - 1;
        for ele in list.iter() {
            assert_eq!(ele, &i);
            i -= 1;
        }
    }

    #[test]
    fn list_pop_back() {
        let capacity = 10;
        let mut list = LinkedList::<i32>::new_with_cap(capacity);

        assert_eq!(list.pop_back(), Err(ListError::ListEmpty));

        for ele in 0..capacity {
            assert!(list.push_front(ele as i32).is_ok());
        }

        for ele in 0..capacity {
            assert_eq!(list.pop_back().unwrap(), ele as i32);
        }

        assert!(list.is_empty());
        assert_eq!(list.pop_back(), Err(ListError::ListEmpty));
    }

    #[test]
    fn list_remove() {
        let mut list = LinkedList::<i32>::new_with_cap(5);
        assert!(list.is_empty());

        let link_0 = list.push_front(0).unwrap();
        let _link_1 = list.push_front(1).unwrap();
        let link_2 = list.push_front(2).unwrap();
        let _link_3 = list.push_front(3).unwrap();
        let link_4 = list.push_front(4).unwrap();
        assert!(list.is_full());
        assert!(list.iter().eq([4, 3, 2, 1, 0].iter()));

        assert!(list.remove(&link_0).is_ok());
        assert_eq!(list.len(), 4);
        assert!(list.iter().eq([4, 3, 2, 1].iter()));

        assert!(list.remove(&link_4).is_ok());
        assert_eq!(list.len(), 3);
        assert!(list.iter().eq([3, 2, 1].iter()));

        assert!(list.remove(&link_2).is_ok());
        assert_eq!(list.len(), 2);
        assert!(list.iter().eq([3, 1].iter()));
    }

    #[test]
    fn list_retire() {
        let capacity = 10;
        let mut list =
            LinkedList::<i32>::new_with_cap_timeout(capacity, time::Duration::from_millis(1000));
        for ele in 0..5 {
            assert!(list.push_front(ele as i32).is_ok());
        }

        thread::sleep(time::Duration::from_millis(500));
        assert_eq!(list.len(), 5);

        for ele in 0..5 {
            assert!(list.push_front(5 + ele as i32).is_ok());
        }
        assert_eq!(list.len(), 10);

        assert!(list.retire().is_ok());
        assert!(list.retire().unwrap().is_none());
        assert_eq!(list.len(), 10);

        thread::sleep(time::Duration::from_millis(500));
        assert_eq!(list.retire().unwrap().unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(list.len(), 5);
        assert_eq!(list.pop_back().unwrap(), 5);

        thread::sleep(time::Duration::from_millis(500));
        assert_eq!(list.retire().unwrap().unwrap(), vec![6, 7, 8, 9]);
        assert_eq!(list.len(), 0);

        assert!(list.retire().unwrap().is_none());
    }
}
