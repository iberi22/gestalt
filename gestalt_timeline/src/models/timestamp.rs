use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A wrapper for timestamps that can be deserialized from both strings and SurrealDB's internal datetime objects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlexibleTimestamp(pub DateTime<Utc>);

impl From<DateTime<Utc>> for FlexibleTimestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

impl From<FlexibleTimestamp> for DateTime<Utc> {
    fn from(ts: FlexibleTimestamp) -> Self {
        ts.0
    }
}

impl FlexibleTimestamp {
    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339()
    }
}

impl fmt::Display for FlexibleTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl Serialize for FlexibleTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as RFC3339 string
        self.0.to_rfc3339().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FlexibleTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}

/// Helper function for serde(deserialize_with = "...")
pub fn deserialize<'de, D>(deserializer: D) -> Result<FlexibleTimestamp, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    let v = Value::deserialize(deserializer)?;

    match v {
        Value::String(s) => {
            let s_clean = if s.starts_with("s'") && s.ends_with("'") {
                &s[2..s.len() - 1]
            } else {
                s.as_str()
            };

            let dt = DateTime::parse_from_rfc3339(s_clean)
                .map_err(|e| {
                    serde::de::Error::custom(format!("Parse error: {} on '{}'", e, s_clean))
                })?
                .with_timezone(&Utc);
            Ok(FlexibleTimestamp(dt))
        }
        Value::Object(map) => {
            if let Some(dt_val) = map.get("datetime").or_else(|| {
                map.get("$surreal")
                    .and_then(|v| v.as_object())
                    .and_then(|io| io.get("datetime"))
            }) {
                if let Some(s) = dt_val.as_str() {
                    let dt = DateTime::parse_from_rfc3339(s)
                        .map_err(serde::de::Error::custom)?
                        .with_timezone(&Utc);
                    return Ok(FlexibleTimestamp(dt));
                }
            }

            if let Some(s) = map.values().next().and_then(|v| v.as_str()) {
                if let Ok(ts) = DateTime::parse_from_rfc3339(s) {
                    return Ok(FlexibleTimestamp(ts.with_timezone(&Utc)));
                }
            }

            Err(serde::de::Error::custom(format!(
                "Expected datetime string or object, found: {:?}",
                map
            )))
        }
        _ => Err(serde::de::Error::custom(
            "Expected string or object for timestamp",
        )),
    }
}

/// Helper function for serde(serialize_with = "...")
pub fn serialize<S>(ts: &FlexibleTimestamp, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ts.serialize(serializer)
}

/// Helper function for Option<FlexibleTimestamp>
pub mod option {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<FlexibleTimestamp>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<serde_json::Value>::deserialize(deserializer)?
            .map(|v| {
                // Manually deserialize from Value
                match v {
                    serde_json::Value::String(s) => {
                        let s_clean = if s.starts_with("s'") && s.ends_with("'") {
                            &s[2..s.len() - 1]
                        } else {
                            s.as_str()
                        };
                        let dt = DateTime::parse_from_rfc3339(s_clean)
                            .map_err(serde::de::Error::custom)?
                            .with_timezone(&Utc);
                        Ok(FlexibleTimestamp(dt))
                    }
                    serde_json::Value::Object(map) => {
                        if let Some(dt_val) = map.get("datetime").or_else(|| {
                            map.get("$surreal")
                                .and_then(|v| v.as_object())
                                .and_then(|io| io.get("datetime"))
                        }) {
                            if let Some(s) = dt_val.as_str() {
                                let dt = DateTime::parse_from_rfc3339(s)
                                    .map_err(serde::de::Error::custom)?
                                    .with_timezone(&Utc);
                                return Ok(FlexibleTimestamp(dt));
                            }
                        }
                        Err(serde::de::Error::custom("Invalid object for timestamp"))
                    }
                    _ => Err(serde::de::Error::custom("Expected string or object")),
                }
            })
            .transpose()
    }

    pub fn serialize<S>(ts: &Option<FlexibleTimestamp>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match ts {
            Some(t) => super::serialize(t, serializer),
            None => serializer.serialize_none(),
        }
    }
}
