use super::super::extensions::swe::{azalt, get_ayanamsha, set_topo};
use super::math_funcs::{normalize_360, normalize_f64};
use super::models::{general::*, geo_pos::*, graha_pos::*, houses::calc_ascendant};
use super::{
  math_funcs::subtract_360,
  math_funcs::{adjust_lng_by_body_key, calc_opposite},
  settings::ayanamshas::*,
  traits::*,
  rise_set_phases::get_pheno_result,
  transposed_transitions::calc_transitions_from_source_refs_minmax,
};
use libswe_sys::sweconst::{Bodies, OptionalFlag};
use libswe_sys::swerust::handler_swe03::*;
use math::round::floor;
use std::collections::HashMap;

pub fn calc_body_jd(jd: f64, key: &str, sidereal: bool, topo: bool, aya_offset: f64) -> GrahaPos {
  let combo: i32;
  let speed_flag = OptionalFlag::Speed as i32;
  let swe_flag = OptionalFlag::SwissEph as i32;
  if topo {
    let topo_flag = OptionalFlag::TopocentricPosition as i32;
    if sidereal {
      combo = swe_flag | speed_flag | OptionalFlag::SideralPosition as i32 | topo_flag;
    } else {
      combo = swe_flag | speed_flag | topo_flag;
    }
  } else {
    if sidereal {
      combo = swe_flag | speed_flag | OptionalFlag::SideralPosition as i32;
    } else {
      combo = swe_flag | speed_flag;
    }
  }
  let result = calc_ut(jd, Bodies::from_key(key), combo);
  // only apply for ecliptic lng if the sidereal mode is not applied via SE in conjunction with set_sid_mode
  let aya_offset_val = if sidereal { 0f64 } else { aya_offset };
  let lng = subtract_360(
    adjust_lng_by_body_key(key, result.longitude),
    aya_offset_val,
  );
  GrahaPos::new(
    key,
    lng,
    result.latitude,
    result.speed_longitude,
    result.speed_latitude,
  )
}

/**
 * Only implement tropical variants for equatorial positions
 * Ayanamsha value may be subtracted if required
 */
pub fn calc_body_eq_jd_swe(jd: f64, key: &str, topo: bool) -> GrahaPos {
  let combo: i32;
  //let eq_flag = OptionalFlag::SEFLG_EQUATORIAL;
  let eq_flag = OptionalFlag::EquatorialPosition as i32;
  let swe_flag = OptionalFlag::SwissEph as i32;
  let speed_flag = OptionalFlag::Speed as i32;
  if topo {
    combo = swe_flag | speed_flag | OptionalFlag::TopocentricPosition as i32 | eq_flag;
  } else {
    combo = swe_flag | speed_flag | eq_flag;
  }
  let result = calc_ut(jd, Bodies::from_key(key), combo);
  let lng = adjust_lng_by_body_key(key, result.longitude);
  GrahaPos::new_eq(
    key,
    lng,
    result.latitude,
    result.speed_longitude,
    result.speed_latitude,
  )
}

/**
 * For Ketu fetch reversed Rahu ecliptic position and then calculate the right ascension and declination via
 * ecliptic_to_equatorial_basic
 */
pub fn calc_body_eq_jd(jd: f64, key: &str, topo: bool) -> GrahaPos {
  match key {
    "ke" => {
      let pos = calc_body_jd(jd, key, false, topo, 0f64);
      let eq_pos = ecliptic_to_equatorial_basic(jd, pos.lng, pos.lat);
      GrahaPos::new_eq(key, eq_pos.lng, eq_pos.lat, pos.lng_speed, pos.lat_speed)
    }
    _ => calc_body_eq_jd_swe(jd, key, topo),
  }
}

