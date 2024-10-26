use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub type SchemaName = &'static str;

pub trait Schema {
    const NAME: SchemaName;
    type Key: Serialize + DeserializeOwned + Debug;
    type Value: Serialize + DeserializeOwned + Debug;
}

#[macro_export]
macro_rules! define_schema {
    ($(#[$doc:meta])* ($name:ident) $key:ty => $value:ty) => {
        $(#[$doc])*
        #[derive(Debug)]
        pub struct $name;

        impl $crate::schema::Schema for $name {
            const NAME: $crate::schema::SchemaName = stringify!($name);
            type Key = $key;
            type Value = $value;
        }
    };
}
