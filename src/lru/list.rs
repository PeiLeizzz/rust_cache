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

    // 从内存中创建一定容量的链表，并带有超时淘汰机制
    pub fn new_with_cap_timeout(cap: usize, timeout: time::Duration) -> Self {
        let mut list = LinkedList::new();
        list.reserve(cap);
        list.timeout = Some(timeout);
        list
    }

    // 从内存中创建一定容量的链表，并不带有超时淘汰机制
    pub fn new_with_cap(cap: usize) -> Self {
        let mut list = LinkedList::new();
        list.reserve(cap);
        list
    }
    
    // 从内存中获取cap容量的内存
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

    // 从尾部插入节点
    pub fn push_back(&mut self, value: T) -> Result<Index, ListError> {
        let node = Node {
            value,
            prev: self.tail,
            next: None,
            expire_time: None,
        };

        let index = self.arena.insert(node).map_err(ListError::ListOOM)?;
        let link = index;
        if let Some(tail) = self.tail {
            let tail_node = self.get_mut(&tail)?;
            tail_node.next = Some(link);
        } else {
            self.head = Some(link)
        }

        self.tail = Some(link);

        self.len += 1;
        Ok(link)
    }

    // 头部删除节点
    pub fn pop_front(&mut self) -> Result<T, ListError> {
        if let Some(head_index) = self.head {
            self.remove(&head_index)
        } else {
            Err(ListError::ListEmpty)
        }
    }

    // 尾部删除节点
    pub fn pop_back(&mut self) -> Result<T, ListError> {
        if let Some(tail_index) = self.tail {
            self.remove(&tail_index)
        } else {
            Err(ListError::ListEmpty)
        }
    }

    // 将 index 节点移动到头部
    // 返回的是该节点的最新 index，原来的 index 会失效！
    pub fn reposition_to_head(&mut self, index: &Index) -> Result<Index, ListError> {
        let value = self.remove(&index)?;
        self.push_front(value)
    }

    // 返回头节点的值
    pub fn peek_front(&self) -> Result<&T, ListError> {
        let head_index = self.head.ok_or(ListError::ListEmpty)?;
        return self.get(&head_index).map(|x| &x.value);
    }

    // 返回尾节点的值
    pub fn peek_back(&self) -> Result<&T, ListError> {
        let tail_index = self.tail.ok_or(ListError::ListEmpty)?;
        return self.get(&tail_index).map(|x| &x.value);
    }

    // 根据节点索引删除该节点，返回该节点值的所有权
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

    // 从链表尾开始淘汰过期节点，并返回其值的所有权的集合
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
    fn list_reposition_to_head() {
        let capacity = 5;

        let mut list = LinkedList::<i32>::new_with_cap(capacity);
        assert!(list.is_empty());

        for ele in 0..capacity {
            list.push_back(ele as i32).unwrap();
        }
        assert!(list.iter().eq([0, 1, 2, 3, 4].iter()));

        // [0, 1, 2, 3, 4] --> [2, 3, 4, 0, 1]
        for _ in capacity/2..capacity {
            list.reposition_to_head(&list.tail.unwrap()).unwrap();
        }
        assert!(list.iter().eq([2, 3, 4, 0, 1].iter()));

        let mut i = 0;
        let mut rh = 0 as i32;
        let mut lh = capacity as i32 / 2;
        for ele in list.iter() {
            if i <= (capacity / 2) {
                assert_eq!(ele, &lh);
                lh += 1;
            } else {
                assert_eq!(ele, &rh);
                rh += 1;
            }
            i += 1
        }

        let mut list = LinkedList::<i32>::new_with_cap(2);
        // [0]
        let index_0 = list.push_back(0).unwrap();
        let index_0_another = list.reposition_to_head(&index_0).unwrap();
        assert_ne!(Some(index_0), Some(index_0_another));
        assert_eq!(Some(index_0_another), list.head);
        assert_eq!(Some(index_0_another), list.tail);

        // [0, 1]
        let index_1 = list.push_back(1).unwrap();
        // [1, 0]
        let index_1_another = list.reposition_to_head(&index_1).unwrap();

        assert_eq!(list.head, Some(index_1_another));
        assert_eq!(list.tail, Some(index_0_another));

        list.reserve(1);
        // [1, 0, 2]
        list.push_back(2).unwrap();
        // [0, 1, 2]
        list.reposition_to_head(&index_0_another).unwrap();

        assert!(list.iter().eq([0, 1, 2].iter()));
    }

    #[test]
    fn list_pop_front() {
        let capacity = 10;
        let mut list = LinkedList::<i32>::new_with_cap(capacity);

        assert_eq!(list.pop_front(), Err(ListError::ListEmpty));

        for ele in 0..capacity {
            assert!(list.push_back(ele as i32).is_ok());
        }

        for ele in 0..capacity {
            assert_eq!(list.pop_front().unwrap(), ele as i32);
        }

        assert!(list.is_empty());
        assert_eq!(list.pop_front(), Err(ListError::ListEmpty));
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


    impl<T> Node<T> {
        pub fn value(&self) -> &T {
            &self.value
        }
    }
    #[test]
    fn list_retire_and_reposition_to_head() {
        let capacity = 5;
        let mut list =
            LinkedList::<i32>::new_with_cap_timeout(capacity, time::Duration::from_millis(1000));

        let mut live_index = list.head;
        for ele in 0..capacity {
            let index = list.push_front(ele as i32).unwrap();
            // 记录中间节点
            if ele == capacity / 2 {
                live_index = Some(index);
            }
        }
        assert_eq!(list.len(), capacity);

        // 此时应该节点全都过期了
        thread::sleep(time::Duration::from_millis(1000));

        // 更新中心节点
        let live_index = list.reposition_to_head(&live_index.unwrap()).unwrap();
        assert_eq!(*list.get(&live_index).unwrap().value(), capacity as i32 / 2);
        assert_eq!(list.head.unwrap(), live_index);

        // 淘汰其余 capacity - 1 个节点
        assert!(list.retire().is_ok());
        assert_eq!(list.len(), 1);
        assert!(list.iter().eq([capacity as i32 / 2].iter()));
    }
}
