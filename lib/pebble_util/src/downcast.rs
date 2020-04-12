//! A copy of the `downcast-rs` library, but that has been made `no_std` compatible.
// TODO: this should be upstreamed at some point
/*
 * This file alone is licensed under either MIT and Apache-2 at your discretion, and is:
 *    Copyright (c) 2015 Ashish Myles
 * */
// TODO: fix all the warnings
// TODO: work out and make the tests less confusing

#![deny(unsafe_code)]

use alloc::{boxed::Box, rc::Rc, sync::Arc};
use core::any::Any;

/// Supports conversion to `Any`. Traits to be extended by `impl_downcast!` must extend `Downcast`.
pub trait Downcast: Any {
    /// Convert `Box<Trait>` (where `Trait: Downcast`) to `Box<Any>`. `Box<Any>` can then be
    /// further `downcast` into `Box<ConcreteType>` where `ConcreteType` implements `Trait`.
    fn into_any(self: Box<Self>) -> Box<Any>;
    /// Convert `Rc<Trait>` (where `Trait: Downcast`) to `Rc<Any>`. `Rc<Any>` can then be
    /// further `downcast` into `Rc<ConcreteType>` where `ConcreteType` implements `Trait`.
    fn into_any_rc(self: Rc<Self>) -> Rc<Any>;
    /// Convert `&Trait` (where `Trait: Downcast`) to `&Any`. This is needed since Rust cannot
    /// generate `&Any`'s vtable from `&Trait`'s.
    fn as_any(&self) -> &Any;
    /// Convert `&mut Trait` (where `Trait: Downcast`) to `&Any`. This is needed since Rust cannot
    /// generate `&mut Any`'s vtable from `&mut Trait`'s.
    fn as_any_mut(&mut self) -> &mut Any;
}

impl<T: Any> Downcast for T {
    fn into_any(self: Box<Self>) -> Box<Any> {
        self
    }
    fn into_any_rc(self: Rc<Self>) -> Rc<Any> {
        self
    }
    fn as_any(&self) -> &Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut Any {
        self
    }
}

/// Extends `Downcast` to support `Sync` traits that thus support `Arc` downcasting as well.
pub trait DowncastSync: Downcast + Send + Sync {
    /// Convert `Arc<Trait>` (where `Trait: Downcast`) to `Arc<Any>`. `Arc<Any>` can then be
    /// further `downcast` into `Arc<ConcreteType>` where `ConcreteType` implements `Trait`.
    fn into_any_arc(self: Arc<Self>) -> Arc<Any + Send + Sync>;
}

impl<T: Any + Send + Sync> DowncastSync for T {
    fn into_any_arc(self: Arc<Self>) -> Arc<Any + Send + Sync> {
        self
    }
}

