use scroll::Pread;

#[derive(Debug)]
pub struct NoteEntry<'a> {
    pub name: &'a [u8],
    pub entry_type: u32,
    pub desc: &'a [u8],
}

pub struct NoteIter<'a> {
    data: &'a [u8],
}

impl<'a> NoteIter<'a> {
    pub(crate) fn new(data: &'a [u8]) -> NoteIter<'a> {
        NoteIter { data }
    }
}

impl<'a> Iterator for NoteIter<'a> {
    type Item = NoteEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let name_size = self.data.pread::<u32>(0).ok()? as usize;
        let desc_size = self.data.pread::<u32>(4).ok()? as usize;
        let entry_type = self.data.pread::<u32>(8).ok()?;

        // Calculate the offsets to the description and next entry
        let desc_offset = align_up(12 + name_size, 4);
        let next_entry_offset = align_up(desc_offset + desc_size, 4);

        // Make sure the next entry is complete - otherwise we'll panic. We treat incomplete
        // entries as missing by returning `None`.
        if self.data.len() < next_entry_offset {
            return None;
        }

        let name = &self.data[12..(12 + name_size)];
        let desc = &self.data[desc_offset..(desc_offset + desc_size)];

        self.data = &self.data[next_entry_offset..];
        Some(NoteEntry { name, entry_type, desc })
    }
}

fn align_up(offset: usize, alignment: usize) -> usize {
    if offset % alignment == 0 {
        offset
    } else {
        offset + alignment - (offset % alignment)
    }
}