pub fn calc_body_dual_jd(
  jd: f64,
  key: &str,
  topo: bool,
  show_pheno: bool,
  geo_opt: Option<GeoPos>,
  aya_offset: f64,
) -> GrahaPos {
  let combo: i32;
  //let eq_flag = OptionalFlag::SEFLG_EQUATORIAL;
  let eq_flag = OptionalFlag::EquatorialPosition as i32;
  let swe_flag = OptionalFlag::SwissEph as i32;
  let speed_flag = OptionalFlag::Speed as i32;
  if topo {
    combo = swe_flag | speed_flag | OptionalFlag::TopocentricPosition as i32 | eq_flag;
  } else {
    combo = swe_flag | speed_flag | eq_flag;
  }
  let combo_geo = if topo {
    swe_flag | speed_flag | OptionalFlag::TopocentricPosition as i32
  } else {
    swe_flag | speed_flag
  };
  let result = calc_ut(jd, Bodies::from_key(key), combo);
  let result_ec = calc_ut(jd, Bodies::from_key(key), combo_geo);
  let pheno = if show_pheno {
    Some(get_pheno_result(jd, key, 0i32))
  } else {
    None
  };
  let lng = subtract_360(adjust_lng_by_body_key(key, result_ec.longitude), aya_offset);
  // let ra = adjust_lng_by_body_key(key, result.longitude);
  let (ra, dec) = adjust_ra_dec_by_body_key(
    key,
    jd,
    result.longitude,
    result.latitude,
    result_ec.longitude,
    result_ec.latitude,
  );
  let altitude_set = match geo_opt {
    Some(geo) => Some(azalt(jd, true, geo.lat, geo.lng, ra, dec)),
    _ => None,
  };
  let altitude = match altitude_set {
    Some(a_set) => Some(a_set.value),
    None => None,
  };
  let azimuth = match altitude_set {
    Some(a_set) => Some(a_set.azimuth),
    None => None,
  };
  GrahaPos::new_extended(
    key,
    lng,
    result_ec.latitude,
    ra,
    dec,
    result_ec.speed_longitude,
    result_ec.speed_latitude,
    result.speed_longitude,
    result.speed_latitude,
    pheno,
    altitude,
    azimuth,
  )
}

pub fn calc_body_hor_jd(
  jd: f64,
  key: &str,
  topo: bool,
  geo_opt: Option<GeoPos>,
) -> GrahaPos {
  let combo: i32;
  //let eq_flag = OptionalFlag::SEFLG_EQUATORIAL;
  match geo_opt {
    Some(geo) => {
      set_topo(geo.lat, geo.lng, geo.alt);
    },
    _ => ()
  };
  let eq_flag = OptionalFlag::EquatorialPosition as i32;
  let swe_flag = OptionalFlag::SwissEph as i32;
  let speed_flag = OptionalFlag::Speed as i32;
  if topo {
    combo = swe_flag | speed_flag | OptionalFlag::TopocentricPosition as i32 | eq_flag;
  } else {
    combo = swe_flag | speed_flag | eq_flag;
  }
  let combo_geo = if topo {
    swe_flag | speed_flag | OptionalFlag::TopocentricPosition as i32
  } else {
    swe_flag | speed_flag
  };
  let result = calc_ut(jd, Bodies::from_key(key), combo);
  let result_ec = calc_ut(jd, Bodies::from_key(key), combo_geo);
  
  // let ra = adjust_lng_by_body_key(key, result.longitude);
  let (ra, dec) = adjust_ra_dec_by_body_key(
    key,
    jd,
    result.longitude,
    result.latitude,
    result_ec.longitude,
    result_ec.latitude,
  );
  let altitude_set = match geo_opt {
    Some(geo) => Some(azalt(jd, true, geo.lat, geo.lng, ra, dec)),
    _ => None,
  };
  let one_hour_hence_jd = jd + 1f64 / 24f64;
  let altitude_set_2 = match geo_opt {
    Some(geo) => Some(azalt(one_hour_hence_jd, true, geo.lat, geo.lng, ra, dec)),
    _ => None,
  };
  
  let altitude = match altitude_set {
    Some(a_set) => a_set.value,
    None => 0f64,
  };
  let azimuth = match altitude_set {
    Some(a_set) => a_set.azimuth,
    None => 0f64,
  };
  let altitude_2 = match altitude_set_2 {
    Some(a_set) => a_set.value,
    None => 0f64,
  };
  let azimuth_2 = match altitude_set_2 {
    Some(a_set) => a_set.azimuth,
    None => 0f64,
  };
  let lng_speed = normalize_360(azimuth_2) - normalize_360(azimuth);
  let lat_speed = normalize_f64(altitude_2, 180) - normalize_f64(altitude, 180);
  GrahaPos::new_extended(
    key,
    result_ec.longitude,
    result.longitude,
    ra,
    dec,
    lng_speed,
    lat_speed,
    result.speed_longitude,
    result.speed_latitude,
    None,
    Some(azimuth),
    Some(altitude),
  )
}