/// Adds downcasting support to traits that extend `downcast::Downcast` by defining forwarding
/// methods to the corresponding implementations on `std::any::Any` in the standard library.
///
/// See https://users.rust-lang.org/t/how-to-create-a-macro-to-impl-a-provided-type-parametrized-trait/5289
/// for why this is implemented this way to support templatized traits.
#[macro_export(local_inner_macros)]
macro_rules! impl_downcast {
    (@impl_full
        $trait_:ident [$($param_types:tt)*]
        for [$($forall_types:ident),*]
        where [$($preds:tt)*]
    ) => {
        impl_downcast! {
            @inject_where
                [impl<$($forall_types),*> $trait_<$($param_types)*>]
                types [$($forall_types),*]
                where [$($preds)*]
                [{
                    impl_downcast! { @impl_body $trait_ [$($param_types)*] }
                }]
        }
    };

    (@impl_full_sync
        $trait_:ident [$($param_types:tt)*]
        for [$($forall_types:ident),*]
        where [$($preds:tt)*]
    ) => {
        impl_downcast! {
            @inject_where
                [impl<$($forall_types),*> $trait_<$($param_types)*>]
                types [$($forall_types),*]
                where [$($preds)*]
                [{
                    impl_downcast! { @impl_body $trait_ [$($param_types)*] }
                    impl_downcast! { @impl_body_sync $trait_ [$($param_types)*] }
                }]
        }
    };

    (@impl_body $trait_:ident [$($types:tt)*]) => {
        /// Returns true if the trait object wraps an object of type `__T`.
        #[inline]
        pub fn is<__T: $trait_<$($types)*>>(&self) -> bool {
            $crate::downcast::Downcast::as_any(self).is::<__T>()
        }
        /// Returns a boxed object from a boxed trait object if the underlying object is of type
        /// `__T`. Returns the original boxed trait if it isn't.
        #[inline]
        pub fn downcast<__T: $trait_<$($types)*>>(
            self: $crate::alloc_reexport::boxed::Box<Self>
        ) -> Result<$crate::alloc_reexport::boxed::Box<__T>, $crate::alloc_reexport::boxed::Box<Self>> {
            if self.is::<__T>() {
                Ok($crate::downcast::Downcast::into_any(self).downcast::<__T>().unwrap())
            } else {
                Err(self)
            }
        }
        /// Returns an `$crate::alloc_reexport::rc::Rc`-ed object from an `$crate::alloc_reexport::rc::Rc`-ed trait object if the underlying object is of
        /// type `__T`. Returns the original `$crate::alloc_reexport::rc::Rc`-ed trait if it isn't.
        #[inline]
        pub fn downcast_rc<__T: $trait_<$($types)*>>(
            self: $crate::alloc_reexport::rc::Rc<Self>
        ) -> Result<$crate::alloc_reexport::rc::Rc<__T>, $crate::alloc_reexport::rc::Rc<Self>> {
            if self.is::<__T>() {
                Ok($crate::downcast::Downcast::into_any_rc(self).downcast::<__T>().unwrap())
            } else {
                Err(self)
            }
        }
        /// Returns a reference to the object within the trait object if it is of type `__T`, or
        /// `None` if it isn't.
        #[inline]
        pub fn downcast_ref<__T: $trait_<$($types)*>>(&self) -> Option<&__T> {
            $crate::downcast::Downcast::as_any(self).downcast_ref::<__T>()
        }
        /// Returns a mutable reference to the object within the trait object if it is of type
        /// `__T`, or `None` if it isn't.
        #[inline]
        pub fn downcast_mut<__T: $trait_<$($types)*>>(&mut self) -> Option<&mut __T> {
            $crate::downcast::Downcast::as_any_mut(self).downcast_mut::<__T>()
        }
    };

    (@impl_body_sync $trait_:ident [$($types:tt)*]) => {
        /// Returns an `Arc`-ed object from an `Arc`-ed trait object if the underlying object is of
        /// type `__T`. Returns the original `Arc`-ed trait if it isn't.
        #[inline]
        pub fn downcast_arc<__T: $trait_<$($types)*>>(
            self: $crate::alloc_reexport::sync::Arc<Self>,
        ) -> Result<$crate::alloc_reexport::sync::Arc<__T>, $crate::alloc_reexport::sync::Arc<Self>>
            where __T: core::any::Any + Send + Sync
        {
            if self.is::<__T>() {
                Ok($crate::downcast::DowncastSync::into_any_arc(self).downcast::<__T>().unwrap())
            } else {
                Err(self)
            }
        }
    };

    (@inject_where [$($before:tt)*] types [] where [] [$($after:tt)*]) => {
        impl_downcast! { @as_item $($before)* $($after)* }
    };

    (@inject_where [$($before:tt)*] types [$($types:ident),*] where [] [$($after:tt)*]) => {
        impl_downcast! {
            @as_item
                $($before)*
                where $( $types: core::any::Any + 'static ),*
                $($after)*
        }
    };
    (@inject_where [$($before:tt)*] types [$($types:ident),*] where [$($preds:tt)+] [$($after:tt)*]) => {
        impl_downcast! {
            @as_item
                $($before)*
                where
                    $( $types: core::any::Any + 'static, )*
                    $($preds)*
                $($after)*
        }
    };

    (@as_item $i:item) => { $i };

    // TODO: can these be merged using optional modifiers and stuff?
    // No type parameters.
    ($trait_:ident   ) => { impl_downcast! { @impl_full $trait_ [] for [] where [] } };
    ($trait_:ident <>) => { impl_downcast! { @impl_full $trait_ [] for [] where [] } };
    (sync $trait_:ident   ) => { impl_downcast! { @impl_full_sync $trait_ [] for [] where [] } };
    (sync $trait_:ident <>) => { impl_downcast! { @impl_full_sync $trait_ [] for [] where [] } };
    // Type parameters.
    ($trait_:ident < $($types:ident),* >) => {
        impl_downcast! { @impl_full $trait_ [$($types),*] for [$($types),*] where [] }
    };
    (sync $trait_:ident < $($types:ident),* >) => {
        impl_downcast! { @impl_full_sync $trait_ [$($types),*] for [$($types),*] where [] }
    };
    // Type parameters and where clauses.
    ($trait_:ident < $($types:ident),* > where $($preds:tt)+) => {
        impl_downcast! { @impl_full $trait_ [$($types),*] for [$($types),*] where [$($preds)*] }
    };
    (sync $trait_:ident < $($types:ident),* > where $($preds:tt)+) => {
        impl_downcast! { @impl_full_sync $trait_ [$($types),*] for [$($types),*] where [$($preds)*] }
    };
    // Associated types.
    ($trait_:ident assoc $($atypes:ident),*) => {
        impl_downcast! { @impl_full $trait_ [$($atypes = $atypes),*] for [$($atypes),*] where [] }
    };
    (sync $trait_:ident assoc $($atypes:ident),*) => {
        impl_downcast! { @impl_full_sync $trait_ [$($atypes = $atypes),*] for [$($atypes),*] where [] }
    };
    // Associated types and where clauses.
    ($trait_:ident assoc $($atypes:ident),* where $($preds:tt)+) => {
        impl_downcast! { @impl_full $trait_ [$($atypes = $atypes),*] for [$($atypes),*] where [$($preds)*] }
    };
    (sync $trait_:ident assoc $($atypes:ident),* where $($preds:tt)+) => {
        impl_downcast! { @impl_full_sync $trait_ [$($atypes = $atypes),*] for [$($atypes),*] where [$($preds)*] }
    };
    // Type parameters and associated types.
    ($trait_:ident < $($types:ident),* > assoc $($atypes:ident),*) => {
        impl_downcast! {
            @impl_full
                $trait_ [$($types),*, $($atypes = $atypes),*]
                for [$($types),*, $($atypes),*]
                where []
        }
    };
    (sync $trait_:ident < $($types:ident),* > assoc $($atypes:ident),*) => {
        impl_downcast! {
            @impl_full_sync
                $trait_ [$($types),*, $($atypes = $atypes),*]
                for [$($types),*, $($atypes),*]
                where []
        }
    };
    // Type parameters, associated types, and where clauses.
    ($trait_:ident < $($types:ident),* > assoc $($atypes:ident),* where $($preds:tt)+) => {
        impl_downcast! {
            @impl_full
                $trait_ [$($types),*, $($atypes = $atypes),*]
                for [$($types),*, $($atypes),*]
                where [$($preds)*]
        }
    };
    (sync $trait_:ident < $($types:ident),* > assoc $($atypes:ident),* where $($preds:tt)+) => {
        impl_downcast! {
            @impl_full_sync
                $trait_ [$($types),*, $($atypes = $atypes),*]
                for [$($types),*, $($atypes),*]
                where [$($preds)*]
        }
    };
    // Concretely-parametrized types.
    (concrete $trait_:ident < $($types:ident),* >) => {
        impl_downcast! { @impl_full $trait_ [$($types),*] for [] where [] }
    };
    (sync concrete $trait_:ident < $($types:ident),* >) => {
        impl_downcast! { @impl_full_sync $trait_ [$($types),*] for [] where [] }
    };
    // Concretely-associated types types.
    (concrete $trait_:ident assoc $($atypes:ident = $aty:ty),*) => {
        impl_downcast! { @impl_full $trait_ [$($atypes = $aty),*] for [] where [] }
    };
    (sync concrete $trait_:ident assoc $($atypes:ident = $aty:ty),*) => {
        impl_downcast! { @impl_full_sync $trait_ [$($atypes = $aty),*] for [] where [] }
    };
    // Concretely-parametrized types with concrete associated types.
    (concrete $trait_:ident < $($types:ident),* > assoc $($atypes:ident = $aty:ty),*) => {
        impl_downcast! { @impl_full $trait_ [$($types),*, $($atypes = $aty),*] for [] where [] }
    };
    (sync concrete $trait_:ident < $($types:ident),* > assoc $($atypes:ident = $aty:ty),*) => {
        impl_downcast! { @impl_full_sync $trait_ [$($types),*, $($atypes = $aty),*] for [] where [] }
    };
}

#[cfg(test)]
mod test {
    macro_rules! test_mod {
        (
            $test_mod_name:ident,
            trait $base_trait:path { $($base_impl:tt)* },
            non_sync: { $($non_sync_def:tt)+ },
            sync: { $($sync_def:tt)+ }
        ) => {
            test_mod! {
                $test_mod_name,
                trait $base_trait { $($base_impl:tt)* },
                type $base_trait,
                non_sync: { $($non_sync_def)* },
                sync: { $($sync_def)* }
            }
        };

        (
            $test_mod_name:ident,
            trait $base_trait:path { $($base_impl:tt)* },
            type $base_type:ty,
            non_sync: { $($non_sync_def:tt)+ },
            sync: { $($sync_def:tt)+ }
        ) => {
            mod $test_mod_name {
                test_mod!(
                    @test
                    $test_mod_name,
                    test_name: test_non_sync,
                    trait $base_trait { $($base_impl)* },
                    type $base_type,
                    { $($non_sync_def)+ },
                    []);

                test_mod!(
                    @test
                    $test_mod_name,
                    test_name: test_sync,
                    trait $base_trait { $($base_impl)* },
                    type $base_type,
                    { $($sync_def)+ },
                    [{
                        // Fail to convert Arc<Base> into Arc<Bar>.
                        let arc: $crate::alloc_reexport::sync::Arc<$base_type> = $crate::alloc_reexport::sync::Arc::new(Foo(42));
                        let res = arc.downcast_arc::<Bar>();
                        assert!(res.is_err());
                        let arc = res.unwrap_err();
                        // Convert Arc<Base> into Arc<Foo>.
                        assert_eq!(
                            42, arc.downcast_arc::<Foo>().map_err(|_| "Shouldn't happen.").unwrap().0);
                    }]);
            }
        };

        (
            @test
            $test_mod_name:ident,
            test_name: $test_name:ident,
            trait $base_trait:path { $($base_impl:tt)* },
            type $base_type:ty,
            { $($def:tt)+ },
            [ $($more_tests:block)* ]
        ) => {
            #[test]
            fn $test_name() {
                #[allow(unused_imports)]
                use super::super::{Downcast, DowncastSync};

                // Should work even if standard objects (especially those in the prelude) are
                // aliased to something else.
                #[allow(dead_code)] struct Any;
                #[allow(dead_code)] struct Arc;
                #[allow(dead_code)] struct Box;
                #[allow(dead_code)] struct Option;
                #[allow(dead_code)] struct Result;
                #[allow(dead_code)] struct $crate::alloc_reexport::rc::Rc;
                #[allow(dead_code)] struct Send;
                #[allow(dead_code)] struct Sync;

                // A trait that can be downcast.
                $($def)*

                // Concrete type implementing Base.
                #[derive(Debug)]
                struct Foo(u32);
                impl $base_trait for Foo { $($base_impl)* }
                #[derive(Debug)]
                struct Bar(f64);
                impl $base_trait for Bar { $($base_impl)* }

                // Functions that can work on references to Base trait objects.
                fn get_val(base: &$crate::alloc_reexport::boxed::Box<$base_type>) -> u32 {
                    match base.downcast_ref::<Foo>() {
                        Some(val) => val.0,
                        None => 0
                    }
                }
                fn set_val(base: &mut $crate::alloc_reexport::boxed::Box<$base_type>, val: u32) {
                    if let Some(foo) = base.downcast_mut::<Foo>() {
                        foo.0 = val;
                    }
                }

                let mut base: $crate::alloc_reexport::boxed::Box<$base_type> = $crate::alloc_reexport::boxed::Box::new(Foo(42));
                assert_eq!(get_val(&base), 42);

                // Try sequential downcasts.
                if let Some(foo) = base.downcast_ref::<Foo>() {
                    assert_eq!(foo.0, 42);
                } else if let Some(bar) = base.downcast_ref::<Bar>() {
                    assert_eq!(bar.0, 42.0);
                }

                set_val(&mut base, 6*9);
                assert_eq!(get_val(&base), 6*9);

                assert!(base.is::<Foo>());

                // Fail to convert Box<Base> into Box<Bar>.
                let res = base.downcast::<Bar>();
                assert!(res.is_err());
                let base = res.unwrap_err();
                // Convert Box<Base> into Box<Foo>.
                assert_eq!(
                    6*9, base.downcast::<Foo>().map_err(|_| "Shouldn't happen.").unwrap().0);

                // Fail to convert $crate::alloc_reexport::rc::Rc<Base> into $crate::alloc_reexport::rc::Rc<Bar>.
                let rc: $crate::alloc_reexport::rc::Rc<$base_type> = $crate::alloc_reexport::rc::Rc::new(Foo(42));
                let res = rc.downcast_rc::<Bar>();
                assert!(res.is_err());
                let rc = res.unwrap_err();
                // Convert $crate::alloc_reexport::rc::Rc<Base> into $crate::alloc_reexport::rc::Rc<Foo>.
                assert_eq!(
                    42, rc.downcast_rc::<Foo>().map_err(|_| "Shouldn't happen.").unwrap().0);

                $($more_tests)*
            }
        };
    }

    test_mod!(non_generic, trait Base {},
    non_sync: {
        trait Base: Downcast {}
        impl_downcast!(Base);
    },
    sync: {
        trait Base: DowncastSync {}
        impl_downcast!(sync Base);
    });

    test_mod!(generic, trait Base<u32> {},
    non_sync: {
        trait Base<T>: Downcast {}
        impl_downcast!(Base<T>);
    },
    sync: {
        trait Base<T>: DowncastSync {}
        impl_downcast!(sync Base<T>);
    });

    test_mod!(constrained_generic, trait Base<u32> {},
    non_sync: {
        trait Base<T: Copy>: Downcast {}
        impl_downcast!(Base<T> where T: Copy);
    },
    sync: {
        trait Base<T: Copy>: DowncastSync {}
        impl_downcast!(sync Base<T> where T: Copy);
    });

    test_mod!(associated, trait Base { type H = f32; }, type Base<H=f32>,
    non_sync: {
        trait Base: Downcast { type H; }
        impl_downcast!(Base assoc H);
    },
    sync: {
        trait Base: DowncastSync { type H; }
        impl_downcast!(sync Base assoc H);
    });

    test_mod!(constrained_associated, trait Base { type H = f32; }, type Base<H=f32>,
    non_sync: {
        trait Base: Downcast { type H: Copy; }
        impl_downcast!(Base assoc H where H: Copy);
    },
    sync: {
        trait Base: DowncastSync { type H: Copy; }
        impl_downcast!(sync Base assoc H where H: Copy);
    });

    test_mod!(param_and_associated, trait Base<u32> { type H = f32; }, type Base<u32, H=f32>,
    non_sync: {
        trait Base<T>: Downcast { type H; }
        impl_downcast!(Base<T> assoc H);
    },
    sync: {
        trait Base<T>: DowncastSync { type H; }
        impl_downcast!(sync Base<T> assoc H);
    });

    test_mod!(constrained_param_and_associated, trait Base<u32> { type H = f32; }, type Base<u32, H=f32>,
    non_sync: {
        trait Base<T: Clone>: Downcast { type H: Copy; }
        impl_downcast!(Base<T> assoc H where T: Clone, H: Copy);
    },
    sync: {
        trait Base<T: Clone>: DowncastSync { type H: Copy; }
        impl_downcast!(sync Base<T> assoc H where T: Clone, H: Copy);
    });

    test_mod!(concrete_parametrized, trait Base<u32> {},
    non_sync: {
        trait Base<T>: Downcast {}
        impl_downcast!(concrete Base<u32>);
    },
    sync: {
        trait Base<T>: DowncastSync {}
        impl_downcast!(sync concrete Base<u32>);
    });

    test_mod!(concrete_associated, trait Base { type H = u32; }, type Base<H=u32>,
    non_sync: {
        trait Base: Downcast { type H; }
        impl_downcast!(concrete Base assoc H=u32);
    },
    sync: {
        trait Base: DowncastSync { type H; }
        impl_downcast!(sync concrete Base assoc H=u32);
    });

    test_mod!(concrete_parametrized_associated, trait Base<u32> { type H = f32; }, type Base<u32, H=f32>,
    non_sync: {
        trait Base<T>: Downcast { type H; }
        impl_downcast!(concrete Base<u32> assoc H=f32);
    },
    sync: {
        trait Base<T>: DowncastSync { type H; }
        impl_downcast!(sync concrete Base<u32> assoc H=f32);
    });
}
