use serde::{Serialize, Deserialize};
use crate::calc::dates::julian_day_to_iso_datetime;
use crate::calc::traits::{MatchVecKey, AddKeyedItem};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct KeyNumValue {
  pub key: String,
  pub value: f64,
}

impl KeyNumValue {
  pub fn new(key: &str, value: f64) -> KeyNumValue {
    KeyNumValue { key: key.to_string(), value: value }
  }

  pub fn as_iso_string(&self) -> KeyStringValue {
    KeyStringValue { 
      key: self.key.clone(),
      value: julian_day_to_iso_datetime(self.value)
    }
  }

  pub fn as_flexi_value(&self, iso_mode: bool) -> FlexiValue {
    match self.key.as_str() {
      "min" | "max" => FlexiValue::NumValue(KeyNumValue::new(self.key.as_str(), self.value)),
      _ => match iso_mode {
        true => FlexiValue::StringValue(KeyStringValue::new(self.key.as_str(),  julian_day_to_iso_datetime(self.value).as_str())),
        _ => FlexiValue::NumValue(KeyNumValue::new(self.key.as_str(), self.value)),
      }
    }
  }

}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct KeyNumIdValue {
  pub key: String,
  pub num: u8,
  pub value: f64,
}

impl KeyNumIdValue {
  pub fn new(key: &str, num: u8, value: f64) -> KeyNumIdValue {
    KeyNumIdValue { key: key.to_string(), num, value }
  }
}


impl AddKeyedItem<KeyNumValue, f64> for Vec<KeyNumValue> {
    fn add(&mut self, key: &str, value: f64) -> Vec<KeyNumValue> {
        self.push(KeyNumValue::new(key, value));
        self.to_owned()
    }
}

impl MatchVecKey<KeyNumValue> for Vec<KeyNumValue> {
    fn match_by_key(&self, key: &str) -> Option<KeyNumValue> {
        self.to_owned().into_iter().find(|row| row.to_owned().key.as_str() == key)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyStringValue {
  pub key: String,
  pub value: String,
}

impl KeyStringValue {
  pub fn new(key: &str, value: &str) -> KeyStringValue {
    KeyStringValue { key: key.to_string(), value: value.to_string() }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyStringValueSet {
  pub key: String,
  pub items: Vec<KeyStringValue>,
}

/* impl KeyStringValueSet {
  pub fn new(key: &str, items: Vec<KeyStringValue>) -> KeyStringValueSet {
    KeyStringValueSet { key: key.to_string(), items }
  }
} */

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyNumValueSet {
  pub key: String,
  pub items: Vec<KeyNumValue>,
}

impl KeyNumValueSet {
  pub fn new(key: &str, items: Vec<KeyNumValue>) -> KeyNumValueSet {
    KeyNumValueSet { key: key.to_string(), items }
  }

  pub fn as_flexi_values(&self, iso_mode: bool) -> KeyFlexiValueSet {
    KeyFlexiValueSet::new(self.key.as_str(), self.items.iter().filter(|item| item.value != 0f64).map(|item| match item.key.as_str() {
      "max" | "min" => FlexiValue::NumValue(item.to_owned()),
      _ => match iso_mode {
        true => FlexiValue::StringValue(item.as_iso_string()),
        _ => FlexiValue::NumValue(item.to_owned())
      }
    }).collect() )
  }

}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NumValue {
  pub num: u16,
  pub value: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NumValueKeySet {
  pub num: u16,
  pub key: String,
  pub values: Vec<NumValue>,
}

/**
 * Used for celestial objects
 */

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct LngLat {
  pub lng: f64,
  pub lat: f64,
}

impl LngLat {
  pub fn new(lng: f64, lat: f64) -> LngLat {
    LngLat { lng, lat }
  }
}

pub trait ToLngLat {
  fn to_lng_lat(&self) -> LngLat;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LngLatKey {
  pub lng: f64,
  pub lat: f64,
  pub key: String,
}

/* impl LngLatKey {
  pub fn new(key: &str, lng: f64, lat: f64) -> LngLatKey {
    LngLatKey { key: key.to_string(), lng, lat }
  }
} */

pub trait ToLngLatKey {
  fn to_lng_lat_key(&self) -> LngLatKey;
}


#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum FlexiValueSet {
  NumValues(Vec<KeyNumValueSet>),
  StringValues(Vec<KeyStringValueSet>),
  FlexiValues(Vec<KeyFlexiValueSet>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum FlexiValue {
  NumValue(KeyNumValue),
  StringValue(KeyStringValue),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyFlexiValueSet {
  pub key: String,
  pub items: Vec<FlexiValue>,
}

impl KeyFlexiValueSet {
  pub fn new(key: &str, items: Vec<FlexiValue>) -> KeyFlexiValueSet {
    KeyFlexiValueSet { key: key.to_string(), items }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TimeMode {
  Jd = 0,
  Iso = 1,
  Unix = 2,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SunPeriod {
  pub jd: f64,
  pub start: f64,
  pub end: f64,
  pub night: bool
}

impl SunPeriod {
  pub fn new(jd: f64, start: f64, end: f64, night: bool) -> Self {
    SunPeriod { jd, start, end, night }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum CoordinateSystem {
  Ecliptic = 0,
  Equatorial = 1,
  Dual = 2, // Show Ecliptic and Equatorial (and in the full chart mode horizontal too)
  Horizontal = 3
}

impl CoordinateSystem {
  pub fn to_key(&self) -> String {
    match self {
      CoordinateSystem::Horizontal => "hr",
      CoordinateSystem::Dual => "dl",
      CoordinateSystem::Equatorial => "eq",
      _ => "ec"
    }.to_string()
  }
}