#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TrbType {
    Normal = 1,
    SetupStage,
    DataStage,
    StatusStage,
    Isoch,
    Link,
    EventData,
    NoOp,
    EnableSlot,
    DisableSlot,
    AddressDeviceCommand,
    ConfigureEndpointCommand,
    EvaluateContextCommand,
    ResetEndpoint,
    StopEndpoint,
    SetTRDequeuePointer,
    ResetDevice,
    ForceEventCommand,
    NegotiateBandwidthCommand,
    SetLatencyToleranceValueCommand,
    GetPortBandwidthCommand,
    ForceHeaderCommand,
    NoOpCommand,
    GetExtendedPropertyCommand,
    SetExtendedPropertyCommand,

    TransferEvent = 32,
    CommandCompletionEvent,
    PortStatusChangeEvent,
    BandwidthRequestEvent,
    DoorbellEvent,
    HostControllerEvent,
    DeviceNotificationEvent,
    MFINDEXWrapEvent,
}

/// A Normal TRB is used in several ways:
///    - Exclusively on Bulk and Interrupt Transfer Rings for normal and Scatter/Gather ops
///    - To define additional data buffers for Fine and Coarse Grain Scatter/Gather ops on Isoch Transfer Rings
///    - To define the Data state information for Control Transfer Rings
///
/// They have the structure:
/// ```ignore
///   31                       22              17  16                                                 0
///    +----------------------------------------------------------------------------------------------+ 0x00
///    |   Data Buffer Pointer Lo                                                                     |
///    +----------------------------------------------------------------------------------------------+ 0x04
///    |   Data Buffer Pointer Hi                                                                     |
///    +----------------------------------------------------------------------------------------------+ 0x08
///    |   Interrupter target   |    TD Size    |               TRB Transfer length                   |
///    +----------------------------------------------------------------------------------------------+ 0x0c
///    |   RsvdZ                                    | TRB Type |BEI|RsvdZ |IDT|IOC| CH| NS|ISP|ENT| C |
///    +----------------------------------------------------------------------------------------------+
/// C: Cycle bit
///     Marks the Enqueue Pointer of the Transfer Ring
/// ENT: Evaluate Next TRB
///     If this flag is set, the controller fetches and evaluates the next TRB before saving the enpoint state
/// ISP: Interrupt on Short Packet
///     If this flag is set, the controller generates a Transfer Event TRB if a Short Packet is encountered for
///     this TRB
/// NS: No Snoop
///     If set, the controller may set the No Snoop bit in the Requester Attributes of the PCIe transactions it
///     makes (if the PCIe config also allows it). If software sets this bit, it is responsible for maintaining
///     cache consistency.
/// CH: Chain bit
///     Set if this TRB is associated with the next TRB on the Ring (they are part of the same Transfer
///     Descriptor). Clear for the last TRB in the TD.
/// IOC: Interrupt on Completion
///     If set, the controller will alert software of the completion of this TRB by placing a Transfer Event TRB on
///     the Event Ring and asserting an interrupt. The interrupt may be blocked by BEI.
/// IDT: Immediate Data
///     If set, the Data Buffer Pointer field of this TRB actually contains data, not a pointer. The Length field
///     will contain a value 0..8 for the number of bytes that are valid. TRBs containing immediate data may not be
///     chained.
/// BEI: Block Event Interrupt
///     If this and IOC are set, the controller will not assert an interrupt when the TRB completes.
/// ```
pub struct NormalTrb([u32; 4]);
