use bit_field::BitField;
use std::{
    collections::VecDeque,
    mem,
    poplar::ddk::dma::{DmaArray, DmaObject, DmaPool, DmaToken},
};
use usb::setup::SetupPacket;

pub struct Queue {
    pub head: DmaObject<QueueHead>,
    pub transactions: VecDeque<Transaction>,
    max_packet_size: u16,
}

pub struct Transaction {
    descriptors: DmaArray<TransferDescriptor>,
    setup: DmaObject<SetupPacket>,
    data: Option<DmaToken>,
}

impl Queue {
    pub fn new(head: DmaObject<QueueHead>, max_packet_size: u16) -> Queue {
        Queue { head, transactions: VecDeque::new(), max_packet_size }
    }

    // TODO: once we have an async runtime, this should return a future that is awoken once the
    // transaction has completed via the IRQ handler
    pub fn control_transfer(
        &mut self,
        setup: SetupPacket,
        data: Option<DmaToken>,
        transfer_to_device: bool,
        pool: &mut DmaPool,
    ) {
        let num_data = if let Some(ref data) = data {
            // TODO: this currently only supports one data TD (transfers up to `0x4000` bytes)
            assert!(data.length < 0x4000);
            1
        } else {
            0
        };

        let setup = pool.create(setup).unwrap();
        let mut transfers = pool.create_array(num_data + 2, TransferDescriptor::new()).unwrap();

        transfers.write(
            0,
            TransferDescriptor {
                next_ptr: TdPtr::new(transfers.phys_of_element(1) as u32, false),
                alt_ptr: TdPtr::new(0x0, true),
                token: TdToken::new()
                    .with(TdToken::ACTIVE, true)
                    // .with(TdToken::INTERRUPT_ON_COMPLETE, true)
                    .with(TdToken::PID_CODE, PidCode::Setup)
                    .with(TdToken::TOTAL_BYTES_TO_TRANSFER, mem::size_of::<SetupPacket>() as u32)
                    .with(TdToken::ERR_COUNTER, 1),
                buffer_ptr_0: setup.phys as u32,
                buffer_ptr_1: 0,
                buffer_ptr_2: 0,
                buffer_ptr_3: 0,
                buffer_ptr_4: 0,
            },
        );

        if let Some(ref data) = data {
            transfers.write(
                1,
                TransferDescriptor {
                    next_ptr: TdPtr::new(transfers.phys_of_element(num_data + 1) as u32, false),
                    alt_ptr: TdPtr::new(0x0, true),
                    token: TdToken::new()
                        .with(TdToken::ACTIVE, true)
                        .with(TdToken::DATA_TOGGLE, true)
                        .with(TdToken::ERR_COUNTER, 1)
                        // .with(TdToken::INTERRUPT_ON_COMPLETE, true)
                        .with(TdToken::PID_CODE, if transfer_to_device { PidCode::Out } else { PidCode::In })
                        .with(TdToken::TOTAL_BYTES_TO_TRANSFER, data.length as u32),
                    buffer_ptr_0: data.phys as u32,
                    buffer_ptr_1: 0,
                    buffer_ptr_2: 0,
                    buffer_ptr_3: 0,
                    buffer_ptr_4: 0,
                },
            );
        }

        // This is the DATA1 token sent by the status stage.
        transfers.write(
            num_data + 1,
            TransferDescriptor {
                next_ptr: TdPtr::new(0x0, true),
                alt_ptr: TdPtr::new(0x0, true),
                token: TdToken::new()
                    .with(TdToken::ACTIVE, true)
                    .with(TdToken::INTERRUPT_ON_COMPLETE, true)
                    .with(TdToken::DATA_TOGGLE, true)
                    .with(TdToken::ERR_COUNTER, 1)
                    .with(TdToken::PID_CODE, if transfer_to_device { PidCode::In } else { PidCode::Out }),
                buffer_ptr_0: 0,
                buffer_ptr_1: 0,
                buffer_ptr_2: 0,
                buffer_ptr_3: 0,
                buffer_ptr_4: 0,
            },
        );

        // TODO: don't just replace `next_td` if we've got running transactions. Need to queue them
        // and somehow progress the queue as stuff completes I think?
        self.head.write().next_td = TdPtr::new(transfers.phys_of_element(0) as u32, false);

        self.transactions.push_back(Transaction { descriptors: transfers, setup, data });
    }

