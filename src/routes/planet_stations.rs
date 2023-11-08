use std::{thread, time};
use serde_json::*;
use super::super::calc::{ dates::current_datetime_string, models::date_info::*, planet_stations::{match_all_planet_stations_range, BodySpeedSet}, utils::converters::*};
use actix_web::{get, Responder,web::{Query, Json}};
use super::super::{query_params::*, reset_ephemeris_path};

#[get("/planet-stations")]
async fn planet_stations_progress(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(30);  
  let date = to_date_object(&params);
  let def_keys = vec!["me", "ve", "ma", "ju", "sa", "ur", "ne", "pl"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let dateref_end: String = params.dt2.clone().unwrap_or(current_datetime_string());
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let end_date = DateInfo::new(dateref_end.to_string().as_str());
  let items: Vec<BodySpeedSet> = match_all_planet_stations_range(date.jd, end_date.jd, to_str_refs(&keys), iso_mode);
  let valid = items.len() > 0;
  thread::sleep(micro_interval);
  Json(json!({ "valid": valid, "start": date,  "end": end_date, "items": items }))
}