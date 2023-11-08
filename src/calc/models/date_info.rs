use julian_day_converter::*;
use serde::{Serialize, Deserialize};
use crate::calc::dates::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DateInfo {
  pub utc: String,
  pub jd: f64,
  pub unix: i64
}

impl DateInfo {
    pub fn new(dateref: &str) -> DateInfo {
        let dt = iso_string_to_datetime(dateref);
        DateInfo {
            utc: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
             jd: dt.to_jd(),
             unix: dt.timestamp()
         }
    }

    pub fn new_from_jd(jd: f64) -> DateInfo {
      let dt = julian_day_to_datetime(jd).unwrap();
      DateInfo {
        utc: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        jd: dt.to_jd(),
        unix: dt.timestamp()
      }
    }

    pub fn now() -> DateInfo {
        let dt = current_datetime().unwrap();
        DateInfo {
             utc: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
             jd: dt.to_jd(),
             unix: dt.timestamp()
         }
    }
}