pub fn calc_body_dual_jd_geo(jd: f64, key: &str, show_pheno: bool, geo_opt: Option<GeoPos>, aya_offset: f64) -> GrahaPos {
  calc_body_dual_jd(jd, key, false, show_pheno, geo_opt, aya_offset)
}

pub fn calc_body_dual_jd_topo(
  jd: f64,
  key: &str,
  geo: GeoPos,
  show_pheno: bool,
  aya_offset: f64,
) -> GrahaPos {
  set_topo(geo.lat, geo.lng, geo.alt);
  calc_body_dual_jd(jd, key, true, show_pheno, Some(geo), aya_offset)
}

pub fn calc_body_eq_jd_topo(jd: f64, key: &str, geo: GeoPos) -> GrahaPos {
  set_topo(geo.lat, geo.lng, geo.alt);
  calc_body_eq_jd(jd, key, true)
}

/*
 Get tropical geocentric coordinates
*/
pub fn calc_body_jd_geo(jd: f64, key: &str, aya_offset: f64) -> GrahaPos {
  calc_body_jd(jd, key, false, false, aya_offset)
}

/*
 Get set of tropical geocentric coordinates for one celestial body
*/
/* pub fn calc_body_positions_jd_geo(
  jd_start: f64,
  key: &str,
  days: i32,
  num_per_day: f64,
) -> Vec<GrahaPosItem> {
  let mut items: Vec<GrahaPosItem> = Vec::new();
  let max_f64 = floor(days as f64 * num_per_day, 0);
  let max = max_f64 as i32;
  let increment = 1f64 / num_per_day;
  for i in 0..max {
    let curr_jd = jd_start + (i as f64 * increment);
    let graha_pos = calc_body_jd_geo(curr_jd, key, 0f64);
    items.push(GrahaPosItem::new(curr_jd, graha_pos));
  }
  items
}
 */
/*
 Get set of tropical geocentric coordinates for groups of celestial bodies
*/
pub fn calc_bodies_positions_jd(
  jd_start: f64,
  keys: &Vec<&str>,
  days: u16,
  num_per_day: f64,
  geo: Option<GeoPos>,
  cs: u8,
  iso_mode: bool,
  aya_offset: f64,
) -> Vec<GrahaPosSet> {
  let mut items: Vec<GrahaPosSet> = Vec::new();
  let max_f64 = floor(days as f64 * num_per_day, 0);
  let max = max_f64 as i32;
  let increment = 1f64 / num_per_day;
  let topo = match geo {
    None => false,
    _ => true,
  };
  for i in 0..max {
    let curr_jd = jd_start + (i as f64 * increment);
    let mut bodies: Vec<GrahaPos> = Vec::new();
    for key in keys {
      let graha_pos = match cs {
        1 => match topo {
          true => calc_body_eq_jd_topo(curr_jd, key, geo.unwrap()),
          _ => calc_body_eq_jd(curr_jd, key, false),
        },
        3 => calc_body_hor_jd(curr_jd, key, topo, geo),
        _ => match topo {
          true => calc_body_jd_topo(curr_jd, key, geo.unwrap(), aya_offset),
          _ => calc_body_jd_geo(curr_jd, key, aya_offset),
        },
      };
      bodies.push(graha_pos);
    }
    let mode = match cs {
      1 => CoordinateSystem::Equatorial,
      3 => CoordinateSystem::Horizontal,
      _ => CoordinateSystem::Ecliptic,
    };
    items.push(GrahaPosSet::new(curr_jd, bodies, mode, iso_mode));
  }
  items
}

