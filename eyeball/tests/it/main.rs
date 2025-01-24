#![allow(missing_docs)]

macro_rules! test {
    (
        $(#[$post_attr:meta])*
        async fn $name:ident() $(-> $ret:ty)? $bl:block
    ) => {
        $(#[$post_attr])*
        #[core::prelude::v1::test]
        fn $name () $(-> $ret)? {
            futures_executor::block_on(async {
                $bl
            })
        }
    };
}

#[cfg(feature = "async-lock")]
mod async_lock;
mod shared;
mod unique;