    pub fn set_address(&mut self, address: u8) {
        let endpoint_characteristics = self.head.read().endpoint_characteristics;
        self.head.write().endpoint_characteristics =
            endpoint_characteristics.with(EndpointCharacteristics::DEVICE_ADDRESS, address as u32);
    }

    pub fn set_max_packet_size(&mut self, max_packet_size: u16) {
        let endpoint_characteristics = self.head.read().endpoint_characteristics;
        self.head.write().endpoint_characteristics =
            endpoint_characteristics.with(EndpointCharacteristics::MAX_PACKET_SIZE, max_packet_size as u32);

        self.max_packet_size = max_packet_size;
    }

    pub fn set_reclaim_head(&mut self, head: bool) {
        let endpoint_characteristics = self.head.read().endpoint_characteristics;
        self.head.write().endpoint_characteristics =
            endpoint_characteristics.with(EndpointCharacteristics::HEAD_OF_RECLAMATION_LIST, head);
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, align(32))]
pub struct QueueHead {
    pub horizontal_link: HorizontalLinkPtr,
    pub endpoint_characteristics: EndpointCharacteristics,
    pub endpoint_caps: EndpointCapabilities,
    pub current_td: TdPtr,
    /*
     * The next portion of the queue head is the overlay area - it matches the layout of a qTD,
     * with some extra bits defined.
     */
    pub next_td: TdPtr,
    pub alt_td: TdPtr,
    pub status: TdToken,
    pub buffer_ptr_0: u32,
    pub buffer_ptr_1: u32,
    pub buffer_ptr_2: u32,
    pub buffer_ptr_3: u32,
    pub buffer_ptr_4: u32,
}

impl QueueHead {
    /// Create a new `QueueHead`. This does not initialize the horizontal link, as it does not know
    /// where it will end up in physical memory yet. The overlay area of the current qTD is
    /// zero-initialized - we load the `next_td` field and let the controller initialize the
    /// overlay area for them.
    pub fn new(device: u8, endpoint: u8, max_packet_size: u16) -> QueueHead {
        QueueHead {
            horizontal_link: HorizontalLinkPtr(0x0),
            endpoint_characteristics: EndpointCharacteristics::new()
                .with(EndpointCharacteristics::DEVICE_ADDRESS, device as u32)
                .with(EndpointCharacteristics::ENDPOINT_SPEED, EndpointSpeed::High)
                .with(EndpointCharacteristics::ENDPOINT, endpoint as u32)
                .with(EndpointCharacteristics::MAX_PACKET_SIZE, max_packet_size as u32)
                .with(EndpointCharacteristics::DATA_TOGGLE_CONTROL, true), // TODO: I think this
            // should only be true for control endpoints?
            endpoint_caps: EndpointCapabilities::new().with(EndpointCapabilities::HIGH_BANDWIDTH_MULTIPLIER, 0b01),
            current_td: TdPtr::new(0x0, false),
            next_td: TdPtr::new(0x0, true),
            alt_td: TdPtr::new(0x0, true),
            status: TdToken::new(),
            buffer_ptr_0: 0x0,
            buffer_ptr_1: 0x0,
            buffer_ptr_2: 0x0,
            buffer_ptr_3: 0x0,
            buffer_ptr_4: 0x0,
        }
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(PartialEq, Eq, Debug)]
    pub enum EndpointSpeed<u8> {
        Full = 0b00,
        Low = 0b01,
        High = 0b10,
    }
}

