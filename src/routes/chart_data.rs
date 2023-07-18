use crate::lib::settings::{ayanamshas::match_ayanamsha_num, house_systems::houses_as_key_map};
use crate::lib::{
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
  let longitudes = match eq {
    1 => match topo {
      1 => get_body_longitudes_eq_topo(date.jd, geo, aya_offset, &to_str_refs(&keys)),
      _ => get_body_longitudes_eq_geo(date.jd, geo, aya_offset, &to_str_refs(&keys)),
    },
    _ => match topo {
      1 => get_body_longitudes_topo(date.jd, geo, aya_offset, &to_str_refs(&keys)),
      _ => get_body_longitudes_geo(date.jd, geo, aya_offset, &to_str_refs(&keys)),
    },
  };
  let valid = longitudes.len() > 0;
  let sun_rise_sets = calc_transition_sun(date.jd, geo, true).to_value_set(iso_mode);
  let moon_rise_sets = calc_transition_moon(date.jd, geo, true).to_value_set(iso_mode);
  let coord_system = build_coord_system_label(eq, topo > 0);
  thread::sleep(micro_interval);
  Json(
    json!({ "valid": valid, "date": date, "geo": geo, "longitudes": longitudes, "ayanamsha": { "key": aya_key, "value": ayanamsha, "applied": sidereal }, "coordinateSystem": coord_system, "sunRiseSets": sun_rise_sets, "moonRiseSets": moon_rise_sets }),
  )
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
    get_transition_sets(date.jd, to_str_refs(&tr_keys), geo)
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
