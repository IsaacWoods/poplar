# Wire Format of Pebble messages
The wire format describes how messages can be encoded into a stream of bytes suitable for transmission over a
channel, or over another transport layer such as the network or a serial port.

### Primitives
Primitives are transmitted as little-endian and packed to their natural alignment. The following primitive types
are recognised:
| Name                      | Size (bytes)  | Description                                   |
|---------------------------|---------------|-----------------------------------------------|
| `bool`                    | 1             | A boolean value                               |
| `u8`, `u16`, `u32`, `u64` | 1, 2, 4, 8    | An unsigned integer                           |
| `i8`, `i16`, `i32`, `i64` | 1, 2, 4, 8    | A signed integer                              |
| `f32`, `f64`              | 4, 8          | Single / double-precision IEEE-754 FP values  |
| `char`                    | 4             | A single UTF-8 Unicode scalar value           |
