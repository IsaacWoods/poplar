use alloc::vec::Vec;
use core::mem;
use libmessage::{Generation, Index, ProcessId};

enum Entry<P> {
    Free { next_generation: Generation, next_free: Option<u16> },
    Occupied { generation: Generation, process: P },
}

/// `P` can be any type the architecture-specific code wants to associate each process with.
pub struct ProcessMap<P> {
    entries: Vec<Entry<P>>,
    free_list_head: Option<Index>,
}

impl<P> ProcessMap<P> {
    pub fn new(initial_capacity: usize) -> ProcessMap<P> {
        assert!(initial_capacity > 0);

        let mut map =
            ProcessMap { entries: Vec::with_capacity(initial_capacity), free_list_head: None };
        map.reserve(initial_capacity);
        map
    }

    pub fn insert(&mut self, process: P) -> ProcessId {
        match self.free_list_head {
            /*
             * If we have a free entry in the current map, use that.
             */
            Some(index) => self.insert_into(index as usize, process),

            /*
             * If there aren't any free entries in the current process-map, extend it.
             */
            None => {
                /*
                 * Double the number of elements we contain.
                 */
                self.reserve(self.len());
                self.insert_into(self.len(), process)
            }
        }
    }

    fn insert_into(&mut self, index: usize, process: P) -> ProcessId {
        match self.entries[index] {
            Entry::Free { next_generation, next_free } => {
                self.entries[index] = Entry::Occupied { generation: next_generation, process };
                self.free_list_head = next_free;

                ProcessId { index: index as Index, generation: next_generation }
            }

            Entry::Occupied { .. } => panic!("Process map free list is corrupted!"),
        }
    }

    pub fn reserve(&mut self, additional_capacity: usize) {
        let start = self.len();
        let end = start + additional_capacity;
        let current_free_head = self.free_list_head;

        /*
         * Reserve the new elements and initialise them, buidling up the linked list of the next
         * free entries.
         */
        self.entries.reserve_exact(additional_capacity);
        self.entries.extend((start..end).map(|i| {
            /*
             * If this is the last new entry, point it back at the old free list head.
             * Otherwise, point it to the next new entry.
             */
            if i == (end - 1) {
                Entry::Free { next_generation: 0, next_free: current_free_head }
            } else {
                Entry::Free { next_generation: 0, next_free: Some((i + 1) as Index) }
            }
        }));

        /*
         * Set the new free head to the first element we just added. After we run through the new
         * ones, it'll return to whatever the next free element was in the old list.
         */
        self.free_list_head = Some(start as Index);
    }

    pub fn get(&self, entry: ProcessId) -> Option<&P> {
        match self.entries.get(entry.index as usize) {
            /*
             * Only "find" the entry if the generations are the same. If they're not, the
             * expected entry has been removed and replaced by another process!
             */
            Some(Entry::Occupied { generation, ref process })
                if *generation == entry.generation =>
            {
                Some(process)
            }

            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: ProcessId) -> Option<&mut P> {
        match self.entries.get_mut(id.index as usize) {
            /*
             * Only "find" the entry if the generations are the same. If they're not, the
             * expected entry has been removed and replaced by another process!
             */
            Some(Entry::Occupied { generation, ref mut process })
                if *generation == id.generation =>
            {
                Some(process)
            }

            _ => None,
        }
    }

    pub fn remove(&mut self, id: ProcessId) -> Option<P> {
        if (id.index as usize) >= self.entries.len() {
            return None;
        }

        /*
         * Work out the generation that the new free entry should have, if we do remove
         * whatever's there at the moment.
         */
        let next_generation = match self.entries[id.index as usize] {
            Entry::Free { next_generation, .. } => next_generation,
            Entry::Occupied { generation, .. } => generation + 1,
        };

        /*
         * Remove the entry in advance so we own it.
         */
        let entry = mem::replace(
            &mut self.entries[id.index as usize],
            Entry::Free { next_generation, next_free: self.free_list_head },
        );

        match entry {
            Entry::Occupied { generation, process } if generation == id.generation => {
                /*
                 * Found the correct entry. We've already replaced the entry with the correct,
                 * free space, so we just need to mark this as the next free
                 * entry.
                 */
                self.free_list_head = Some(id.index);
                Some(process)
            }

            _ => {
                /*
                 * Either the generation wasn't correct, or this entry isn't even occupied.
                 * Either way, put whatever was there before back.
                 */
                self.entries[id.index as usize] = entry;
                None
            }
        }
    }

    pub fn contains(&self, id: ProcessId) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[test]
fn can_get_values() {
    let mut map = ProcessMap::new(3);
    let thing_0 = map.insert(8);
    let thing_1 = map.insert(17);
    let thing_2 = map.insert(42);

    assert_eq!(map.get(thing_0), Some(&8));
    assert_eq!(map.get(thing_1), Some(&17));
    assert_eq!(map.get(thing_2), Some(&42));
}

#[test]
fn access_old_generation() {
    let mut map = ProcessMap::new(2);
    let thing = map.insert(4);
    let other_thing = map.insert(84);
    map.remove(thing);
    let new_thing = map.insert(13);

    assert_eq!(map.get(thing), None);
    assert_eq!(map.get(other_thing), Some(&84));
    assert_eq!(map.get(new_thing), Some(&13));
}