/*
 Get sidereal geocentric coordinates with an ayanamsha key
*/
/* pub fn calc_body_jd_geo_sidereal(jd: f64, key: &str, aya_key: &str) -> GrahaPos {
  set_sid_mode(Ayanamsha::from_key(aya_key).as_i32());
  calc_body_jd(jd, key, true, false, 0f64)
} */

/*
 Get tropical topocentric coordinates with geo-coordinates
*/
pub fn calc_body_jd_topo(jd: f64, key: &str, geo: GeoPos, aya_offset: f64) -> GrahaPos {
  set_topo(geo.lat, geo.lng, geo.alt);
  calc_body_jd(jd, key, false, true, aya_offset)
}

/*
 Get sidereal topocentric coordinates with geo-coordinates and an ayanamsha key
*/
/* pub fn calc_body_jd_topo_sidereal(jd: f64, key: &str, geo: GeoPos, aya_key: &str) -> GrahaPos {
  set_topo(geo.lat, geo.lng, geo.alt);
  set_sid_mode(Ayanamsha::from_key(aya_key).as_i32());
  calc_body_jd(jd, key, false, true, 0f64)
}
 */
/*
  Fetch a set of
*/
pub fn get_bodies_dual_geo(
  jd: f64,
  keys: &Vec<&str>,
  show_pheno: bool,
  geo_opt: Option<GeoPos>,
  aya_offset: f64,
) -> Vec<GrahaPos> {
  let mut bodies: Vec<GrahaPos> = Vec::new();
  for key in keys {
    let key = key.to_owned();
    let result = calc_body_dual_jd_geo(jd, key, show_pheno, geo_opt, aya_offset);
    bodies.push(result);
  }
  bodies
}

pub fn get_bodies_ecl_geo(jd: f64, keys: &Vec<&str>, aya_offset: f64) -> Vec<GrahaPos> {
  let mut bodies: Vec<GrahaPos> = Vec::new();
  for key in keys {
    let key = key.to_owned();
    let result = calc_body_jd_geo(jd, key, aya_offset);
    bodies.push(result);
  }
  bodies
}

pub fn get_bodies_eq_geo(jd: f64, keys: &Vec<&str>) -> Vec<GrahaPos> {
  let mut bodies: Vec<GrahaPos> = Vec::new();
  for key in keys {
    let key = key.to_owned();
    let result = calc_body_eq_jd(jd, key, false);
    bodies.push(result);
  }
  bodies
}

pub fn get_bodies_eq_topo(jd: f64, keys: &Vec<&str>, geo: GeoPos) -> Vec<GrahaPos> {
  let mut bodies: Vec<GrahaPos> = Vec::new();
  for key in keys {
    let key = key.to_owned();
    let result = calc_body_eq_jd_topo(jd, key, geo);
    bodies.push(result);
  }
  bodies
}

pub fn get_bodies_ecl_topo(
  jd: f64,
  keys: &Vec<&str>,
  geo: GeoPos,
  aya_offset: f64,
) -> Vec<GrahaPos> {
  let mut bodies: Vec<GrahaPos> = Vec::new();
  for key in keys {
    let key = key.to_owned();
    let result = calc_body_jd_topo(jd, key, geo, aya_offset);
    bodies.push(result);
  }
  bodies
}

