#![feature(type_ascription)]

use ptah::{CursorWriter, Deserialize, DeserializeOwned, Serialize};
use std::{collections::BTreeMap, fmt::Debug};

const BUFFER_SIZE: usize = 1024;

fn test_value<T>(value: T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug + 'static,
{
    /*
     * The stdout output will only actually be printed if a test fails, so we print some stuff that might be useful
     * in debugging a failing case.
     */
    let mut buffer = [0u8; BUFFER_SIZE];
    match ptah::to_wire(&value, CursorWriter::new(&mut buffer)) {
        Ok(()) => (),
        Err(err) => panic!("Failed to serialize value: {:?} (err = {:?})", value, err),
    }
    println!("Encoded: {:x?}", buffer);
    let size = match ptah::serialized_size(&value) {
        Ok(size) => size,
        Err(err) => panic!("Failed to calculate serialized size of value: {:?} (err = {:?})", value, err),
    };
    println!("Calculated size of value {:?}: {}", value, size);
    let decoded = match ptah::from_wire(&buffer[0..size]) {
        Ok(value) => value,
        Err(err) => panic!("Failed to deserialize value: {:?} (err = {:?})", value, err),
    };
    assert_eq!(value, decoded);
}

#[test]
fn numbers() {
    test_value(91u8);
    test_value(412u16);
    test_value(912u32);
    test_value(0xf4f4_8273_e5a3_2f54u64);
    test_value(0xf4f4_8273_e5a3_2f54usize);

    test_value(-5i8);
    test_value(345i16);
    test_value(-23556i32);
    test_value(-67i64);
    test_value(-4562isize);

    test_value(34.6f32);
    test_value(-192.3245f64);
}

#[test]
fn bools() {
    test_value(false);
    test_value(true);
}

#[test]
fn arrays() {
    test_value([0xff]);
    test_value([5, 4, 7, 7, 2]);
    test_value([0xff_a4_96_2e_9a_8e_8b_ddu64; 32]);
}

#[test]
fn strings() {
    test_value("".to_string());
    test_value("Hello, World!".to_string());
}

#[test]
fn vec() {
    test_value(vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn simple_struct_manual() {
    #[derive(Clone, Copy, PartialEq, Debug)]
    struct Foo {
        a: u8,
        b: usize,
        c: f64,
    }

    impl Serialize for Foo {
        fn serialize<W>(&self, serializer: &mut ptah::Serializer<W>) -> ptah::ser::Result<()>
        where
            W: ptah::Writer,
        {
            ptah::Serialize::serialize(&self.a, serializer)?;
            ptah::Serialize::serialize(&self.b, serializer)?;
            ptah::Serialize::serialize(&self.c, serializer)?;
            Ok(())
        }
    }

    impl<'de> Deserialize<'de> for Foo {
        fn deserialize(deserializer: &mut ptah::Deserializer<'de>) -> ptah::de::Result<Self> {
            let a: u8 = ptah::Deserialize::deserialize(deserializer)?;
            let b: usize = ptah::Deserialize::deserialize(deserializer)?;
            let c: f64 = ptah::Deserialize::deserialize(deserializer)?;
            Ok(Foo { a, b, c })
        }
    }

    test_value(Foo { a: 0, b: 43, c: 28.99 });
}

#[test]
fn simple_struct_derive() {
    #[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
    struct Foo {
        a: u8,
        b: usize,
        c: f64,
    }

    test_value(Foo { a: 0, b: 43, c: 28.99 });
}

#[test]
fn less_simple_structs() {
    #[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
    struct Foo {
        a: u8,
        b: usize,
        c: f64,
    }

    test_value(Foo { a: 0, b: 43, c: 28.99 });

    #[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
    struct Bar {
        thing_on_heap: String,
        other_heap_thing: Vec<u16>,
        just_a_number: usize,
    }

    test_value(Bar {
        thing_on_heap: "Serde is pretty cool".to_string(),
        other_heap_thing: vec![9, 14, 66, 34, 0],
        just_a_number: 11,
    });
}

#[test]
fn nested_structs() {
    #[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
    struct Egg {
        foo: String,
        bar: usize,
    }

    #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
    struct Nest {
        egg: Egg,
        a: f64,
        other_egg: Egg,
        yet_another_egg: Egg,
    }

    test_value(Nest {
        egg: Egg { foo: "Egg One".to_string(), bar: 43 },
        a: 3.14159265,
        other_egg: Egg { foo: "Egg B".to_string(), bar: 963 },
        yet_another_egg: Egg { foo: "Tertiary Egg".to_string(), bar: 7 },
    });
}

// #[test]
// fn newtype_struct() {
//     #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
//     #[repr(transparent)]
//     struct Foo(u16);

//     test_value(Foo(8));
// }

// #[test]
// fn tuple_struct() {
//     #[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
//     struct Foo(u8, String, ());

//     test_value(Foo(9, "Foo".to_string(), ()));
// }

#[test]
fn unit() {
    test_value(());
}

// #[test]
// fn tuples() {
//     test_value((11,));
//     test_value((0.0f32, 73, "Foo".to_string(), -6));
// }

// #[test]
// fn options() {
//     test_value(None: Option<usize>);
//     test_value(Some(6));
//     test_value(Some("Hello, World!".to_string()));

//     #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
//     struct Foo {
//         a: usize,
//         b: u8,
//     }

//     test_value(Some(Foo { a: 0, b: 43 }));
// }

// #[test]
// fn enums() {
//     #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
//     enum Foo {
//         A,
//         B,
//         C,
//         D,
//     }
//     test_value(Foo::A);
//     test_value(Foo::B);
//     test_value(Foo::C);
//     test_value(Foo::D);

//     #[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
//     enum Bar {
//         None,
//         Some(Vec<usize>),
//     }

//     test_value(Bar::None);
//     test_value(Bar::Some(vec![654, 9]));
// }

// #[test]
// fn maps() {
//     /*
//      * Because we enable the `alloc` feature of serde but not the `std` feature (so it's no_std compatible), we
//      * can't use `HashMap` here.
//      */
//     let mut map = BTreeMap::new();
//     map.insert("one".to_string(), 1);
//     map.insert("two".to_string(), 2);
//     map.insert("three".to_string(), 3);
//     map.insert("four".to_string(), 4);
//     map.insert("seventy-four".to_string(), 74);
//     map.insert("eight-hundred-and-six".to_string(), 806);

//     test_value(map);
// }
