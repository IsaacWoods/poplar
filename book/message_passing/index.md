# Message Passing
Poplar has a kernel object called a `Channel` for providing first-class message passing support to userspace.
Channels move packets, called "messages", which contain a stream of bytes, and optionally one or more handles that
are transferred from the sending task to the receiving task.

### Ptah
Channels can move arbitrary bytes, but Poplar also includes a layer on top of of Channels called Ptah, which
consists of a data model and wire format suitable for encoding data which can be serialized and deserialized from
any sensible language without too much difficulty.

Ptah is heavily inspired by [Serde](https://serde.rs), and the first implementation of Ptah was actually a [Serde
data format](https://github.com/IsaacWoods/poplar/tree/04f3eed45a40f196a02374ca053aaee16517dccb/lib/ptah).
Unfortunately, it made properly handling Poplar handles very difficult - when a handle is sent over a channel, it
needs to be put into a separate array, and the in-line data replaced by an index into that array.  When the message
travels over a task boundary, the kernel examines and replaces each handle in this array with a handle to the same
kernel object in the new task. This effectively means we need to add a new `Handle` type to our data model, which
is not easily possible with Serde (and would make it incompatible with standard Serde anyway).

### The Ptah Data Model
The Ptah data model maps pretty well to the Rust type system, and relatively closely to the Serde data model. Key
differences are some stronger guarantees about the encoding of types such as enums (the data model only needs to
fit a single wire format, and so can afford to be less flexible than Serde's), and the lack of a few types -
`unit`-based types, and the statically-sized version of `seq` and `map` - `tuple` and `struct`. Ptah is not a
self-describing format (i.e. the types you're trying to deserialize is fully known), so the elements of structs and
tuples can simply be serialized in the order they appear, and then deserialized in order at the other end.

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
    - A variable-length sequence of values, mapping to many types such as `Vec<T>`.
- `map`
    - A variable-length series of key-value pairings, mapping to collections like `BTreeMap<K, V>`.
- `handle`
    - This is the type that means we need our own data model in the first place
    - These are encoded out-of-line of the rest of the data, so that the Poplar kernel can introspect into them, if
      it needs to
