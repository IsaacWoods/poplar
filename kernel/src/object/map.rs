use super::KernelObject;
use crate::arch::Architecture;
use alloc::vec::Vec;
use core::mem;
use libpebble::object::{Generation, Index, KernelObjectId};

pub const INITIAL_OBJECT_CAPACITY: usize = 32;

enum Entry<A: Architecture> {
    Free { next_generation: Generation, next_free: Option<u16> },
    Occupied { generation: Generation, object: KernelObject<A> },
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

        let mut map =
            ObjectMap { entries: Vec::with_capacity(initial_capacity), free_list_head: None };
        map.reserve(initial_capacity);
        map
    }

    /// Insert a new object into the map, assigning it a `KernelObjectId`. The map will not assign
    /// an ID with an index of `0`, because it is reserved as a null ID.
    pub fn insert(&mut self, object: KernelObject<A>) -> KernelObjectId {
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

    fn insert_into(&mut self, index: usize, object: KernelObject<A>) -> KernelObjectId {
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

    pub fn get(&self, id: KernelObjectId) -> Option<&KernelObject<A>> {
        match self.entries.get((id.index - 1) as usize) {
            /*
             * Only "find" the entry if the generations are the same. If they're not, the
             * expected entry has been removed and replaced by something else!
             */
            Some(Entry::Occupied { generation, ref object }) if *generation == id.generation => {
                Some(object)
            }

            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: KernelObjectId) -> Option<&mut KernelObject<A>> {
        match self.entries.get_mut((id.index - 1) as usize) {
            /*
             * Only "find" the entry if the generations are the same. If they're not, the
             * expected entry has been removed and replaced by something else!
             */
            Some(Entry::Occupied { generation, ref mut object })
                if *generation == id.generation =>
            {
                Some(object)
            }

            _ => None,
        }
    }

    pub fn remove(&mut self, id: KernelObjectId) -> Option<KernelObject<A>> {
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
    use crate::{arch::test::FakeArch, object::KernelObject};

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

    macro assert_some($entry: expr, $expected_value: expr) {
        match $entry {
            Some(&KernelObject::Test(value)) => assert_eq!(value, $expected_value),
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
        let thing_0 = map.insert(KernelObject::Test(8));
        let thing_1 = map.insert(KernelObject::Test(17));
        let thing_2 = map.insert(KernelObject::Test(42));

        assert_some!(map.get(thing_0), 8);
        assert_some!(map.get(thing_1), 17);
        assert_some!(map.get(thing_2), 42);
    }

    #[test]
    fn access_old_generation() {
        let mut map = ObjectMap::<FakeArch>::new(2);
        let thing = map.insert(KernelObject::Test(4));
        let other_thing = map.insert(KernelObject::Test(84));
        map.remove(thing);
        let new_thing = map.insert(KernelObject::Test(13));

        assert_none!(map.get(thing));
        assert_some!(map.get(other_thing), 84);
        assert_some!(map.get(new_thing), 13);
    }

    #[test]
    fn access_old_across_allocation() {
        let mut map = ObjectMap::<FakeArch>::new(2);
        let thing_0 = map.insert(KernelObject::Test(8));
        let thing_1 = map.insert(KernelObject::Test(17));
        // This next insert causes the backing `Vec` to expand
        let thing_2 = map.insert(KernelObject::Test(42));

        map.remove(thing_1);

        assert_some!(map.get(thing_0), 8);
        assert_none!(map.get(thing_1));
        assert_some!(map.get(thing_2), 42);
    }
}
