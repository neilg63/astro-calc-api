use chrono::NaiveDateTime;
use julian_day_converter::*;
use crate::calc::rise_set_phases::MIN_JD;

/**
 * Get NaiveDateTimeObject for the current date time. Will only fail in exceptional circumstmances
 */
pub fn current_datetime() -> Option<NaiveDateTime> {
  NaiveDateTime::from_timestamp_opt(chrono::offset::Utc::now().timestamp(), 0)
}
/**
 * This should not fail under normal circumstances
 */
pub fn current_datetime_string() -> String {
  if let Some(dt) = current_datetime() {
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
  } else {
    return "".to_string()
  }
  
}

pub fn iso_string_to_datetime(dt_str: &str) -> NaiveDateTime {
  if let Some(dt) = NaiveDateTime::from_fuzzy_iso_string(dt_str) {
    dt
  } else {
    current_datetime().unwrap()
  }
}

/**
 * This should not fail with most JD values
 */
pub fn julian_day_to_iso_datetime(jd: f64) -> String {
  if jd > MIN_JD {  
    if let Ok(dt) = julian_day_to_datetime(jd) {
      dt.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
      return "".to_string()
    }
  } else {
    return "".to_string()
  }
}
