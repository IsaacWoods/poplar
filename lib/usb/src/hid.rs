#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct HidDescriptor {
    pub length: u8,
    pub typ: u8,
    pub bcd_hid: u16,
    pub country_code: CountryCode,
    /// The number of included class descriptors. Will be `>=1` as a `Report` descriptor will
    /// always be present.
    pub num_descriptors: u8,
    pub descriptor_typ: u8,
    pub descriptor_length: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum CountryCode {
    NotSupported = 0,
    Arabic = 1,
    Belgian = 2,
    CanadianBilingual = 3,
    CanadianFrench = 4,
    Czech = 5,
    Danish = 6,
    Finnish = 7,
    French = 8,
    German = 9,
    Greek = 10,
    Hebrew = 11,
    Hungary = 12,
    International = 13,
    Italian = 14,
    Japan = 15,
    Korean = 16,
    LatinAmerican = 17,
    Dutch = 18,
    Norwegian = 19,
    Farsi = 20,
    Poland = 21,
    Portuguese = 22,
    Russia = 23,
    Slovakia = 24,
    Spanish = 25,
    Swedish = 26,
    SwissFrench = 27,
    SwissGerman = 28,
    Switzerland = 29,
    Taiwan = 30,
    Turkish = 31,
    // 36-255 are reserved
}
