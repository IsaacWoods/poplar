use alloc::{vec, vec::Vec};
use bit_field::BitField;
use core::ops::Range;

#[derive(Debug)]
pub struct ReportDescriptor {
    fields: Vec<ReportField>,
}

#[derive(Debug)]
pub enum ReportField {
    Padding {
        num_bits: u32,
    },
    Array {
        /// The size, in bits, of one element of the array
        size: u32,
        /// The number of elements in the array
        count: u32,
        logical_min: u32,
        logical_max: u32,

        usage_page: u16,
        usage_min: u32,
        usage_max: u32,
    },
    Variable {
        /// Size, in bits
        size: u32,
        data_min: u32,
        data_max: u32,

        usage_page: u16,
        usage_id: u32,
    },
}

#[derive(Debug)]
pub enum FieldValue {
    Selector(Option<Usage>),
    DynamicValue(Usage, bool),
}

impl ReportDescriptor {
    pub fn interpret(&self, report: &[u8]) -> Vec<FieldValue> {
        let mut bit_offset = 0;
        let mut result = Vec::new();

        for field in &self.fields {
            match field {
                ReportField::Padding { num_bits } => bit_offset += num_bits,
                ReportField::Array { size, count, usage_page, usage_min, .. } => {
                    for _ in 0..*count {
                        let value = Self::extract_field_as_u32(report, bit_offset..(bit_offset + size));
                        bit_offset += size;
                        let usage_id = usage_min + value;
                        let usage = translate_usage(*usage_page, usage_id);
                        result.push(FieldValue::Selector(usage));
                    }
                }
                ReportField::Variable { size, usage_page, usage_id, .. } => {
                    let value = Self::extract_field_as_u32(report, bit_offset..(bit_offset + size));
                    bit_offset += size;
                    let usage = translate_usage(*usage_page, *usage_id).unwrap();
                    result.push(FieldValue::DynamicValue(usage, value != 0));
                }
            }
        }

        result
    }

    /// Extract the given range of bits from the report bytes. This is used to extract a field from
    /// the larger report.
    fn extract_field(report: &[u8], bits: Range<u32>) -> Vec<u8> {
        assert!(bits.end.div_ceil(8) as usize <= report.len());
        let mut result = vec![0u8; bits.len().div_ceil(8)];

        for (dst_bit, src_bit) in bits.enumerate() {
            let src_byte = report[(src_bit / 8) as usize];
            let src_bit_value = (src_byte >> (src_bit % 8)) & 0x01;
            let dst_byte_mask = src_bit_value << (dst_bit % 8);
            result[dst_bit / 8] |= dst_byte_mask;
        }

        result
    }

    fn extract_field_as_u32(report: &[u8], bits: Range<u32>) -> u32 {
        let bits = Self::extract_field(report, bits);
        let mut bytes = [0u8; 4];
        bytes[..bits.len()].copy_from_slice(&bits);
        u32::from_le_bytes(bytes)
    }
}

#[derive(Debug)]
struct GlobalState {
    pub usage_page: Option<u16>,
    // TODO: these can actually all be signed or unsigned I think??
    pub logical_min: Option<u32>,
    pub logical_max: Option<u32>,
    pub report_size: Option<u32>,
    pub report_count: Option<u32>,
    pub physical_min: Option<u32>,
    pub physical_max: Option<u32>,
}

impl GlobalState {
    pub fn new() -> GlobalState {
        GlobalState {
            usage_page: None,
            logical_min: None,
            logical_max: None,
            report_size: None,
            report_count: None,
            physical_min: None,
            physical_max: None,
        }
    }
}

#[derive(Debug)]
struct LocalState {
    pub usage: Vec<u32>,
    pub usage_min: Option<u32>,
    pub usage_max: Option<u32>,
}

impl LocalState {
    pub fn new() -> LocalState {
        LocalState { usage: Vec::new(), usage_min: None, usage_max: None }
    }
}

pub struct ReportDescriptorParser {
    descriptor: ReportDescriptor,
    local: LocalState,
    global: GlobalState,
}

