# Encoding and Decoding messages in Pebble
To send and receive messages between nodes, we need to encode and decode them reliably. This process is incredibly important - message passing is the largest attack surface accessible from programs in
userland, so the kernel must be able to correctly parse all malformed messages without violating safety.

Every message starts with a header, with the format:
```
*----------------------* 0x00
| Destination(NodeId)  |
*----------------------* 0x02
| Payload length (u16) |
*----------------------* 0x04
```

This header is followed by the message's payload, which can represent any type that can be serialized and deserialized by `serde`. This encoding needs to be compact, but also must be verifiable to be the
correct type of message. Pebble uses a custom encoding format, heavily inspired by [BinCode](https://github.com/TyOverby/bincode) and [MessagePack](https://github.com/msgpack/msgpack/blob/master/spec.md).
By effectively having a tiny type system that closely resembles the Serde data model, we can sanity-check that the types we're deserializing into will make at least some sense.

### Encoding types
To handle any type that can be serialized and deserialized using `serde`, we need to be able to handle the 29 types of the Serde Data Model. While lots of these types are specifically tagged, some are
encoded transparently for simplicity and compactness (specifically `Newtype Struct` and `Newtype Variant`). All multi-byte structures are little-endian. `Struct`s and `Tuple`s are simply encoded by encoding
each of their fields in order.

| First byte | Number of following bytes    | Format                        | Description                                       |
| ---------- | ---------------------------- | ----------------------------- | ------------------------------------------------- |
| 0x00       | 0                            |                               | `Unit`, `Unit Struct`, `Unit Variant`             |
| 0x01       | 0                            |                               | False - `Bool`                                    |
| 0x02       | 0                            |                               | True - `Bool`                                     |
| 0x03       | 0                            |                               | None - `Option`                                   |
| 0x04       | `n`                          | XX(`n`)                       | Some - `Option` (followed by a `T`)               |
| 0x05       | 1                            | XX                            | `Char`                                            |
| 0x10       | 1                            | XX                            | `u8`                                              |
| 0x11       | 2                            | XX-XX                         | `u16`                                             |
| 0x12       | 4                            | XX-XX-XX-XX                   | `u32`                                             |
| 0x13       | 8                            | XX-XX-XX-XX-XX-XX-XX-XX       | `u64`                                             |
| 0x20       | 1                            | ZZ                            | `i8`                                              |
| 0x21       | 2                            | ZZ-ZZ                         | `i16`                                             |
| 0x22       | 4                            | ZZ-ZZ-ZZ-ZZ                   | `i32`                                             |
| 0x23       | 8                            | ZZ-ZZ-ZZ-ZZ-ZZ-ZZ-ZZ-ZZ       | `i64`                                             |
| 0x30       | 4                            | FF-FF-FF-FF                   | `f32`                                             |
| 0x31       | 8                            | FF-FF-FF-FF-FF-FF-FF-FF       | `f64`                                             |
| 0x40-0x4F  | (1-16){`n`} + `n` of data    | NN(`first` - 0x3F)-CC(`n`)    | `String`                                          |
| 0x50-0x5F  | (1-16){`n`} + `n` of data    | NN(`first` - 0x4F)-XX(`n`)    | `Byte Array`                                      |
| 0x60       | 4{`n`} + `n` of data         | NN-NN-NN-NN-XX(`n`)           | `Seq`                                             |

#### Strings
Strings are encoded as UTF-8 byte arrays without null terminators. The first byte defines how many bytes are used to encode the length of the string, where `0x40` means 1 byte is used and `0x4F` means 16
bytes are used (this is `x`). The following `x` bytes are used to encode the number of bytes used to encode the string (`n`). Following this are `n` bytes of UTF-8 string data.
