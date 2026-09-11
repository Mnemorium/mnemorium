use config::Map;
use config::Source;
use config::Value;
use config::ValueKind;
use serde_json::Value as JsonValue;

use crate::domain::model::configuration::Configuration;

/// A `config` crate source exposing the configuration singleton row.
///
/// The row is fetched asynchronously before the source is built: `collect`
/// stays synchronous, as required by the `config` crate.
#[derive(Debug, Clone)]
pub struct DbSqlite3Source {
    /// The configuration snapshot collected from the database.
    configuration: Configuration,
}

impl DbSqlite3Source {
    /// Create a new source from a configuration snapshot.
    #[must_use]
    pub fn new(configuration: Configuration) -> Self {
        Self { configuration }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the default `collect_to` sets values through private path machinery that cannot be reimplemented outside the config crate"
)]
impl Source for DbSqlite3Source {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<Map<String, Value>, config::ConfigError> {
        let json = serde_json::to_value(&self.configuration)
            .map_err(|error| config::ConfigError::Foreign(Box::new(error)))?;
        let JsonValue::Object(object) = json else {
            return Err(config::ConfigError::Message(
                "the configuration snapshot is not a JSON object".to_owned(),
            ));
        };

        let mut map = Map::new();
        for (key, value) in object {
            map.insert(key, to_value(value)?);
        }
        Ok(map)
    }
}

/// Convert a `serde_json` value into a `config` crate value.
///
/// # Errors
///
/// Returns an error when the JSON value cannot be represented as a
/// configuration value.
fn to_value(json: JsonValue) -> Result<Value, config::ConfigError> {
    let kind = match json {
        JsonValue::Null => ValueKind::Nil,
        JsonValue::Bool(value) => ValueKind::Boolean(value),
        JsonValue::Number(value) => {
            if let Some(unsigned) = value.as_u64() {
                ValueKind::U64(unsigned)
            } else if let Some(signed) = value.as_i64() {
                ValueKind::I64(signed)
            } else {
                ValueKind::Float(value.as_f64().ok_or_else(|| {
                    config::ConfigError::Message(
                        "the configuration snapshot contains an unrepresentable number".to_owned(),
                    )
                })?)
            }
        }
        JsonValue::String(value) => ValueKind::String(value),
        JsonValue::Array(values) => {
            let mut array = Vec::new();
            for value in values {
                array.push(to_value(value)?);
            }
            ValueKind::Array(array)
        }
        JsonValue::Object(object) => {
            let mut table = Map::new();
            for (key, value) in object {
                table.insert(key, to_value(value)?);
            }
            ValueKind::Table(table)
        }
    };

    Ok(Value::new(None, kind))
}