pub fn get_body_longitudes(
  jd: f64,
  geo: GeoPos,
  mode: &str,
  equatorial: bool,
  aya_offset: f64,
  keys: &Vec<&str>,
) -> HashMap<String, f64> {
  let mut items: HashMap<String, f64> = HashMap::new();
  let bodies = match equatorial {
    true => match mode {
      "topo" => get_bodies_eq_topo(jd, keys, geo),
      _ => get_bodies_eq_geo(jd, keys),
    },
    _ => match mode {
      "topo" => get_bodies_ecl_topo(jd, keys, geo, aya_offset),
      _ => get_bodies_ecl_geo(jd, keys, aya_offset),
    },
  };
  let aya_offset_val = if equatorial { 0f64 } else { aya_offset };
  items.insert(
    "as".to_string(),
    subtract_360(calc_ascendant(jd, geo), aya_offset_val),
  );
  for body in bodies {
    let lng = if equatorial {
      body.rect_ascension
    } else {
      body.lng
    };
    items.insert(body.key, lng);
  }
  items
}

pub fn get_body_longitudes_geo(
  jd: f64,
  geo: GeoPos,
  aya_offset: f64,
  keys: &Vec<&str>,
) -> HashMap<String, f64> {
  get_body_longitudes(jd, geo, "geo", false, aya_offset, keys)
}

pub fn get_body_longitudes_topo(
  jd: f64,
  geo: GeoPos,
  aya_offset: f64,
  keys: &Vec<&str>,
) -> HashMap<String, f64> {
  get_body_longitudes(jd, geo, "topo", false, aya_offset, keys)
}

pub fn get_body_longitudes_eq_geo(
  jd: f64,
  geo: GeoPos,
  aya_offset: f64,
  keys: &Vec<&str>,
) -> HashMap<String, f64> {
  get_body_longitudes(jd, geo, "geo", true, aya_offset, keys)
}

pub fn get_body_longitudes_eq_topo(
  jd: f64,
  geo: GeoPos,
  aya_offset: f64,
  keys: &Vec<&str>,
) -> HashMap<String, f64> {
  get_body_longitudes(jd, geo, "topo", true, aya_offset, keys)
}

pub fn get_bodies_dual_topo(
  jd: f64,
  keys: Vec<&str>,
  geo: GeoPos,
  show_pheno: bool,
  aya_offset: f64,
) -> Vec<GrahaPos> {
  let mut bodies: Vec<GrahaPos> = Vec::new();
  for key in keys {
    let result = calc_body_dual_jd_topo(jd, key, geo, show_pheno, aya_offset);
    bodies.push(result);
  }
  bodies
}

/*
* Match the projected altitude of any celestial object
*/
pub fn calc_altitude(
  tjd_ut: f64,
  is_equal: bool,
  geo_lat: f64,
  geo_lng: f64,
  lng: f64,
  lat: f64,
) -> f64 {
  azalt(tjd_ut, is_equal, geo_lat, geo_lng, lng, lat).value
}

/*
* Match the projected altitude of any celestial object
*/
pub fn calc_altitude_tuple(
  tjd_ut: f64,
  is_equal: bool,
  geo_lat: f64,
  geo_lng: f64,
  lng: f64,
  lat: f64,
) -> (Option<f64>, Option<f64>) {
  let result = azalt(tjd_ut, is_equal, geo_lat, geo_lng, lng, lat);
  (Some(result.value), Some(result.azimuth))
}

/*
* Match the projected altitude of any celestial object
*/
pub fn calc_altitude_object(
  tjd_ut: f64,
  is_equal: bool,
  geo_lat: f64,
  geo_lng: f64,
  key: &str,
) -> f64 {
  let pos = match is_equal {
    true => calc_body_eq_jd_topo(tjd_ut, key, GeoPos::simple(geo_lat, geo_lng)),
    _ => calc_body_jd_topo(tjd_ut, key, GeoPos::simple(geo_lat, geo_lng), 0f64),
  };
  calc_altitude(tjd_ut, is_equal, geo_lat, geo_lng, pos.lng, pos.lat)
}

