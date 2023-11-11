use std::{thread, time};
use serde_json::*;
use actix_web::{get, Responder,web::{Query, Json}};
use crate::{query_params::*, reset_ephemeris_path};
use crate::calc::{
  traits::FromKey,
  rise_set_phases::*,
  transposed_transitions::{calc_transposed_graha_transitions_from_source_refs_topo, calc_transposed_graha_transitions_from_source_refs_geo },
  models::{geo_pos::*, general::*},
  utils::converters::*
};
use libswe_sys::sweconst::Bodies;

// temp name transitions
#[get("/rise-set-times")]
async fn list_rise_set_times(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(30);
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let date = to_date_object(&params);
  let def_keys = vec!["su", "mo", "ma", "me", "ju", "ve", "sa"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let days_int = params.days.unwrap_or(1u16);
  let num_days = if days_int >= 1 { days_int } else { 1u16 };
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let transition_sets_jd = get_transition_sets_extended(date.jd, keys, geo, num_days, mode);
  let valid = transition_sets_jd.len() > 0;
  let transit_sets = FlexiValueSet::FlexiValues(transition_sets_jd.iter().map(|vs| vs.as_flexi_values(iso_mode)).collect());
  thread::sleep(micro_interval);
  Json(json!({ "valid": valid, "date": date, "geo": geo, "sets": transit_sets }))
}

#[get("/sun-rise-set-times")]
async fn list_sun_rise_set_times(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(30);  
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let date = to_date_object(&params);
  let days: u16 = params.days.unwrap_or(28);
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let extended_set_mode = params.full.unwrap_or(0) > 0;
  let json_output =  if extended_set_mode {
    let sun_rise_sets_jd = calc_transition_sets_sun(date.jd, days, geo, mode);
    let sun_rise_sets: Vec<AltTransitionValueSet> = sun_rise_sets_jd.iter().map(|item| item.to_value_set(iso_mode)).collect();
    json!({ "valid": sun_rise_sets.len() > 0, "date": date, "geo": geo, "sets": sun_rise_sets })
  } else {
    let sun_transitions_jd = calc_transitions_sun(date.jd, days, geo, mode);
    let sun_transitions: Vec<FlexiValue> = sun_transitions_jd.iter().filter(|item| item.value != 0f64).map(|item| item.as_flexi_value(iso_mode)).collect();
    json!({ "valid": sun_transitions.len() > 0, "date": date, "geo": geo, "items": sun_transitions })
  };
  thread::sleep(micro_interval);
  Json(json_output)
}

#[get("/pheno")]
async fn pheno_data(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let def_keys = vec!["su", "mo", "ma", "me", "ju", "ve", "sa"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let date = to_date_object(&params);
  let items =  get_pheno_results(date.jd, to_str_refs(&keys));
  let valid = items.len() > 0;
  Json(json!({ "valid": valid, "date": date, "result": items }))
}

#[get("/transposed-rise-times")]
async fn body_transposed_transitions_range(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(50);
  let historic_dt = to_date_object_2(&params);
  let current_dt = to_date_object(&params);
  let loc: String = params.loc2.clone().unwrap_or("0,0".to_string());
  let historic_geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let current_loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let current_geo = if let Some(geo_pos) = loc_string_to_geo(current_loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let show_transitions: bool = params.ct.clone().unwrap_or(0) > 0;
  let def_keys = vec!["su", "mo", "ma", "me", "ju", "ve", "sa"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let days_int = params.days.unwrap_or(1u16);
  let num_days = if days_int >= 1 { days_int } else { 1u16 };
  let iso_mode = params.iso.unwrap_or(0) > 0;
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let rise_set_times = calc_transposed_graha_transitions_from_source_refs_geo(current_dt.jd, current_geo, historic_dt.jd, historic_geo, keys.clone(), num_days);
  let valid = rise_set_times.len() > 0;
  let current_rise_times:  Vec<KeyNumValueSet> = if show_transitions { get_transition_sets_extended(current_dt.jd, keys, current_geo, num_days, mode) } else { Vec::new() };
  let transposed = rise_set_times.into_iter().map(|row| row.as_flexi_values(iso_mode)).collect::<Vec<KeyFlexiValueSet>>();
  let current = current_rise_times.into_iter().map(|row| row.as_flexi_values(iso_mode)).collect::<Vec<KeyFlexiValueSet>>();
  thread::sleep(micro_interval);
  Json(json!({ "valid": valid, "date": current_dt, "geo": current_geo, "historicDate": historic_dt, "historicGeo": historic_geo, "days": num_days, "transposed": transposed, "current": current }))
}

#[get("/test-rise-set-times")]
async fn test_rise_set_times(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(30);
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let date = to_date_object(&params);
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let def_keys = vec!["su", "mo", "ma", "me", "ju", "ve", "sa"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let days_int = params.days.unwrap_or(1u16);
  let num_days = if days_int >= 1 { days_int } else { 1u16 };
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let transition_sets_jd = get_transition_sets_extended(date.jd, keys.clone(), geo, num_days, mode);
  let valid = transition_sets_jd.len() > 0;
  let transit_sets = FlexiValueSet::FlexiValues(transition_sets_jd.iter().map(|vs| vs.as_flexi_values(iso_mode)).collect());
  let alt_transit_sets_jd = calc_transposed_graha_transitions_from_source_refs_topo(date.jd, geo, date.jd, geo, keys.clone(), num_days);
  let alt_transit_sets = FlexiValueSet::FlexiValues(alt_transit_sets_jd.iter().map(|vs| vs.as_flexi_values(iso_mode)).collect());
  thread::sleep(micro_interval);
  Json(json!({ "valid": valid, "date": date, "geo": geo, "transitSets": transit_sets, "altTransitSets": alt_transit_sets }))
}

#[get("/test-swe-rise")]
async fn test_mcs(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(30);
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  let geo = if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) { geo_pos } else { GeoPos::zero() };
  let date = to_date_object(&params);
  let def_keys = vec!["su", "mo", "ma", "me", "ju", "ve", "sa"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let iso_mode = params.iso.unwrap_or(0) > 0;
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let mut mcs: Vec<FlexiValue> = vec![];
  let mut ics: Vec<FlexiValue> = vec![];
  let mut rises: Vec<FlexiValue> = vec![];
  let mut sets: Vec<FlexiValue> = vec![];
  let mut num_valid: usize = 0;
  for key in keys {
    let mc = next_mc(date.jd, Bodies::from_key(key.as_str()), geo.lat, geo.lng);
    mcs.push(KeyNumValue::new(key.as_str(), mc).as_flexi_value(iso_mode));
    if mc >= 0f64 { 
      num_valid += 1;
    }
    let body = Bodies::from_key(key.as_str());
    let ic = next_ic(date.jd, body, geo.lat, geo.lng);
    ics.push(KeyNumValue::new(key.as_str(), ic).as_flexi_value(iso_mode));
    let rise = next_rise(date.jd, body, geo.lat, geo.lng, mode);
    rises.push(KeyNumValue::new(key.as_str(), rise).as_flexi_value(iso_mode));
    let set = next_set(date.jd, body, geo.lat, geo.lng, mode);
    sets.push(KeyNumValue::new(key.as_str(), set).as_flexi_value(iso_mode));
  }
  let num_items = mcs.len();
  let valid = num_valid == num_items && num_items > 0;
  let desc = "Tests the native Swiss Ephemeris implementation with MC/IC and rise/set flags with and without the center disc flag. Where an object does not set or rise, the MC and IC are calculated by sampling max and min altitdues.";
  let mode_notes: [&str; 8] = [
    "0 => None / unadjusted",
    "1 => No Refraction only",
    "2 => Centre disc + no refraction",
    "3 => Centre disc only",
    "4 => Bottom disc + no refraction",
    "5 => Bottom disc only",
    "6 => Fixed disc + no refraction",
    "7 => Fixed disc only"
  ];
  let mode_usize = mode as usize;
  let mod_index = if mode_usize < 8 { mode_usize } else { 0 };
  let mode_label = format!("{}", mode_notes[mod_index]);
  let sun_rise = extract_flexi_value_string(&rises, "su");
  let sun_set = extract_flexi_value_string(&sets, "su");
  let sun_data = if let Some(sun_r) = sun_rise {
    json!({ "rise": sun_r, "set": sun_set.unwrap_or("N/A".to_owned()) })
  } else {
    json!({ "rise": "N/A", "set": "N/A" })
  };
  thread::sleep(micro_interval);
  Json(json!({ "valid": valid, "astroNotes": { "desc": desc, "modes": mode_notes, "date": date, "geo": geo, "mode": mode_label, "sun": sun_data }, "results": { "mc": mcs, "ic": ics, "rise": rises, "set": sets } }))
}