impl ReportDescriptorParser {
    pub fn parse(bytes: &[u8]) -> ReportDescriptor {
        let tokenizer = ItemTokenizer::new(bytes);
        let mut parser = ReportDescriptorParser {
            descriptor: ReportDescriptor { fields: Vec::new() },
            local: LocalState::new(),
            global: GlobalState::new(),
        };

        for item in tokenizer {
            match item.typ {
                ItemType::Main => parser.parse_main_item(&item),
                ItemType::Global => parser.parse_global_item(&item),
                ItemType::Local => parser.parse_local_item(&item),
                ItemType::Reserved => panic!("Unhandled reserved item type in HID report descriptor"),
            }
        }

        parser.descriptor
    }

    fn parse_main_item(&mut self, item: &Item) {
        match item.tag {
            0b1000 => {
                // Input
                let is_array = !item.data_as_u32().get_bit(1);
                self.generate_fields(is_array);
                self.local = LocalState::new();
            }
            0b1001 => {
                // Output
                // TODO: we might want to handle these at some point for e.g. keyboard LEDs
                self.local = LocalState::new();
            }
            0b1011 => {
                // Feature
                self.local = LocalState::new();
            }
            0b1010 => {
                // Collection
                self.local = LocalState::new();
            }
            0b1100 => {
                // End collection
            }
            _ => panic!("Reserved tag on main item!"),
        }
    }

    fn parse_global_item(&mut self, item: &Item) {
        match item.tag {
            0b0000 => {
                // Usage
                self.global.usage_page = Some(item.data_as_u16());
            }
            0b0001 => {
                // Logical minimum
                self.global.logical_min = Some(item.data_as_u32());
            }
            0b0010 => {
                // Logical maximum
                self.global.logical_max = Some(item.data_as_u32());
            }
            0b0011 => {
                // Physical minimum
                self.global.physical_min = Some(item.data_as_u32());
            }
            0b0100 => {
                // Physical maximum
                self.global.physical_max = Some(item.data_as_u32());
            }
            0b0101 => {
                todo!("Unit exponent")
            }
            0b0110 => {
                todo!("Unit")
            }
            0b0111 => {
                // Report size
                self.global.report_size = Some(item.data_as_u32());
            }
            0b1000 => {
                todo!("Report ID")
            }
            0b1001 => {
                // Report count
                self.global.report_count = Some(item.data_as_u32());
            }
            0b1010 => {
                todo!("Push")
            }
            0b1011 => {
                todo!("Pop")
            }
            _ => panic!("Reserved tag on global item!"),
        }
    }

    fn parse_local_item(&mut self, item: &Item) {
        match item.tag {
            0b0000 => {
                // Usage
                self.local.usage.push(item.data_as_u32());
            }
            0b0001 => {
                // Usage minimum
                assert!(item.data.len() < 4, "Overriding the usage page is not supported!");
                let min = item.data_as_u32();
                self.local.usage_min = Some(min);
            }
            0b0010 => {
                // Usage maximum
                assert!(item.data.len() < 4, "Overriding the usage page is not supported!");
                let max = item.data_as_u32();
                self.local.usage_max = Some(max);
            }
            0b0011 => {
                todo!("Designator index")
            }
            0b0100 => {
                todo!("Designator minimum")
            }
            0b0101 => {
                todo!("Designator maximum")
            }
            0b0111 => {
                todo!("String index")
            }
            0b1000 => {
                todo!("Delimiter")
            }
            _ => panic!("Reserved tag on local item!"),
        }
    }

    fn generate_fields(&mut self, is_array: bool) {
        if self.global.report_size.is_none() || self.global.report_count.is_none() {
            panic!("Tried to generate fields without specified report size or count!");
        }

        if self.local.usage.is_empty() && self.local.usage_min.is_none() && self.local.usage_max.is_none() {
            // If no usages are specified, this field describes padding
            let padding = self.global.report_size.unwrap() * self.global.report_count.unwrap();
            self.descriptor.fields.push(ReportField::Padding { num_bits: padding });
        } else if is_array {
            let logical_min = self.global.logical_min.unwrap();
            let logical_max = self.global.logical_max.unwrap();

            // TODO: support signed values if we end up needing to
            assert!(!i32::try_from(logical_min).unwrap().is_negative());

            self.descriptor.fields.push(ReportField::Array {
                size: self.global.report_size.unwrap(),
                count: self.global.report_count.unwrap(),
                logical_min,
                logical_max,
                usage_page: self.global.usage_page.unwrap(),
                usage_min: self.local.usage_min.unwrap(),
                usage_max: self.local.usage_max.unwrap(),
            });
        } else {
            for i in 0..self.global.report_count.unwrap() {
                let usage_id = if self.local.usage.is_empty() {
                    self.local.usage_min.unwrap() + i
                } else {
                    *self.local.usage.get(i as usize).unwrap()
                };

                self.descriptor.fields.push(ReportField::Variable {
                    size: self.global.report_size.unwrap(),
                    data_min: self.global.logical_min.unwrap(),
                    data_max: self.global.logical_max.unwrap(),

                    usage_page: self.global.usage_page.unwrap(),
                    usage_id,
                });
            }
        }
    }
}

