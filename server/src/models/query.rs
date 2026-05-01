use serde::de;
use serde::Deserialize;

/// Deserialize `Option<String>` → `Option<i64>` for query params.
pub fn deserialize_i64_from_str<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(de)?;
    match opt {
        Some(s) => s.parse::<i64>().map(Some).map_err(de::Error::custom),
        None => Ok(None),
    }
}

/// Deserialize `Option<String>` → `Option<f64>` for query params.
pub fn deserialize_f64_from_str<'de, D>(de: D) -> Result<Option<f64>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(de)?;
    match opt {
        Some(s) => s.parse::<f64>().map(Some).map_err(de::Error::custom),
        None => Ok(None),
    }
}