/*
* reconstructed from Lahiri by calculating proportional differences over 200 years. Native C implementation may be bug-prone
* on some platforms.
*/
pub fn calc_true_citra(jd: f64) -> f64 {
  let jd1 = 2422324.5f64;
  let p1 = 0.9992925739019888f64;
  let jd2 = 2458849.5f64;
  let p2 = 0.99928174751934f64;
  let jd3 = 2495373.5f64;
  let p3 = 0.9992687765534588f64;
  let diff_jd2 = jd - jd2;
  let before2020 = diff_jd2 < 0f64;
  let dist = if before2020 {
    (0f64 - diff_jd2) / (jd2 - jd1)
  } else {
    diff_jd2 / (jd3 - jd2)
  };
  let diff_p = if before2020 { p2 - p1 } else { p3 - p2 };
  let multiple = if before2020 {
    p2 - (diff_p * dist)
  } else {
    p2 + (diff_p * dist)
  };
  get_ayanamsha_value_raw(jd, "lahiri") * multiple
}

pub fn get_ayanamsha_value_raw(jd: f64, key: &str) -> f64 {
  let aya_flag = Ayanamsha::from_key(key);
  get_ayanamsha(jd, aya_flag)
}

pub fn get_ayanamsha_value(jd: f64, key: &str) -> f64 {
  let aya_flag = Ayanamsha::from_key(key);
  match aya_flag {
    Ayanamsha::Tropical => 0f64,
    Ayanamsha::TrueCitra => calc_true_citra(jd),
    _ => get_ayanamsha(jd, aya_flag),
  }
}

pub fn get_ayanamsha_values(jd: f64, keys: Vec<&str>) -> Vec<KeyNumIdValue> {
  let mut items: Vec<KeyNumIdValue> = Vec::new();
  for key in keys {
    let value = get_ayanamsha_value(jd, key);
    let num = match_ayanamsha_num(key);
    items.push(KeyNumIdValue::new(match_ayanamsha_key(key).as_str(), num, value));
  }
  items
}

pub fn get_all_ayanamsha_values(jd: f64) -> Vec<KeyNumIdValue> {
  let mut items: Vec<KeyNumIdValue> = Vec::new();
  let keys = all_ayanamsha_keys();
  for key in keys {
    let value = get_ayanamsha_value(jd, key);
    let num = match_ayanamsha_num(key);
    items.push(KeyNumIdValue::new(key, num, value));
  }
  items
}

pub fn ecliptic_obliquity(jd: f64) -> f64 {
  let epoch = 2451545f64;
  let t = (jd - epoch) / 36525f64;
  let ecl_obl = 23.439292f64 - 0.013004166666666666f64 * t - 1.6666666666666665E-7f64 * t * t
    + 5.027777777777778E-7f64 * t * t * t;
  ecl_obl * 0.017453292519943295f64
}

pub fn ecliptic_to_equatorial_basic(jd: f64, lng: f64, lat: f64) -> LngLat {
  let obliq = ecliptic_obliquity(jd);
  let rad = std::f64::consts::PI / 180f64;
  let sin_e = obliq.sin();
  let cos_e = obliq.cos();
  let sin_l = (lng * rad).sin();
  let cos_l = (lng * rad).cos();
  let sin_b = (lat * rad).sin();
  let cos_b = (lat * rad).cos();
  let tan_b = (lat * rad).tan();
  let ra = (sin_l * cos_e - tan_b * sin_e).atan2(cos_l);
  let dec = (sin_b * cos_e + cos_b * sin_e * sin_l).asin();
  return LngLat::new((ra / rad + 360f64) % 360f64, dec / rad);
}