pub struct ItemTokenizer<'a> {
    bytes: &'a [u8],
    current: usize,
}

impl<'a> ItemTokenizer<'a> {
    pub fn new(bytes: &'a [u8]) -> ItemTokenizer<'a> {
        ItemTokenizer { bytes, current: 0 }
    }

    fn take_byte(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.current)?;
        self.current += 1;
        Some(byte)
    }
}

impl<'a> Iterator for ItemTokenizer<'a> {
    type Item = Item<'a>;

    fn next(&mut self) -> Option<Item<'a>> {
        let next = self.take_byte()?;

        let mut size = match next.get_bits(0..2) {
            // XXX: short items of size `4` bytes are encoded as `0b11` (3)
            0b11 => 4,
            other => other,
        };
        let typ = match next.get_bits(2..4) {
            0 => ItemType::Main,
            1 => ItemType::Global,
            2 => ItemType::Local,
            3 => ItemType::Reserved,
            _ => unreachable!(),
        };
        let mut tag = next.get_bits(4..8);

        // Detect long items and parse the full size and tag fields
        if size == 2 && tag == 0xf && typ == ItemType::Reserved {
            size = self.take_byte()?;
            tag = self.take_byte()?;
        }

        let data = self.bytes.get(self.current..(self.current + size as usize))?;
        self.current += size as usize;

        Some(Item { typ, tag, data })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Item<'a> {
    pub typ: ItemType,
    pub tag: u8,
    pub data: &'a [u8],
}

impl<'a> Item<'a> {
    /// Interprets up to 2 bytes of `data` as a `u16`. In the case that there are less than 2
    /// bytes, the upper bytes will be padded with zeros.
    pub fn data_as_u16(&self) -> u16 {
        let mut bytes = [0; 2];
        bytes[..self.data.len()].copy_from_slice(self.data);
        u16::from_le_bytes(bytes)
    }

