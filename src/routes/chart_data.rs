use crate::calc::math_funcs::subtract_360;
use crate::calc::settings::{ayanamshas::match_ayanamsha_num, house_systems::houses_as_key_map};
use crate::calc::{
  core::*,
  models::{date_info::*, general::*, geo_pos::*, graha_pos::*, houses::*},
  planet_stations::{match_all_nextprev_planet_stations, BodySpeedSet},
  settings::ayanamshas::match_ayanamsha_key,
  rise_set_phases::*,
  utils::converters::*,
};
use crate::query_params::*;
use crate::reset_ephemeris_path;
use actix_web::{
  get,
  web::{Json, Query},
  Responder,
};
use serde::{Deserialize, Serialize};
use serde_json::*;
use std::collections::HashMap;
use std::{thread, time};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChartDataResult {
  valid: bool,
  date: DateInfo,
  geo: GeoPos,
  bodies: FlexiBodyPos,
  #[serde(rename = "topoVariants", skip_serializing_if = "Vec::is_empty")]
  topo_variants: Vec<LngLatKey>,
  house: HouseSetData,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  ayanamshas: Vec<KeyNumIdValue>,
  #[serde(rename = "riseSets", skip_serializing_if = "Vec::is_empty")]
  rise_sets: Vec<KeyFlexiValueSet>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pheno: Vec<PhenoItem>,
  #[serde(rename = "planetStations", skip_serializing_if = "Vec::is_empty")]
  planet_stations: Vec<BodySpeedSet>,
  #[serde(rename = "sunPositions", skip_serializing_if = "Vec::is_empty")]
  sun_positions: Vec<KeyNumValue>,
  #[serde(rename = "sunPeriod", skip_serializing_if = "Option::is_none")]
  sun_period: Option<SunPeriod>,
}

#[get("/positions")]
async fn body_positions(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(20);
  let date = to_date_object(&params);
  let geo = to_geopos_object(&params);
  let aya: String = params.aya.clone().unwrap_or("tropical".to_string());
  let sidereal: bool = params.sid.unwrap_or(0) > 0;
  let topo: u8 = params.topo.clone().unwrap_or(0);
  let def_keys = vec![
    "su", "mo", "ma", "me", "ju", "ve", "sa", "ur", "ne", "pl", "ra", "ke",
  ];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let eq: u8 = params.eq.clone().unwrap_or(2); // 0 ecliptic, 1 equatorial, 2 both
  let aya_key = match_ayanamsha_key(aya.as_str());
  let ayanamsha = get_ayanamsha_value(date.jd, aya.as_str());
  let aya_offset = if sidereal { ayanamsha } else { 0f64 };
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let longitudes = get_body_longitudes_contextual(date.jd, geo, eq, topo, aya_offset, &to_str_refs(&keys));
  let valid = longitudes.len() > 0;
  let sun_rise_sets = calc_transition_sun(date.jd, geo, true, mode).to_value_set(iso_mode);
  let moon_rise_sets = calc_transition_moon(date.jd, geo, true, mode).to_value_set(iso_mode);
  let coord_system = build_coord_system_label(eq, topo > 0);

  thread::sleep(micro_interval);
  Json(
    json!({ "valid": valid, "date": date, "geo": geo, "longitudes": longitudes, "ayanamsha": { "key": aya_key, "value": ayanamsha, "applied": sidereal }, "coordinateSystem": coord_system, "sunRiseSets": sun_rise_sets, "moonRiseSets": moon_rise_sets }),
  )
}