mycelium_bitfield::bitfield! {
    pub struct EndpointCharacteristics<u32> {
        const DEVICE_ADDRESS = 7;
        /// Only used in the Periodic List.
        const INACTIVATE: bool;
        const ENDPOINT = 4;
        const ENDPOINT_SPEED: EndpointSpeed;
        const DATA_TOGGLE_CONTROL: bool;
        /// This bit is used by the controller to correctly detect an empty async schedule. We must
        /// ensure that only one queue head has this bit set, and that it is always coherent with
        /// respect to the schedule.
        const HEAD_OF_RECLAMATION_LIST: bool;
        const MAX_PACKET_SIZE = 10;
        /// Not used for High-Speed devices.
        const CONTROL_ENDPOINT: bool;
        const NAK_RELOAD = 4;
    }
}

mycelium_bitfield::bitfield! {
    pub struct EndpointCapabilities<u32> {
        const INTERRUPT_SCHEDULE_MASK = 8;
        const SPLIT_COMPLETION_MASK = 8;
        const HUB_ADDRESS = 7;
        const PORT_NUMBER = 7;
        const HIGH_BANDWIDTH_MULTIPLIER = 2;
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, align(32))]
pub struct TransferDescriptor {
    pub next_ptr: TdPtr,
    pub alt_ptr: TdPtr,
    pub token: TdToken,
    pub buffer_ptr_0: u32,
    pub buffer_ptr_1: u32,
    pub buffer_ptr_2: u32,
    pub buffer_ptr_3: u32,
    pub buffer_ptr_4: u32,
}

impl TransferDescriptor {
    pub fn new() -> TransferDescriptor {
        TransferDescriptor {
            next_ptr: TdPtr::new(0x0, true),
            alt_ptr: TdPtr::new(0x0, true),
            token: TdToken::new(),
            buffer_ptr_0: 0x0,
            buffer_ptr_1: 0x0,
            buffer_ptr_2: 0x0,
            buffer_ptr_3: 0x0,
            buffer_ptr_4: 0x0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct TdPtr(pub u32);

impl TdPtr {
    pub fn new(ptr: u32, terminate: bool) -> TdPtr {
        let mut value = ptr;
        value.set_bit(0, terminate);
        TdPtr(value)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct HorizontalLinkPtr(pub u32);

impl HorizontalLinkPtr {
    pub fn new(ptr: u32, typ: u32, terminate: bool) -> HorizontalLinkPtr {
        let mut value = ptr;
        value.set_bit(0, terminate);
        value.set_bits(1..3, typ);
        HorizontalLinkPtr(value)
    }
}

mycelium_bitfield::enum_from_bits! {
    /// The 2-bit encodings used by EHCI to encode the token PIDs that should be used for various
    /// transaction types.
    #[derive(Debug)]
    pub enum PidCode<u8> {
        /// Generates a PID of `0b1110_0001`.
        Out = 0b00,
        /// Generates a PID of `0b0110_1001`.
        In = 0b01,
        /// Generates a PID of `0b0010_1101`.
        Setup = 0b10,
    }
}

mycelium_bitfield::bitfield! {
    pub struct TdToken<u32> {
        pub const DO_PING: bool;
        pub const SPLIT_TRANSACTION_STATE: bool;
        pub const MISSED_MICRO_FRAME: bool;
        pub const TRANSACTION_ERR: bool;
        pub const BABBLE_DETECTED: bool;
        pub const DATA_BUFFER_ERR: bool;
        pub const HALTED: bool;
        pub const ACTIVE: bool;
        pub const PID_CODE: PidCode;
        pub const ERR_COUNTER = 2;
        pub const CURRENT_PAGE = 3;
        pub const INTERRUPT_ON_COMPLETE: bool;
        pub const TOTAL_BYTES_TO_TRANSFER = 15;
        pub const DATA_TOGGLE: bool;
    }
}
