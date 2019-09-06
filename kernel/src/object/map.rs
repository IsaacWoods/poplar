use super::{KernelObject, WrappedKernelObject};
use crate::arch::Architecture;
use alloc::{sync::Arc, vec::Vec};
use core::mem;
use libpebble::object::{Generation, Index, KernelObjectId};

pub const INITIAL_OBJECT_CAPACITY: usize = 32;

enum Entry<A: Architecture> {
    Free { next_generation: Generation, next_free: Option<u16> },
    Occupied { generation: Generation, object: Arc<KernelObject<A>> },
}

/// Stores all the `KernelObject`s against their generational `KernelObjectId`s.
pub struct ObjectMap<A: Architecture> {
    entries: Vec<Entry<A>>,
    free_list_head: Option<Index>,
}

impl<A> ObjectMap<A>
where
    A: Architecture,
{
    pub fn new(initial_capacity: usize) -> ObjectMap<A> {
        if initial_capacity == 0 {
            panic!("Can't create object map with size of zero!");
        }

        let mut map = ObjectMap { entries: Vec::with_capacity(initial_capacity), free_list_head: None };
        map.reserve(initial_capacity);
        map
    }

    /// Insert a new object into the map, assigning it a `KernelObjectId`. The map will not assign
    /// an ID with an index of `0`, because it is reserved as a null ID.
    pub fn insert(&mut self, object: Arc<KernelObject<A>>) -> KernelObjectId {
        match self.free_list_head {
            /*
             * If we have a free entry in the current map, use that.
             */
            Some(index) => self.insert_into(index as usize, object),

            /*
             * If there aren't any free entries in the current map, extend it.
             */
            None => {
                /*
                 * Double the number of elements we contain.
                 */
                let current_len = self.len();
                self.reserve(current_len);
                self.insert_into(current_len, object)
            }
        }
    }

    fn insert_into(&mut self, index: usize, object: Arc<KernelObject<A>>) -> KernelObjectId {
        match self.entries[index] {
            Entry::Free { next_generation, next_free } => {
                self.entries[index] = Entry::Occupied { generation: next_generation, object };
                self.free_list_head = next_free;

                KernelObjectId { index: (index + 1) as Index, generation: next_generation }
            }

            Entry::Occupied { .. } => panic!("Object-map free list is corrupted!"),
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

    pub fn get(&self, id: KernelObjectId) -> Option<WrappedKernelObject<A>> {
        match self.entries.get((id.index - 1) as usize) {
            /*
             * Only "find" the entry if the generations are the same. If they're not, the
             * expected entry has been removed and replaced by something else!
             */
            Some(Entry::Occupied { generation, object }) if *generation == id.generation => {
                Some(WrappedKernelObject { id, object: object.clone() })
            }

            _ => None,
        }
    }

    pub fn remove(&mut self, id: KernelObjectId) -> Option<Arc<KernelObject<A>>> {
        if (id.index as usize) >= self.len() {
            return None;
        }

        /*
         * Work out the generation that the new free entry should have, if we do remove
         * whatever's there at the moment.
         */
        let next_generation = match self.entries[(id.index - 1) as usize] {
            Entry::Free { next_generation, .. } => next_generation,
            Entry::Occupied { generation, .. } => generation + 1,
        };

        /*
         * Remove the entry in advance so we own it.
         */
        let entry = mem::replace(
            &mut self.entries[(id.index - 1) as usize],
            Entry::Free { next_generation, next_free: self.free_list_head },
        );

        match entry {
            Entry::Occupied { generation, object } if generation == id.generation => {
                /*
                 * Found the correct entry. We've already replaced the entry with the correct,
                 * free space, so we just need to mark this as the next free
                 * entry.
                 */
                self.free_list_head = Some(id.index - 1);
                Some(object)
            }

            _ => {
                /*
                 * Either the generation wasn't correct, or this entry isn't even occupied.
                 * Either way, put whatever was there before back.
                 */
                self.entries[(id.index - 1) as usize] = entry;
                None
            }
        }
    }

    pub fn contains(&self, id: KernelObjectId) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod test {
    use super::ObjectMap;
    use crate::{
        arch::test::FakeArch,
        object::{KernelObject, WrappedKernelObject},
    };
    use alloc::sync::Arc;
    use core::ops::Deref;

    /*
     * These macros are needed because not all the variants of `KernelObject` can implement `Eq`,
     * so we can't just `assert_eq` the entries.
     */
    macro assert_none($entry: expr) {
        match $entry {
            None => (),
            entry => panic!("Incorrect entry during ObjectMap testing: {:?}", entry),
        }
    }

    fn assert_some(entry: Option<WrappedKernelObject<FakeArch>>, expected_value: usize) {
        match entry {
            Some(wrapped_object) => match wrapped_object.object.deref() {
                KernelObject::Test(value) => assert_eq!(*value, expected_value),
                entry => panic!("Incorrect entry during ObjectMap testing: {:?}", entry),
            },
            entry => panic!("Incorrect entry during ObjectMap testing: {:?}", entry),
        }
    }

    #[test]
    #[should_panic]
    fn no_empty_maps() {
        let _: ObjectMap<FakeArch> = ObjectMap::new(0);
    }

    #[test]
    fn can_get_values() {
        let mut map = ObjectMap::<FakeArch>::new(3);
        let thing_0 = KernelObject::Test(8).add_to_map(&mut map);
        let thing_1 = KernelObject::Test(17).add_to_map(&mut map);
        let thing_2 = KernelObject::Test(42).add_to_map(&mut map);

        assert_some(map.get(thing_0.id), 8);
        assert_some(map.get(thing_1.id), 17);
        assert_some(map.get(thing_2.id), 42);
    }

    #[test]
    fn access_old_generation() {
        let mut map = ObjectMap::<FakeArch>::new(2);
        let thing = KernelObject::Test(4).add_to_map(&mut map);
        let other_thing = KernelObject::Test(84).add_to_map(&mut map);
        map.remove(thing.id);
        let new_thing = KernelObject::Test(13).add_to_map(&mut map);

        assert_none!(map.get(thing.id));
        assert_some(map.get(other_thing.id), 84);
        assert_some(map.get(new_thing.id), 13);
    }

    #[test]
    fn access_old_across_allocation() {
        let mut map = ObjectMap::<FakeArch>::new(2);
        let thing_0 = KernelObject::Test(8).add_to_map(&mut map);
        let thing_1 = KernelObject::Test(17).add_to_map(&mut map);
        // This next insert causes the backing `Vec` to expand
        let thing_2 = KernelObject::Test(42).add_to_map(&mut map);

        map.remove(thing_1.id);

        assert_some(map.get(thing_0.id), 8);
        assert_none!(map.get(thing_1.id));
        assert_some(map.get(thing_2.id), 42);
    }
}
