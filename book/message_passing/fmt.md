# Message Format
Rust's type system can't be used over process boundaries, and so we need another way to encode and decode messages reliably. The soundness of this is incredible important - message passing is Pebble's
largest attack surface, and so the kernel must be able to correctly parse all malformed messaged without violating safety. We also want to make it as easy as possible for developers to build programs that
can send and receive messages.

Every message starts with a header, with the format (the meaning of the `ProcessId` depends on whether the message is in the Send or Receive buffer):
```
*----------------------------------* 0x00
| Sender / Recepient (ProcessId)   |
*----------------------------------* 0x02
| Payload length (u16)             |
*----------------------------------* 0x04
```

This header is followed by the message's payload, which can represent any type that can be serialized and deserialized by `serde`. This encoding needs to be compact, but also must be verifiable to be the
correct type of message. Pebble uses a custom encoding format, heavily inspired by [BinCode](https://github.com/TyOverby/bincode) and [MessagePack](https://github.com/msgpack/msgpack/blob/master/spec.md).
By effectively having a tiny type system that closely resembles the Serde data model, we can sanity-check that the types we're deserializing into will make at least some sense.

### Encoding types
To handle any type that can be serialized and deserialized using `serde`, we need to be able to handle the 29 types of the Serde Data Model. While lots of these types are specifically tagged, some are
encoded transparently for simplicity and compactness (specifically `Newtype Struct` and `Newtype Variant`). All multi-byte structures are little-endian. `Struct`s and `Tuple`s are simply encoded by encoding
each of their fields in order.

TODO XXX: I don't think this is fully good yet. We should think about how long structures are actually going to be (e.g. how long should a string actually be able to be?), and decide if the extra compactness is worth it for e.g. missing out an extra 1 byte of length info...

| First byte | Number of following bytes    | Format                        | Description                                                                   |
| ---------- | ---------------------------- | ----------------------------- | ----------------------------------------------------------------------------- |
| 0x00       | 0                            |                               | `Unit`, `Unit Struct`, `Unit Variant`                                         |
| 0x01       | 0                            |                               | `bool` - False                                                                |
| 0x02       | 0                            |                               | `bool` - True                                                                 |
| 0x03       | 0                            |                               | `None`                                                                        |
| 0x04       | `n`                          | XX(`n`)                       | `Some(T)` (this marks the `Some`. The `T` is then encoded after this byte)    |
| 0x05       | 1                            | CC                            | `char` - UTF-8 code point that requires 1 byte to encode                      |
| 0x06       | 2                            | CC-CC                         | `char` - UTF-8 code point that requires 2 byte to encode                      |
| 0x07       | 3                            | CC-CC-CC                      | `char` - UTF-8 code point that requires 3 byte to encode                      |
| 0x08       | 4                            | CC-CC-CC-CC                   | `char` - UTF-8 code point that requires 4 byte to encode                      |
| 0x10       | 1                            | XX                            | `u8`                                                                          |
| 0x11       | 2                            | XX-XX                         | `u16`                                                                         |
| 0x12       | 4                            | XX-XX-XX-XX                   | `u32`                                                                         |
| 0x13       | 8                            | XX-XX-XX-XX-XX-XX-XX-XX       | `u64`                                                                         |
| 0x14       | 16                           | XX(16)                        | `u128`                                                                        |
| 0x20       | 1                            | ZZ                            | `i8`                                                                          |
| 0x21       | 2                            | ZZ-ZZ                         | `i16`                                                                         |
| 0x22       | 4                            | ZZ-ZZ-ZZ-ZZ                   | `i32`                                                                         |
| 0x23       | 8                            | ZZ-ZZ-ZZ-ZZ-ZZ-ZZ-ZZ-ZZ       | `i64`                                                                         |
| 0x24       | 16                           | ZZ(16)                        | `i128`                                                                        |
| 0x30       | 4                            | FF-FF-FF-FF                   | `f32`                                                                         |
| 0x31       | 8                            | FF-FF-FF-FF-FF-FF-FF-FF       | `f64`                                                                         |
| 0x40-0x4F  | (1-16){`n`} + `n` of data    | NN(`first` - 0x3F)-CC(`n`)    | `String`                                                                      |
| 0x50-0x5F  | (1-16){`n`} + `n` of data    | NN(`first` - 0x4F)-XX(`n`)    | `Byte Array`                                                                  |
| 0x60       | 4{`n`} + `n` of data         | NN-NN-NN-NN-XX(`n`)           | `Seq`                                                                         |

#### Strings
Strings are encoded as UTF-8 byte arrays without null terminators. The first byte defines how many bytes are used to encode the length of the string, where `0x40` means 1 byte is used and `0x4F` means 16
bytes are used (this is `x`). The following `x` bytes are used to encode the number of bytes used to encode the string (`n`). Following this are `n` bytes of UTF-8 string data.
