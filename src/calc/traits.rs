use crate::calc::models::geo_pos::GeoPos;

/*
* Serves to match &str keys to enum types
*/
pub trait FromKey<T> {
  fn from_key(key: &str) -> T;
}

pub trait ToKey<T> {
  fn to_key(&self) -> &str;
}

pub trait ToISODateString {
  fn iso_date_string(&self) -> String;
}

pub trait MatchVecKey<T> {
  fn match_by_key(&self, key: &str) -> Option<T>;
}

pub trait AddKeyedItem<T, U> {
    fn add(&mut self, key: &str, value: U) -> Vec<T>;
}

pub trait ToOffsetSecs {
  fn offset_secs(&self) -> i32;
}

pub trait ToGeo {
  fn geo(&self) -> GeoPos;
}