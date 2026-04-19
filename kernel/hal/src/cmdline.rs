//! This module implements a no-alloc parser for kernel-style cmdlines, used by the loader and
//! kernel for runtime configuration options.
//!
//! The syntax should be relatively familiar. An example cmdline would be:
//! `earlylog=e9 foo bar.baz=4 acpi.os="Microsoft Windows NT"`

use core::str::pattern::Pattern;

pub struct Cmdline<'s> {
    source: &'s str,
}

impl<'s> Cmdline<'s> {
    pub fn new(source: &'s str) -> Cmdline<'s> {
        Cmdline { source }
    }

    pub fn iter(&self) -> CmdlineIter<'s> {
        CmdlineIter {
            source: self.source,
            cursor: 0,
        }
    }

    /// Query the cmdline for a given `key`:
    ///    - `None` means the key is not present
    ///    - `Some(None)` means the key is present, but has no associated value
    ///    - `Some(Some(value))` means the key is present and has the associated value supplied
    pub fn get(&self, key: &str) -> Option<Option<&'s str>> {
        self.iter()
            .find_map(|(k, v)| if k == key { Some(v) } else { None })
    }
}

pub struct CmdlineIter<'s> {
    source: &'s str,
    cursor: usize,
}

impl<'s> Iterator for CmdlineIter<'s> {
    type Item = (&'s str, Option<&'s str>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.source.len() {
            return None;
        }

        /*
         * Each entry is separated by a space. However, spaces inside quotes should be treated as
         * part of a value, and so we skip over the quoted portion to find the space after that.
         * Nested quotes are not supported.
         */
        let next_space = {
            let next_space = self.find_next(' ');
            if let Some(next_quote) = self.find_next('\"')
                && next_quote < next_space.unwrap_or(self.source.len())
            {
                /*
                 * If there is no closing quote, take the rest of the cmdline as the value.
                 */
                if let Some(closing_quote) = self.find_from('\"', next_quote + 1) {
                    self.find_from(' ', closing_quote)
                        .unwrap_or(self.source.len())
                } else {
                    self.source.len()
                }
            } else {
                next_space.unwrap_or(self.source.len())
            }
        };

        let entry = &self.source[self.cursor..next_space];
        self.cursor = next_space + 1;

        if let Some(equals) = entry.find('=') {
            let key = &entry[..equals];
            let value = &entry[(equals + 1)..].trim_prefix('\"').trim_suffix('\"');
            Some((key, Some(value)))
        } else {
            Some((entry, None))
        }
    }
}

impl CmdlineIter<'_> {
    /// Find the offset of the next occurance of `pat`, starting at `start` in the cmdline.
    #[inline(always)]
    fn find_from(&mut self, pat: impl Pattern, start: usize) -> Option<usize> {
        self.source[start..].find(pat).map(|idx| start + idx)
    }

    #[inline(always)]
    fn find_next(&mut self, pat: impl Pattern) -> Option<usize> {
        self.find_from(pat, self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmdline_iter() {
        let source = "earlylog=e9 foo bar.baz=4";
        let mut cmdline = CmdlineIter { source, cursor: 0 };

        assert_eq!(cmdline.next(), Some(("earlylog", Some("e9"))));
        assert_eq!(cmdline.next(), Some(("foo", None)));
        assert_eq!(cmdline.next(), Some(("bar.baz", Some("4"))));
        assert_eq!(cmdline.next(), None);

        let source = "foo bar=\"some value\" baz=9";
        let mut cmdline = CmdlineIter { source, cursor: 0 };
        assert_eq!(cmdline.next(), Some(("foo", None)));
        assert_eq!(cmdline.next(), Some(("bar", Some("some value"))));
        assert_eq!(cmdline.next(), Some(("baz", Some("9"))));
        assert_eq!(cmdline.next(), None);
    }

    #[test]
    fn unmatched_quotes() {
        let source = "foo bar=\"my value";
        let mut cmdline = CmdlineIter { source, cursor: 0 };

        assert_eq!(cmdline.next(), Some(("foo", None)));
        assert_eq!(cmdline.next(), Some(("bar", Some("my value"))));
        assert_eq!(cmdline.next(), None);
    }
}