#[get("/ascendant")]
async fn ascendant_progress(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(20);

  let pd = params.pd.unwrap_or(24);
  let days_val = params.days.unwrap_or(1);
  let num_days = if days_val < 1 {
    1
  } else if days_val > 366 {
    366
  } else {
    days_val
  };
  let day_span = num_days as f64;
  let date = to_date_object(&params);
  let start_jd = date.jd - 0.5;
  let end_jd = start_jd + day_span;
  let start = DateInfo::new_from_jd(start_jd);
  let end = DateInfo::new_from_jd(end_jd);
  let geo = to_geopos_object(&params);
  let aya: String = params.aya.clone().unwrap_or("tropical".to_string());
  let sidereal: bool = params.sid.unwrap_or(0) > 0;
  let aya_key = match_ayanamsha_key(aya.as_str());
  
  let ayanamsha = get_ayanamsha_value(date.jd, aya.as_str());
  let show_aya = !aya_key.contains("tropical") && !sidereal;
  let aya_offset = if sidereal { ayanamsha } else { 0f64 };
  let num_items = pd as usize * num_days as usize;
  let increment = 1f64 / pd as f64;
  let zero_tolerance = increment / 32.0;
  let mut current_index = -1;
  let mut has_bodies = false;
  let mut result: HashMap<&str, Value> = HashMap::new();
  let mut mode_key = "none".to_string();
  let mut valid = false;
  let mut sun_moon_angle:Option<(f64, bool, u8)> = None;
  let interval = json!({
    "time": time_interval_format(increment),
    "days": increment
  });
  if let Some(key_string) = params.bodies.clone() {
    let keys = body_keys_str_to_keys_or(key_string, vec![]);
    has_bodies = keys.len() > 0;
    let eq = params.eq.unwrap_or(0);
    let topo = params.topo.unwrap_or(0);
    let mut positions: Vec<HashMap<String,f64>> = Vec::new();
    if has_bodies {
      let key_refs = to_str_refs(&keys);
      let has_sun_and_moon = key_refs.contains(&"su") && key_refs.contains(&"mo");
      for i in 0..num_items {
        let ref_jd = start_jd + (increment * i as f64);
        let body_set = get_body_longitudes_contextual(ref_jd, geo, eq, topo, aya_offset, &key_refs);
        if current_index < 0 && (ref_jd - date.jd).abs() < zero_tolerance {
          current_index = i as i32;
          if has_sun_and_moon {
            if let Some(moon_lng) = body_set.get("mo") {
              if let Some(sun_lng) = body_set.get("su") {
                sun_moon_angle = Some(calc_sun_moon_angle(*moon_lng, *sun_lng));
              }
            }
          }
        }
        positions.push(body_set);
      }
      result.insert("values", json!(positions));
      valid = positions.len() >= pd as usize;
      mode_key = build_coord_system_label(eq, topo > 0);
    }
  }
  if !has_bodies {
    let mut items: Vec<f64> = Vec::with_capacity(num_items);
    for i in 0..num_items {
      let ref_jd = start_jd + (increment * i as f64);
      if current_index < 0 && (ref_jd - date.jd).abs() < zero_tolerance {
        current_index = i as i32;
      }
      let asc_val = calc_ascendant(ref_jd, geo);
      let asc_adjusted = subtract_360(asc_val, aya_offset);
      items.push(asc_adjusted);
    }
    result.insert("values", json!(items));
    valid = items.len() >= pd as usize;
    mode_key = "ascendants".to_string();
  }
  // let mut result = json!({ "valid": valid, "mode": mode_key, "date": date, "start": start, "end": end, "interval": interval, "current_index": current_index , "geo": geo, "values": values, "ayanamsha": { "key": aya_key, "value": ayanamsha, "applied": sidereal } });
  result.insert("valid", Value::Bool(valid));
  result.insert("mode", Value::String(mode_key));
  result.insert("geo", json!(geo));
  result.insert("date", json!(date));
  result.insert("start", json!(start));
  result.insert("end", json!(end));
  result.insert("interval", json!(interval));
  result.insert("current_index", json!(current_index));
  if show_aya {
    result.insert("ayanamsha", json!({ "key": aya_key, "value": ayanamsha, "applied": sidereal }));
  }
  let show_sun_rise_sets = params.full.unwrap_or(0) > 0 || params.ct.unwrap_or(0) > 0;
  if show_sun_rise_sets {
    let iso_mode = params.iso.unwrap_or(0) > 0;
    let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
    let sun_transitions_jd = calc_transitions_sun(date.jd, num_days, geo, mode);
    let sun_transitions: Vec<FlexiValue> = sun_transitions_jd.iter().filter(|item| item.value != 0f64).map(|item| item.as_flexi_value(iso_mode)).collect();
    if sun_transitions.len() > 0 {
      result.insert("sunRiseSets", json!(sun_transitions));
    }
  }

  if has_bodies {
    if let Some((angle, waxing, phase)) = sun_moon_angle {
      result.insert("moon", json!({ "sunAngle": angle, "waxing": waxing, "phase": phase }));
    }
  }
  thread::sleep(micro_interval);
  Json(json!(result))
}

