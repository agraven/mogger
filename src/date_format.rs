use chrono::NaiveDateTime;
use serde::{self, Deserialize, Deserializer, Serializer};

const FORMAT: &str = "%s";

pub fn serialize<S>(date: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = format!("{}", date.format(FORMAT));
    serializer.serialize_str(&s)
}

pub fn deserialize<'d, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'d>,
{
    let s = String::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
}
