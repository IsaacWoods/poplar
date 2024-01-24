use bit_field::BitField;
use bitflags::bitflags;
use log::info;
use std::{
    mem,
    ops::Deref,
    poplar::ddk::dma::{DmaArray, DmaObject, DmaPool},
};
use usb::setup::SetupPacket;

pub struct Queue {
    pub head: DmaObject<QueueHead>,
    pub transactions: Vec<Transaction>,
}

pub struct Transaction {
    pub descriptors: DmaArray<TransferDescriptor>,
}

impl Queue {
    pub fn new(head: DmaObject<QueueHead>) -> Queue {
        Queue { head, transactions: Vec::new() }
    }

    // TODO: take a buffer and do stuff with it?
    pub fn control_transfer(
        &mut self,
        setup: &DmaObject<SetupPacket>,
        transfer_to_device: bool,
        pool: &mut DmaPool,
    ) {
        let num_data = 0;

        let mut transfers = pool.create_array(num_data + 2, TransferDescriptor::new()).unwrap();

        transfers.write(
            0,
            TransferDescriptor {
                next_ptr: TdPtr::new(transfers.phys_of_element(1) as u32, false),
                alt_ptr: TdPtr::new(0x0, true),
                token: TdToken::STATUS_ACTIVE
                    | TdToken::INTERRUPT_ON_COMPLETE
                    | TdToken::with_pid_code(0b10)
                    | TdToken::with_total_bytes(mem::size_of::<SetupPacket>() as u32),
                buffer_ptr_0: setup.phys as u32,
                buffer_ptr_1: 0,
                buffer_ptr_2: 0,
                buffer_ptr_3: 0,
                buffer_ptr_4: 0,
            },
        );

        // TODO: if there are data things we need to generate more TDs in the chain here

        transfers.write(
            num_data + 1,
            TransferDescriptor {
                next_ptr: TdPtr::new(0x0, true),
                alt_ptr: TdPtr::new(0x0, true),
                token: TdToken::STATUS_ACTIVE
                    | TdToken::INTERRUPT_ON_COMPLETE
                    | TdToken::DATA_TOGGLE
                    | TdToken::with_pid_code(if transfer_to_device { 0b01 } else { 0b00 }),
                buffer_ptr_0: 0,
                buffer_ptr_1: 0,
                buffer_ptr_2: 0,
                buffer_ptr_3: 0,
                buffer_ptr_4: 0,
            },
        );

        // TODO: don't just replace `next_td` if we've got running transactions. Need to queue them
        // and somehow progress the queue as stuff completes I think?
        self.head.next_td = TdPtr::new(transfers.phys_of_element(0) as u32, false);

        self.transactions.push(Transaction { descriptors: transfers });
    }

    pub fn set_address(&mut self, address: u8) {
        let endpoint_characteristics = self.head.endpoint_characteristics;
        self.head.endpoint_characteristics =
            endpoint_characteristics.with(EndpointCharacteristics::DEVICE_ADDRESS, address as u32);
    }

    pub fn set_reclaim_head(&mut self, head: bool) {
        let endpoint_characteristics = self.head.endpoint_characteristics;
        self.head.endpoint_characteristics =
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
    pub status: QueueHeadStatus,
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
    pub fn new() -> QueueHead {
        QueueHead {
            horizontal_link: HorizontalLinkPtr(0x0),
            endpoint_characteristics: EndpointCharacteristics::new()
                .with(EndpointCharacteristics::ENDPOINT_SPEED, EndpointSpeed::High)
                .with(EndpointCharacteristics::MAX_PACKET_SIZE, 64),
            endpoint_caps: EndpointCapabilities::new().with(EndpointCapabilities::HIGH_BANDWIDTH_MULTIPLIER, 0b01),
            current_td: TdPtr::new(0x0, false),
            next_td: TdPtr::new(0x0, true),
            alt_td: TdPtr::new(0x0, true),
            status: QueueHeadStatus::new(),
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
#[repr(transparent)]
pub struct QueueHeadStatus(pub u32);

impl QueueHeadStatus {
    pub fn new() -> QueueHeadStatus {
        let mut value = 0;
        QueueHeadStatus(value)
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
            token: TdToken::empty(),
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
pub struct TdPtr(u32);

impl TdPtr {
    pub fn new(ptr: u32, terminate: bool) -> TdPtr {
        let mut value = ptr;
        value.set_bit(0, terminate);
        TdPtr(value)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct HorizontalLinkPtr(u32);

impl HorizontalLinkPtr {
    pub fn new(ptr: u32, typ: u32, terminate: bool) -> HorizontalLinkPtr {
        let mut value = ptr;
        value.set_bit(0, terminate);
        value.set_bits(1..3, typ);
        HorizontalLinkPtr(value)
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct TdToken: u32 {
        const STATUS_DO_PING = 1 << 0;
        const STATUS_DO_SPLIT = 1 << 1;
        const STATUS_MISSED_MICRO_FRAME = 1 << 2;
        const STATUS_TRANSACTION_ERR = 1 << 3;
        const STATUS_BABBLE_DETECTED = 1 << 4;
        const STATUS_DATA_BUFFER_ERR = 1 << 5;
        const STATUS_HALTED = 1 << 6;
        const STATUS_ACTIVE = 1 << 7;
        const INTERRUPT_ON_COMPLETE = 1 << 15;
        const DATA_TOGGLE = 1 << 31;

        /*
         * Mark the PID Code, Error Counter, Current Page, and Total Bytes to Transfer fields as
         * known bits. This makes behaviour of `bitflags`-generated methods correct.
         */
        const _ = 0b0_111111111111111_0_111_11_11_00000000;
    }
}

impl TdToken {
    /// Set the token that should be used for transactions associated with this transfer
    /// descriptor:
    ///    `0b00` => `OUT` Token
    ///    `0b01` => `IN` Token
    ///    `0b10` => `SETUP` Token
    pub fn with_pid_code(code: u32) -> TdToken {
        let mut value = 0u32;
        value.set_bits(8..10, code);
        // TODO: this is obvs not supposed to be here
        // value.set_bits(10..12, 1);
        TdToken::from_bits_retain(value)
    }

    pub fn with_total_bytes(bytes: u32) -> TdToken {
        let mut value = 0u32;
        value.set_bits(16..31, bytes);
        TdToken::from_bits_retain(value)
    }
}
