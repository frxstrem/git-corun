use std::fmt::Display;
use std::str::FromStr;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub mod string {
    use super::*;

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToString,
    {
        let value = value.to_string();
        Serialize::serialize(&value, serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        let value: &str = Deserialize::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}
