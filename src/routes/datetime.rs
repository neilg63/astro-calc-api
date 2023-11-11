use serde_json::*;
use crate::calc::rise_set_phases::TransitionMode;

use super::super::calc::{models::{date_info::*, geo_pos::*},rise_set_phases::{start_jd_geo, to_sun_rise_sets}, utils::{converters::*, validators::*}};
use actix_web::{get, Responder,web::{Query, Json, Path}};
use super::super::query_params::*;

#[get("/jd/{dateref}")]
async fn date_info(dateref: Path<String>) -> impl Responder {
  let date_str = dateref.as_str();
  let info = if is_decimal_str(date_str) { DateInfo::new_from_jd(date_str.parse::<f64>().unwrap()) } else { DateInfo::new(date_str) };
  Json(json!(info))
}

#[get("/date")]
async fn date_info_geo(params: Query<InputOptions>) -> impl Responder {
  let date = to_date_object(&params);
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let tz_secs =  params.tzs.clone().unwrap_or(0i32);
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let offset_secs = if tz_secs != 0i32 { Some(tz_secs) } else { None };
  let (prev, base, next, calc_offset_secs) = to_sun_rise_sets(date.jd, geo, offset_secs, iso_mode, mode);
  Json(json!({ "date": date, "offsetSecs": calc_offset_secs, "sun": { "prev": prev, "current": base, "next": next } }))
}

#[get("/test-geo-start")]
async fn test_geo_start(params: Query<InputOptions>) -> impl Responder {
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let date = to_date_object(&params);
  let start_jd = start_jd_geo(date.jd, geo.lng);
  let start = DateInfo::new_from_jd(start_jd);
  Json(json!({ "date": date, "dayStart": start, "lng": geo.lng, "lat": geo.lat }))
}