pub fn ecliptic_to_equatorial_tuple(jd: f64, lng: f64, lat: f64) -> (Option<f64>, Option<f64>) {
  let coords = ecliptic_to_equatorial_basic(jd, lng, lat);
  (Some(coords.lng), Some(coords.lat))
}

pub fn adjust_ra_dec_by_body_key(
  key: &str,
  jd: f64,
  src_ra: f64,
  src_dec: f64,
  lng: f64,
  lat: f64,
) -> (f64, f64) {
  match key {
    "ke" => {
      if let (Some(ra), Some(dec)) = ecliptic_to_equatorial_tuple(jd, calc_opposite(lng), lat) {
        (ra, dec)
      } else {
        (lng, lat)
      }
    }
    _ => (src_ra, src_dec),
  }
}


pub fn extract_sun_rise_sets(items: &Vec<KeyNumValueSet>) -> Vec<KeyNumValue> {
  if let Some(row) = items.into_iter().find(|vs| vs.key == "su") {
    if row.items.len() > 0  {
      row.items.clone()
    } else {
      vec![]
    }
  } else {
    vec![]
  }
}

pub fn extract_sun_rise_set_jd(transitions: &Vec<KeyNumValueSet>, is_set: bool) -> Option<f64> {
  let sun_trs = extract_sun_rise_sets(&transitions);
  let tr_key = if is_set { "set" } else { "rise" };
  if let Some(row) = sun_trs.into_iter().find(|tr| tr.key == tr_key.to_owned()) {
    Some(row.value)
  } else {
    None
  }
}

pub fn calc_sun_at_sun_rise_set(items: &Vec<KeyNumValueSet>, is_set: bool, aya_val: f64) -> Option<BodyPos> {
  let ref_val = extract_sun_rise_set_jd(items, is_set);
  if let Some(tr_jd )= ref_val {
    Some(calc_body_jd(tr_jd, "su", true, false, aya_val).to_body(CoordinateSystem::Ecliptic))
  } else {
    None
  }
}

pub fn calc_sun_at_sun_rise(items: &Vec<KeyNumValueSet>, aya_val: f64) -> Option<BodyPos> {
  calc_sun_at_sun_rise_set(items,  false, aya_val)
}

pub fn calc_sun_at_sun_set(items: &Vec<KeyNumValueSet>, aya_val: f64) -> Option<BodyPos> {
  calc_sun_at_sun_rise_set(items,  true, aya_val)
}

pub fn calc_sun_positions(items: &Vec<KeyNumValueSet>, aya_val: f64) -> Vec<KeyNumValue> {
  let mut positions: Vec<KeyNumValue> = vec![];
  if let Some(body) = calc_sun_at_sun_rise(items,  aya_val) {
    positions.push(KeyNumValue::new("rise", body.lng));
  }
  if let Some(body) = calc_sun_at_sun_set(items,  aya_val) {
    positions.push(KeyNumValue::new("set", body.lng));
  }
  positions
}

pub fn calc_sun_period(rise_set_items: &Vec<KeyNumValueSet>, jd: f64) -> SunPeriod {
  let rows = extract_sun_rise_sets(rise_set_items);
  let mut items = rows.into_iter().filter(|row| row.key.contains("set") || row.key.contains("rise"))
    .collect::<Vec<KeyNumValue>>();
  items.sort_by(|a,b| a.value.partial_cmp(&b.value).unwrap());
  let before_items:Vec<KeyNumValue> = items.clone().into_iter().filter(|row| row.value <= jd).collect();
  let after_items:Vec<KeyNumValue> = items.into_iter().filter(|row| row.value > jd).collect();
  let mut start = 0f64;
  let mut night = false;
  if let Some(start_item) = before_items.last() {
    start = start_item.value;
    night = start_item.key.contains("set");
  }
  let mut end = 0f64;
  if let Some(end_item) = after_items.get(0) {
    end = end_item.value;
  }
  SunPeriod::new(jd,start,end,night)
}