#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct SetupPacket {
    pub typ: RequestType,
    pub request: Request,
    pub value: u16,
    pub index: u16,
    pub length: u16,
}

mycelium_bitfield::bitfield! {
    pub struct RequestType<u8> {
        pub const RECIPIENT: Recipient;
        pub const TYP: RequestTypeType;
        pub const DIRECTION: Direction;
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(PartialEq, Eq, Debug)]
    pub enum Recipient<u8> {
        Device = 0b00000,
        Interface = 0b00001,
        Endpoint = 0b00010,
        Other = 0b00100,
        // XXX: required to make it take up the required number of bits.
        // TODO: Maybe this could be done better via a proc macro that parses the leading zeros
        // too?
        _Dummy = 0b10000,
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(PartialEq, Eq, Debug)]
    pub enum RequestTypeType<u8> {
        Standard = 0b00,
        Class = 0b01,
        Vendor = 0b10,
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(PartialEq, Eq, Debug)]
    pub enum Direction<u8> {
        HostToDevice = 0b0,
        DeviceToHost = 0b1,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Request {
    GetStatus = 0,
    ClearFeature = 1,
    SetFeature = 3,
    SetAddress = 5,
    GetDescriptor = 6,
    SetDescriptor = 7,
    GetConfiguration = 8,
    SetConfiguration = 9,
    GetInterface = 10,
    SetInterface = 11,
    SynchFrame = 12,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_request_type() {
        assert_eq!(
            RequestType::new()
                .with(RequestType::RECIPIENT, Recipient::Endpoint)
                .with(RequestType::TYP, RequestTypeType::Vendor)
                .with(RequestType::DIRECTION, Direction::DeviceToHost)
                .bits(),
            0b1_10_00010
        );
    }
}
