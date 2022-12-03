use crate::lru::err::ArenaOOM;

// 内存单位的索引信息
// 用于在内存区域中查询数据
#[derive(Clone, Copy)]
pub struct Index {
    // 该内存单位在整个内存区域的下标
    // 这里的下标对应 Vec 中的下标
    pub idx: usize,

    // 这个代数是在插入时返回的
    // 可能在查询时数据已被覆盖
    pub generation: u64,
}

// 最小的一块内存单位，存放对应的值（V）
// 有两种状态：空闲 / 被占用
pub enum Entry<T> {
    Free {
        // 下一块空闲区域的下标
        next_free: Option<usize>,
    },
    Occupied {
        // 当前内存中的值
        value: T,
        // 当前内存中数据的代数
        // 每次插入时都会更新代数
        // 因此每个数据都有唯一的代数
        // 查询时将 Index 的代数和 Entry 中实际的代数进行比较
        // 如果不匹配说明 Index 过期
        generation: u64,
    },
}

// 整个连续的内存区域
pub struct Arena<T> {
    // 该连续的内存区域中的所有内存单位
    // 通过 Vec 存储，因为 Vec 本身就是一段连续的内存空间
    items: Vec<Entry<T>>,
    // 该连续的内存区域的容量，表明可以容纳 cap 个内存单位
    cap: usize,

    // 当前整个内存区域下一次插入数据时的代数
    generation: u64,

    // 首个空闲区域的下标（逻辑上）
    // 可能在 Vec 上还有值，但会被覆盖
    free_list_head: Option<usize>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            cap: 0,
            generation: 0,
            free_list_head: None,
        }
    }

    pub fn new_with_cap(cap: usize) -> Self {
        let mut arena = Self::new();
        // 初始化申请 cap 个内存区域
        arena.reserve(cap);
        arena
    }

    // 在内存区域尾部扩充 usize 个连续的内存单位
    // 如果内存区域尾部本身就可以容纳额外的 usize 个连续的内存单位
    // 那么就不会进行扩充
    pub fn reserve(&mut self, cap: usize) {
        // 使用 Vec 的扩充函数
        // 如果 Vec.capacity() >= Vec.size() + cap
        // 那么维持 Vec.capacity 不变
        // 否则扩充至 Vec.size() + cap
        self.items.reserve_exact(cap);

        // 新区域的头
        let start = self.items.len();
        // 新区域的尾（开区间）
        let end = start + cap;

        // 记录原先的首个空闲区域
        // 用于让新空闲区域尾部的 next_free 指向它
        let old_free = self.free_list_head;
        self.items.extend((start..end).map(|i| {
            if i == end - 1 {
                Entry::Free {
                    next_free: old_free,
                }
            } else {
                Entry::Free {
                    next_free: Some(i + 1),
                }
            }
        }));

        // 将之前的首个空闲区域指向新区域的头
        // 因为扩容后推测应该是要 insert
        // 所以可以提前指向更大的连续空闲区域
        self.free_list_head = Some(start);
        // 更新内存容量
        self.cap += cap;
    }

    pub fn insert(&mut self, value: T) -> Result<Index, ArenaOOM> {
        // 如果 Arena 还没有初始化，返回错误
        if self.free_list_head.is_none() {
            return Err(ArenaOOM {});
        }

        // 检查首个空闲区域是否空闲
        // 如果不空闲说明内存已满，返回错误
        // 否则先记录该区域的下标（用于占用）并先将首个空闲区域指向下一个空闲区域
        let old_free = self.free_list_head;
        if let Entry::Free { next_free } = self.items[old_free.unwrap()] {
            self.free_list_head = next_free;
        } else {
            return Err(ArenaOOM {});
        }

        // 占用之前记录的空闲区域
        let entry = Entry::Occupied {
            value: value,
            generation: self.generation,
        };
        self.items[old_free.unwrap()] = entry;
        self.generation += 1;

        // 返回该被占用区域的索引信息
        Ok(Index {
            idx: old_free.unwrap(),
            generation: self.generation - 1,
        })
    }

    pub fn remove(&mut self, index: &Index) -> Option<T> {
        if let Some(Entry::Occupied {
            value: _,
            generation,
        }) = self.items.get(index.idx)
        {
            // 代数不匹配
            // 说明 Index 过期
            // 返回 None
            if &index.generation != generation {
                return None;
            }

            // 释放当前被占用的存储区域
            // 并通过头插法更新首个空闲区域下标
            let entry = Entry::<T>::Free {
                next_free: self.free_list_head,
            };
            let old_entry = core::mem::replace(&mut self.items[index.idx], entry);
            self.free_list_head = Some(index.idx);

            // 将被释放的数据所有权返回
            if let Entry::Occupied {
                value,
                generation: _,
            } = old_entry
            {
                return Some(value);
            }
        }

        None
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn get(&self, index: &Index) -> Option<&T> {
        if let Some(Entry::Occupied { value, generation }) = self.items.get(index.idx) {
            if &index.generation == generation {
                return Some(value);
            }
        }

        None
    }

    pub fn get_mut(&mut self, index: &Index) -> Option<&mut T> {
        if let Some(Entry::Occupied { value, generation }) = self.items.get_mut(index.idx) {
            if &index.generation == generation {
                return Some(value);
            }
        }

        None
    }
}
