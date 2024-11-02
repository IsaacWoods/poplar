# Message Passing
Poplar has a kernel object called a `Channel` for providing first-class message passing support to userspace.
Channels move packets, called "messages", which contain a stream of bytes, and optionally one or more handles that
are transferred from the sending task to the receiving task.

### Ptah
Channels can move arbitrary bytes, but Poplar also includes a layer on top of Channels called Ptah, which
consists of a data model and wire format suitable for encoding data which can be serialized and deserialized from
any sensible language without too much difficulty.

Ptah is used for IPC between tasks running in userspace, and also for more complex communication between the kernel
and userspace.

Ptah is heavily inspired by [Serde](https://serde.rs), and the first implementation of Ptah was actually a [Serde
data format](https://github.com/IsaacWoods/poplar/tree/04f3eed45a40f196a02374ca053aaee16517dccb/lib/ptah).
Unfortunately, it made properly handling Poplar handles very difficult - when a handle is sent over a channel, it
needs to be put into a separate array, and the in-line data replaced by an index into that array.  When the message
travels over a task boundary, the kernel examines and replaces each handle in this array with a handle to the same
kernel object in the new task. This effectively means we need to add a new `Handle` type to our data model, which
is not easily possible with Serde (and would make it incompatible with standard Serde serializers anyway).

### The Ptah Data Model
The Ptah data model maps pretty well to the Rust type system, and relatively closely to the Serde data model. Key
differences are some stronger guarantees about the encoding of types such as enums (the data model only needs to
fit a single wire format, and so can afford to be less flexible than Serde's), and the lack of a few types -
`unit`-based types, and the statically-sized version of `seq` and `map` - `tuple` and `struct`. Ptah is not a
self-describing format (i.e. the type you're trying to (de)serialize must be known at both ends), so the elements
of structs and tuples can simply be serialized in the order they appear, and then deserialized in order at the
other end.

- Primitive types
    - `bool`
    - `u8`, `u16`, `u32`, `u64`, `u128`
    - `i8`, `i16`, `i32`, `i64`, `i128`
    - `f32`, `f64`
    - `char`
- `string`
    - Encoded as a `seq` of `u8`, but with the additional requirement that it is valid UTF-8
    - Not null terminated, as `seq` includes explicit length
- `option`
    - Encoded in the same way as an enum, but separately for the benefit of languages without proper enums
    - Either `None` or `Some({value})`
- `enum`
    - Include a tag, and optionally some data
    - Represent a Rust `enum`, or a tagged union in languages without proper enums
    - The data is encoded separately to the tag, and can be of any other Ptah type:
        - Rust tuple variants (e.g. `E::A(u8, u32)`) are represented by `tuple`
        - Rust struct variants (e.g. `E::B { foo: u8, bar: u32 }`) are represented by `struct`
- `seq`
    - A variable-length sequence of values, mapping to types such as arrays and `Vec<T>`.
- `map`
    - A variable-length series of key-value pairings, mapping to collections like `BTreeMap<K, V>`.
- `handle`
    - A marker in the data stream that a handle to a kernel object is being moved across the channel. The handle itself is encoded out-of-band.
    - This allows the kernel, or something else handling Ptah-encoded data, to process the handle
    - Handles being first-class in the data model is why Poplar can't readily use something like `serde`

### The Ptah Wire Format
The wire format describes how messages can be encoded into a stream of bytes suitable for transmission over a
channel, or over another transport layer such as the network or a serial port.

Primitives are transmitted as little-endian and packed to their natural alignment. The following primitive types
are recognised:

| Name                              | Size (bytes)  | Description                                   |
|-----------------------------------|---------------|-----------------------------------------------|
| `bool`                            | 1             | A boolean value                               |
| `u8`, `u16`, `u32`, `u64`, `u128` | 1, 2, 4, 8    | An unsigned integer                           |
| `i8`, `i16`, `i32`, `i64`, `i128` | 1, 2, 4, 8    | A signed integer                              |
| `f32`, `f64`                      | 4, 8          | Single / double-precision IEEE-754 FP values  |
| `char`                            | 4             | A single UTF-8 Unicode scalar value           |

TODO: rest of the wire format