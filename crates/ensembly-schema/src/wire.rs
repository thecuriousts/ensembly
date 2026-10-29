use crate::SCHEMA_VERSION;
use serde::{de::DeserializeOwned, Serialize};
#[derive(Debug, PartialEq, Eq)]
pub struct Decoded<T> {
    pub version: u32,
    pub value: T,
}
#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    #[serde(flatten)]
    body: &'a T,
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<u32>,
}
pub fn decode_line<T: DeserializeOwned>(line: &str) -> Result<Decoded<T>, String> {
    let mut value: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let version = take_version(value.as_object_mut())?;
    let value = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok(Decoded { version, value })
}
#[rustfmt::skip]
fn take_version(obj: Option<&mut serde_json::Map<String, serde_json::Value>>) -> Result<u32, String> {
    let Some(obj) = obj else { return Err("line is not an object".into()); };
    match obj.remove("v") {
        None => Ok(0),
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(u64::from(SCHEMA_VERSION)) => Ok(SCHEMA_VERSION),
        Some(_) => Err("refused v".into()),
    }
}
pub fn encode_line<T: Serialize>(version: u32, value: &T) -> Result<String, String> {
    let v = match version {
        0 => None,
        SCHEMA_VERSION => Some(SCHEMA_VERSION),
        other => return Err(format!("refused v {other}")),
    };
    serde_json::to_string(&Envelope { body: value, v }).map_err(|e| e.to_string())
}
