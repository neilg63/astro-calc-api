use serde::Deserialize;
use actix_web::web::Query;
use crate::lib::{utils::converters::loc_string_to_geo, models::geo_pos::GeoPos};
use super::lib::{models::date_info::DateInfo, julian_date::current_datetime_string};

#[derive(Deserialize)]
pub struct InputOptions {
  pub dt: Option<String>, // primary UTC date string
  pub dtl: Option<String>, // primary date string in local time (requires offset)
  pub jd: Option<f64>, // primary jd as a float
  pub dt2: Option<String>, // secondary UTC date string 
  pub dtl2: Option<String>, // secondary date string in local time (requires offset)
  pub jd2: Option<f64>, // secondary jd as a float
  pub bodies: Option<String>, // either a comma separated list of required 2-letter celestial body keys or body group keys
  pub trbs: Option<String>, // body keys for transitions where shown. If not specified, the body keys above will be used
  pub topo: Option<u8>, // 0 = geocentric, 1 topocentric, 2 both, default 0
  pub eq: Option<u8>, // 0 = ecliptic, 1 equatorial, 2 both, both 3 with altitude/azimuth, 4 with inline planetary phenomena
  pub ph: Option<u8>, // 0 = none (except via eq=4 in /chart-data), 1 = show pheno(nema) as separate array
  pub days: Option<u16>, // duration in days where applicable
  pub pd: Option<u8>, // number per day or per specified multiple of days (dspan), 2 => every 12 hours
  pub dspan: Option<u16>, // multiple of days in the current base unit 
  pub years: Option<u16>, // duration in years where applicable
  pub loc: Option<String>, // comma-separated lat,lng(,alt) numeric string
  pub body: Option<String>, // primary celestial body key
  pub ct: Option<u8>, // show contemporary rise/set times 
  pub sp: Option<u8>, // show sun-up/sun-down period with sun lat/long at sunrise/sunset
  pub aya: Option<String>, // comma-separated list ayanamshas to be calculated. The first may be applied to ecliptic longitudes via sid=1
  //pub amode: Option<String>, // apply referenced sidereal type (ayanamsha) to all longitudes
  pub sid: Option<u8>, // 0 tropical longitudes, 1 sidereal longitudes of first reference ayanamsha (via aya)
  pub hsys: Option<String>, // comma-separated list of letters representing house systems to be returned. Defaults to W for whole house system
  pub retro: Option<u8>, // show planet stations (retrograde, peak), 0 no, 1 yes
  pub iso: Option<u8>, // 0 show JD, 1 show ISO UTC
  //pub offset: Option<i32>, // offset is seconds from UTC
  pub tzs: Option<i32>, // offset in seconds from UTC
  pub daytime: Option<u8>, // 1 use daytime variants, 0 use night-time variants
  pub num: Option<u32>, // integer number
  pub orb: Option<f64>, // reference degree two
  pub mode: Option<u8>, // response mode, depends on endpoint
}

pub fn to_ayanamsha_keys(params: &Query<InputOptions>, def_val: &str) -> (Vec<String>, String, String) {
  let aya: String = params.aya.clone().unwrap_or(def_val.to_string());
  let aya_keys: Vec<String> = match aya.as_str() {
    "all" => vec![],
    "core" => vec!["true_citra", "lahiri", "krishnamurti"],
    _ => if aya.len() > 1 { aya.as_str().split(",").collect() } else { vec![] },
  }.into_iter().map(|k| k.to_owned()).collect();

  let first = if aya_keys.clone().len() < 1 { "-" .to_string()} else { aya_keys.first().unwrap().to_string() };
  let mode = match aya.as_str() {
    "all"  | "core" => aya,
    _ => "keys".to_string(),
  };
  (aya_keys, mode, first)
}

pub fn to_date_object_by_num(params: &Query<InputOptions>, num: u8) -> DateInfo {
  let jd = match num {
    2 => params.jd2.clone().unwrap_or(0f64),
    _ => params.jd.clone().unwrap_or(0f64)
  };
  if jd > 1_000_000f64 {
    DateInfo::new_from_jd(jd)
  } else {
    let dateref: String = match num {
      2 => params.dt2.clone().unwrap_or(current_datetime_string()),
      _ => params.dt.clone().unwrap_or(current_datetime_string()),
    };
    DateInfo::new(dateref.to_string().as_str())
  }
}

pub fn to_date_object(params: &Query<InputOptions>) -> DateInfo {
  to_date_object_by_num(&params, 1)
}

pub fn to_geopos_object(params: &Query<InputOptions>) -> GeoPos {
  let loc: String = params.loc.clone().unwrap_or("0,0".to_string());
  if let Some(geo_pos) = loc_string_to_geo(loc.as_str()) {
    geo_pos
  } else {
    GeoPos::zero()
  }
}