    /// Interprets up to 4 bytes of `data` as a `u32`. In the case that there are less than 4
    /// bytes, the upper bytes will be padded with zeros.
    pub fn data_as_u32(&self) -> u32 {
        let mut bytes = [0; 4];
        bytes[..self.data.len()].copy_from_slice(self.data);
        u32::from_le_bytes(bytes)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemType {
    Main,
    Global,
    Local,
    Reserved,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Usage {
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    KeyReturn,
    KeyEscape,
    KeyDelete,
    KeyTab,
    KeySpace,
    KeyDash,
    KeyEquals,
    KeyLeftBracket,
    KeyRightBracket,
    KeyForwardSlash,
    KeyPound,
    KeySemicolon,
    KeyApostrophe,
    KeyGrave,
    KeyComma,
    KeyDot,
    KeyBackSlash,
    KeyCapslock,
    KeyF1,
    KeyF2,
    KeyF3,
    KeyF4,
    KeyF5,
    KeyF6,
    KeyF7,
    KeyF8,
    KeyF9,
    KeyF10,
    KeyF11,
    KeyF12,
    KeyPrintScreen,
    KeyScrolllock,
    KeyPause,
    KeyInsert,
    KeyHome,
    KeyPageUp,
    KeyDeleteForward,
    KeyEnd,
    KeyPageDown,
    KeyRightArrow,
    KeyLeftArrow,
    KeyDownArrow,
    KeyUpArrow,
    KeyNumlock,
    KeypadSlash,
    KeypadAsterix,
    KeypadDash,
    KeypadPlus,
    KeypadEnter,
    Keypad1,
    Keypad2,
    Keypad3,
    Keypad4,
    Keypad5,
    Keypad6,
    Keypad7,
    Keypad8,
    Keypad9,
    Keypad0,
    KeypadDot,
    KeypadNonUsBackSlash,
    KeyApplication,
    KeyPower,
    KeypadEquals,
    KeyF13,
    KeyF14,
    KeyF15,
    KeyF16,
    KeyF17,
    KeyF18,
    KeyF19,
    KeyF20,
    KeyF21,
    KeyF22,
    KeyF23,
    KeyF24,
    KeyExecute,
    KeyHelp,
    KeyMenu,
    KeySelect,
    KeyStop,
    KeyAgain,
    KeyUndo,
    KeyCut,
    KeyCopy,
    KeyPaste,
    KeyFind,
    KeyMute,
    KeyVolumeUp,
    KeyVolumeDown,
    KeyLockingCapslock,
    KeyLockingNumlock,
    KeyLockingScrolllock,
    KeypadComma,
    // TODO: a bunch missing here bc I got bored
    KeyLeftControl,
    KeyLeftShift,
    KeyLeftAlt,
    KeyLeftGui,
    KeyRightControl,
    KeyRightShift,
    KeyRightAlt,
    KeyRightGui,
}

pub fn translate_usage(usage_page: u16, usage_id: u32) -> Option<Usage> {
    match usage_page {
        0x07 => {
            // Keyboard/Keypad page
            match usage_id {
                0x04 => Some(Usage::KeyA),
                0x05 => Some(Usage::KeyB),
                0x06 => Some(Usage::KeyC),
                0x07 => Some(Usage::KeyD),
                0x08 => Some(Usage::KeyE),
                0x09 => Some(Usage::KeyF),
                0x0a => Some(Usage::KeyG),
                0x0b => Some(Usage::KeyH),
                0x0c => Some(Usage::KeyI),
                0x0d => Some(Usage::KeyJ),
                0x0e => Some(Usage::KeyK),
                0x0f => Some(Usage::KeyL),
                0x10 => Some(Usage::KeyM),
                0x11 => Some(Usage::KeyN),
                0x12 => Some(Usage::KeyO),
                0x13 => Some(Usage::KeyP),
                0x14 => Some(Usage::KeyQ),
                0x15 => Some(Usage::KeyR),
                0x16 => Some(Usage::KeyS),
                0x17 => Some(Usage::KeyT),
                0x18 => Some(Usage::KeyU),
                0x19 => Some(Usage::KeyV),
                0x1a => Some(Usage::KeyW),
                0x1b => Some(Usage::KeyX),
                0x1c => Some(Usage::KeyY),
                0x1d => Some(Usage::KeyZ),
                0x1e => Some(Usage::Key1),
                0x1f => Some(Usage::Key2),
                0x20 => Some(Usage::Key3),
                0x21 => Some(Usage::Key4),
                0x22 => Some(Usage::Key5),
                0x23 => Some(Usage::Key6),
                0x24 => Some(Usage::Key7),
                0x25 => Some(Usage::Key8),
                0x26 => Some(Usage::Key9),
                0x27 => Some(Usage::Key0),
                0x28 => Some(Usage::KeyReturn),
                0x29 => Some(Usage::KeyEscape),
                0x2a => Some(Usage::KeyDelete),
                0x2b => Some(Usage::KeyTab),
                0x2c => Some(Usage::KeySpace),
                0x2d => Some(Usage::KeyDash),
                0x2e => Some(Usage::KeyEquals),
                0x2f => Some(Usage::KeyLeftBracket),
                0x30 => Some(Usage::KeyRightBracket),
                0x31 => Some(Usage::KeyForwardSlash),
                0x32 => Some(Usage::KeyPound),
                0x33 => Some(Usage::KeySemicolon),
                0x34 => Some(Usage::KeyApostrophe),
                0x35 => Some(Usage::KeyGrave),
                0x36 => Some(Usage::KeyComma),
                0x37 => Some(Usage::KeyDot),
                0x38 => Some(Usage::KeyBackSlash),
                0x39 => Some(Usage::KeyCapslock),
                0x3a => Some(Usage::KeyF1),
                0x3b => Some(Usage::KeyF2),
                0x3c => Some(Usage::KeyF3),
                0x3d => Some(Usage::KeyF4),
                0x3e => Some(Usage::KeyF5),
                0x3f => Some(Usage::KeyF6),
                0x40 => Some(Usage::KeyF7),
                0x41 => Some(Usage::KeyF8),
                0x42 => Some(Usage::KeyF9),
                0x43 => Some(Usage::KeyF10),
                0x44 => Some(Usage::KeyF11),
                0x45 => Some(Usage::KeyF12),
                0x46 => Some(Usage::KeyPrintScreen),
                0x47 => Some(Usage::KeyScrolllock),
                0x48 => Some(Usage::KeyPause),
                0x49 => Some(Usage::KeyInsert),
                0x4a => Some(Usage::KeyHome),
                0x4b => Some(Usage::KeyPageUp),
                0x4c => Some(Usage::KeyDeleteForward),
                0x4d => Some(Usage::KeyEnd),
                0x4e => Some(Usage::KeyPageDown),
                0x4f => Some(Usage::KeyRightArrow),
                0x50 => Some(Usage::KeyLeftArrow),
                0x51 => Some(Usage::KeyDownArrow),
                0x52 => Some(Usage::KeyUpArrow),
                0x53 => Some(Usage::KeyNumlock),
                0x54 => Some(Usage::KeypadSlash),
                0x55 => Some(Usage::KeypadAsterix),
                0x56 => Some(Usage::KeypadDash),
                0x57 => Some(Usage::KeypadPlus),
                0x58 => Some(Usage::KeypadEnter),
                0x59 => Some(Usage::Keypad1),
                0x5a => Some(Usage::Keypad2),
                0x5b => Some(Usage::Keypad3),
                0x5c => Some(Usage::Keypad4),
                0x5d => Some(Usage::Keypad5),
                0x5e => Some(Usage::Keypad6),
                0x5f => Some(Usage::Keypad7),
                0x60 => Some(Usage::Keypad8),
                0x61 => Some(Usage::Keypad9),
                0x62 => Some(Usage::Keypad0),
                0x63 => Some(Usage::KeypadDot),
                0x64 => Some(Usage::KeypadNonUsBackSlash),
                0x65 => Some(Usage::KeyApplication),
                0x66 => Some(Usage::KeyPower),
                0x67 => Some(Usage::KeypadEquals),
                0x68 => Some(Usage::KeyF13),
                0x69 => Some(Usage::KeyF14),
                0x6a => Some(Usage::KeyF15),
                0x6b => Some(Usage::KeyF16),
                0x6c => Some(Usage::KeyF17),
                0x6d => Some(Usage::KeyF18),
                0x6e => Some(Usage::KeyF19),
                0x6f => Some(Usage::KeyF20),
                0x70 => Some(Usage::KeyF21),
                0x71 => Some(Usage::KeyF22),
                0x72 => Some(Usage::KeyF23),
                0x73 => Some(Usage::KeyF24),
                0x74 => Some(Usage::KeyExecute),
                0x75 => Some(Usage::KeyHelp),
                0x76 => Some(Usage::KeyMenu),
                0x77 => Some(Usage::KeySelect),
                0x78 => Some(Usage::KeyStop),
                0x79 => Some(Usage::KeyAgain),
                0x7a => Some(Usage::KeyUndo),
                0x7b => Some(Usage::KeyCut),
                0x7c => Some(Usage::KeyCopy),
                0x7d => Some(Usage::KeyPaste),
                0x7e => Some(Usage::KeyFind),
                0x7f => Some(Usage::KeyMute),
                0x80 => Some(Usage::KeyVolumeUp),
                0x81 => Some(Usage::KeyVolumeDown),
                0x82 => Some(Usage::KeyLockingCapslock),
                0x83 => Some(Usage::KeyLockingNumlock),
                0x84 => Some(Usage::KeyLockingScrolllock),
                0x85 => Some(Usage::KeypadComma),
                0xe0 => Some(Usage::KeyLeftControl),
                0xe1 => Some(Usage::KeyLeftShift),
                0xe2 => Some(Usage::KeyLeftAlt),
                0xe3 => Some(Usage::KeyLeftGui),
                0xe4 => Some(Usage::KeyRightControl),
                0xe5 => Some(Usage::KeyRightShift),
                0xe6 => Some(Usage::KeyRightAlt),
                0xe7 => Some(Usage::KeyRightGui),
                _ => None,
            }
        }
        _ => None,
    }
}