/*
* Body lng/lat positions, rise/set phases, houses, ayanamsha + optionally special degrees, upgrahas and sunrise/set longitudes and special Indian astrology variant data.
*/
#[get("/chart-data")]
pub async fn chart_data_flexi(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(50);
  let date = to_date_object(&params);
  let geo = to_geopos_object(&params);
  let show_rise_sets: bool = params.ct.clone().unwrap_or(0) > 0;
  let (aya_keys, aya_mode, aya) = to_ayanamsha_keys(&params, "tropical");
  let hsys_str = params.hsys.clone().unwrap_or("W".to_string());
  let match_all_houses = hsys_str.to_lowercase().as_str() == "all";
  let h_systems: Vec<char> = if match_all_houses {
    vec![]
  } else {
    match_house_systems_chars(hsys_str)
  };
  let topo: u8 = params.topo.clone().unwrap_or(0);
  let show_sun_period: bool = params.sp.clone().unwrap_or(0) > 0;
  let eq: u8 = params.eq.clone().unwrap_or(2); // 0 ecliptic, 1 equatorial, 2 both
  let show_pheno_inline = eq == 4;
  let show_pheno_below = !show_pheno_inline && params.ph.clone().unwrap_or(0) > 0;
  let show_planet_stations = params.retro.clone().unwrap_or(0) > 0;
  let def_keys = vec!["su", "mo", "ma", "me", "ju", "ve", "sa", "ur", "ne", "pl"];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let sidereal: bool = params.sid.unwrap_or(0) > 0;
  let mode = TransitionMode::from_u8(params.mode.unwrap_or(3));
  let ayanamsha = get_ayanamsha_value(date.jd, aya.as_str());
  let aya_offset = if sidereal { ayanamsha } else { 0f64 };
  let data = match topo {
    1 => match eq {
      0 => get_bodies_ecl_topo(date.jd, &to_str_refs(&keys), geo, aya_offset),
      1 => get_bodies_eq_topo(date.jd, &to_str_refs(&keys), geo),
      _ => get_bodies_dual_topo(
        date.jd,
        to_str_refs(&keys),
        geo,
        show_pheno_inline,
        aya_offset,
      ),
    },
    _ => match eq {
      0 => get_bodies_ecl_geo(date.jd, &to_str_refs(&keys), aya_offset),
      1 => get_bodies_eq_geo(date.jd, &to_str_refs(&keys)),
      _ => get_bodies_dual_geo(date.jd, &to_str_refs(&keys), show_pheno_inline, Some(geo), aya_offset),
    },
  };
  let pheno_items = if show_pheno_below {
    get_pheno_results(date.jd, to_str_refs(&keys))
  } else {
    vec![]
  };
  let mut topo_variants: Vec<LngLatKey> = Vec::new();
  if topo == 2 {
    topo_variants = get_bodies_ecl_topo(date.jd, &to_str_refs(&keys), geo, aya_offset)
      .into_iter()
      .map(|b| b.to_lng_lat_key())
      .collect();
  }
  let valid = data.len() > 0;
  let aya_offset_val = match eq {
    1 => 0f64,
    _ => aya_offset,
  };
  let house = if match_all_houses {
    get_all_house_systems(date.jd, geo, aya_offset_val)
  } else {
    get_house_systems(date.jd, geo, h_systems, aya_offset_val)
  };
  let ayanamshas = match aya_mode.as_str() {
    "all" => get_all_ayanamsha_values(date.jd),
    _ => get_ayanamsha_values(date.jd, to_str_refs(&aya_keys)),
  };
  let rise_set_jds: Vec<KeyNumValueSet> = if show_rise_sets {
    let tr_keys_string = params.trbs.clone().unwrap_or("".to_owned());
    let tr_keys = if tr_keys_string.len() > 1 { body_keys_str_to_keys_or(tr_keys_string, vec![]) } else { keys.clone() };
    get_transition_sets(date.jd, to_str_refs(&tr_keys), geo, mode)
  } else {
    Vec::new()
  };
  let rise_sets: Vec<KeyFlexiValueSet> = rise_set_jds
    .iter()
    .map(|item| item.as_flexi_values(iso_mode))
    .collect();

  let bodies: FlexiBodyPos = match eq {
    0 => FlexiBodyPos::Simple(data.clone().iter().map(|b| b.to_body(CoordinateSystem::Ecliptic)).collect()),
    1 => FlexiBodyPos::Simple(data.clone().iter().map(|b| b.to_body(CoordinateSystem::Equatorial)).collect()),
    _ => FlexiBodyPos::Extended(data.clone()),
  };
  thread::sleep(micro_interval);
  let pl_keys = vec!["ma", "me", "ju", "ve", "sa", "ur", "ne", "pl"];
  let station_keys: Vec<&str> = keys
    .iter()
    .filter(|k| pl_keys.contains(&k.as_str()))
    .map(|k| k.as_str())
    .collect();
  let planet_stations = if show_planet_stations {
    match_all_nextprev_planet_stations(date.jd, station_keys, iso_mode)
  } else {
    vec![]
  };
  let sun_positions = if show_sun_period { calc_sun_positions(&rise_set_jds, aya_offset)} else { vec![] };
  let sun_period = if show_sun_period { Some(calc_sun_period(&rise_set_jds, date.jd)) } else { None };
  
  Json(json!(ChartDataResult {
    valid,
    date,
    geo,
    bodies,
    topo_variants,
    house,
    ayanamshas,
    rise_sets,
    pheno: pheno_items,
    planet_stations,
    sun_positions,
    sun_period,
  }))
}

#[get("/houses")]
pub async fn show_house_systems(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let micro_interval = time::Duration::from_millis(50);
  let date = to_date_object(&params);
  let aya: String = params.aya.clone().unwrap_or("tropical".to_string());
  let aya_key = match_ayanamsha_key(&aya);
  let geo = to_geopos_object(&params);
  
  let hsys_str = params.hsys.clone().unwrap_or("W".to_string());
  let match_all_houses = hsys_str.to_lowercase().as_str() == "all";
  let h_systems: Vec<char> = if match_all_houses {
    vec![]
  } else {
    match_house_systems_chars(hsys_str)
  };
  let aya_offset_val = get_ayanamsha_value(date.jd, &aya_key);
  let house = if match_all_houses {
    get_all_house_systems(date.jd, geo, aya_offset_val)
  } else {
    get_house_systems(date.jd, geo, h_systems, aya_offset_val)
  };
  let ayanamsha = KeyNumIdValue::new(&aya, match_ayanamsha_num(&aya_key), aya_offset_val);
  let systems = houses_as_key_map();
  let valid = house.sets.len() > 0;
  thread::sleep(micro_interval);
  Json(json!({ "valid": valid, "date": date, "geo": geo, "ayanamsha": ayanamsha, "houseSets": house, "systems": systems }))
}

#[get("/progress")]
async fn bodies_progress(params: Query<InputOptions>) -> impl Responder {
  reset_ephemeris_path();
  let date = to_date_object(&params);
  let geo = to_geopos_object(&params);
  let def_keys = vec![
    "su", "mo", "ma", "me", "ju", "ve", "sa", "ur", "ne", "pl", "ke",
  ];
  let key_string: String = params.bodies.clone().unwrap_or("".to_string());
  let topo: bool = params.topo.clone().unwrap_or(0) > 0;
  let cs: u8 = params.eq.clone().unwrap_or(0); // 0 ecliptic, 1 equatorial, 3 horizontal
  let horizontal_mode = cs == 3;
  // let eq: bool = cs > 0; // 0 ecliptic, 1 equatorial, 3 horizontal
  let iso_mode: bool = params.iso.clone().unwrap_or(0) > 0;
  let days: u16 = params.days.unwrap_or(28);
  let per_day = params.pd.clone().unwrap_or(1);
  let day_span = params.dspan.clone().unwrap_or(1);
  let per_day_f64 = if per_day > 1 && per_day < 24 && day_span < 2 {
    per_day as f64
  } else if day_span > 1 && per_day > 0 {
    per_day as f64 / day_span as f64
  } else {
    2f64
  };
  let num_samples = (days as f64 * per_day_f64) as u16;
  let days_spanned = if num_samples > 1000 {
    (1000f64 / per_day_f64) as u16
  } else {
    days
  };
  let micro_interval = time::Duration::from_millis(20 + (num_samples / 4) as u64);
  let keys = body_keys_str_to_keys_or(key_string, def_keys);
  let geo_opt = if topo || horizontal_mode { Some(geo) } else { None };
  let (aya_keys, aya_mode, aya) = to_ayanamsha_keys(&params, "");
  let sidereal: bool = params.sid.unwrap_or(0) > 0;
  let ayanamsha = get_ayanamsha_value(date.jd, aya.as_str());
  let aya_offset = if sidereal { ayanamsha } else { 0f64 };
  let ayanamshas = match aya_mode.as_str() {
    "all" => get_all_ayanamsha_values(date.jd),
    _ => get_ayanamsha_values(date.jd, to_str_refs(&aya_keys)),
  };
  let data = calc_bodies_positions_jd(
    date.jd,
    &to_str_refs(&keys),
    days_spanned,
    per_day_f64,
    geo_opt,
    cs,
    iso_mode,
    aya_offset,
  );
  let frequency = if per_day_f64 < 1f64 {
    format!("{} days", day_span)
  } else {
    format!("{} per day", per_day_f64)
  };
  let coord_system = build_coord_system_label(cs, topo);
  thread::sleep(micro_interval);
  Json(json!(
    json!({ "date": date, "geo": geo, "items": data, "num_samples": num_samples, "days": days, "frequency": frequency, "coordinateSystem": coord_system, "ayanamshas": ayanamshas })
  ))
}

/**
 * Build simple coordinate system / topocentrice key for the API response
 */
fn build_coord_system_label(cs: u8, topo: bool) -> String {
  let eq_label = match cs {
    1 => "equatorial",
    2 => "ecliptic,equatorial",
    3 => "horizontal",
    _ => "ecliptic",
  };
  let topo_label = match topo {
    true => "topocentric",
    _ => "geocentric",
  };
  format!("{}/{}", eq_label, topo_label)
}
