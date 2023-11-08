use crate::calc::{
  core::{calc_altitude_tuple, ecliptic_to_equatorial_tuple},
  math_funcs::{recalc_houses_by_system, subtract_360},
};
use super::geo_pos::*;
use libswe_sys::swerust::handler_swe14::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct AscMc {
  pub ascendant: f64,
  pub mc: f64,
  pub armc: f64,
  pub vertex: f64,
  pub equasc: f64, // "equatorial ascendant" *
  pub coasc1: f64, // "co-ascendant" (W. Koch) *
  pub coasc2: f64, // "co-ascendant" (M. Munkasey) *
  pub polasc: f64,
  #[serde(rename = "ascAzi", skip_serializing_if = "Option::is_none")]
  pub asc_azi: Option<f64>,
  #[serde(rename = "ascRa", skip_serializing_if = "Option::is_none")]
  pub asc_ra: Option<f64>,
  #[serde(rename = "ascDec", skip_serializing_if = "Option::is_none")]
  pub asc_dec: Option<f64>,
  #[serde(rename = "mcAlt", skip_serializing_if = "Option::is_none")]
  pub mc_alt: Option<f64>,
  #[serde(rename = "mcAzi", skip_serializing_if = "Option::is_none")]
  pub mc_azi: Option<f64>,
  #[serde(rename = "mcRa", skip_serializing_if = "Option::is_none")]
  pub mc_ra: Option<f64>,
  #[serde(rename = "mcDec", skip_serializing_if = "Option::is_none")]
  pub mc_dec: Option<f64>,
}

impl AscMc {
  pub fn new(points: [f64; 10]) -> AscMc {
    AscMc {
      ascendant: points[0],
      mc: points[1],
      armc: points[2],
      vertex: points[3],
      equasc: points[4],
      coasc1: points[5],
      coasc2: points[6],
      polasc: points[7],
      asc_azi: None,
      asc_ra: None,
      asc_dec: None,
      mc_alt: None,
      mc_azi: None,
      mc_ra: None,
      mc_dec: None,
    }
  }

  pub fn new_extended(
    points: [f64; 10],
    asc_azi: Option<f64>,
    asc_ra: Option<f64>,
    asc_dec: Option<f64>,
    mc_alt: Option<f64>,
    mc_azi: Option<f64>,
    mc_ra: Option<f64>,
    mc_dec: Option<f64>,
  ) -> AscMc {
    AscMc {
      ascendant: points[0],
      mc: points[1],
      armc: points[2],
      vertex: points[3],
      equasc: points[4],
      coasc1: points[5],
      coasc2: points[6],
      polasc: points[7],
      asc_azi,
      asc_ra,
      asc_dec,
      mc_alt,
      mc_azi,
      mc_ra,
      mc_dec,
    }
  }

  pub fn apply_ayanamsha(&mut self, aya_offset: f64) {
    self.ascendant = subtract_360(self.ascendant, aya_offset);
    self.mc = subtract_360(self.mc, aya_offset);
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HouseData {
  pub jd: f64,
  pub lat: f64,
  pub lng: f64,
  pub system: char,
  pub houses: Vec<f64>,
  pub points: AscMc,
}

impl HouseData {
  pub fn new(jd: f64, lat: f64, lng: f64, system: char, calc_extended: bool) -> HouseData {
    let hd = houses(jd, lat, lng, system);
    let houses: Vec<f64> = match system {
      'G' => hd.cusps[1..37].to_vec(),
      _ => hd.cusps[1..13].to_vec(),
    };
    let add_asc_mc_coords = calc_extended && hd.ascmc.len() > 0;
    let (mc_alt, mc_azi) = match add_asc_mc_coords {
      true => calc_altitude_tuple(jd, false, lat, lng, hd.ascmc[1], 0f64),
      _ => (None, None),
    };
    let (_, asc_azi) = match add_asc_mc_coords {
      true => calc_altitude_tuple(jd, false, lat, lng, hd.ascmc[0], 0f64),
      _ => (None, None),
    };
    let (asc_ra, asc_dec) = match add_asc_mc_coords {
      true => ecliptic_to_equatorial_tuple(jd, hd.ascmc[0], 0f64),
      _ => (None, None),
    };
    let (mc_ra, mc_dec) = match add_asc_mc_coords {
      true => ecliptic_to_equatorial_tuple(jd, hd.ascmc[1], 0f64),
      _ => (None, None),
    };
    HouseData {
      jd: jd,
      lng: lng,
      lat: lat,
      system: system,
      houses,
      points: AscMc::new_extended(
        hd.ascmc, asc_azi, asc_ra, asc_dec, mc_alt, mc_azi, mc_ra, mc_dec,
      ),
    }
  }


}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HouseSet {
  pub system: char,
  pub houses: Vec<f64>,
}

impl HouseSet {
  pub fn new(system: char, houses: Vec<f64>) -> HouseSet {
    HouseSet { system, houses }
  }

/*   pub fn recalc_houses(&self, aya_offset: f64) -> Vec<f64> {
    self.houses.clone().into_iter().map(|deg| subtract_360(deg, aya_offset)).collect::<Vec<f64>>()
  } */
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HouseSetData {
  pub points: AscMc,
  pub sets: Vec<HouseSet>,
}

impl HouseSetData {
  pub fn new(mut points: AscMc, sets: Vec<HouseSet>, aya_offset: f64) -> HouseSetData {
    points.apply_ayanamsha(aya_offset);
    HouseSetData { points, sets }
  }
}

pub fn get_ascendant(jd: f64, lat: f64, lng: f64) -> f64 {
  let hd = houses(jd, lat, lng, 'W');
  hd.ascmc[0]
}

pub fn calc_ascendant(jd: f64, geo: GeoPos) -> f64 {
  get_ascendant(jd, geo.lat, geo.lng)
}

pub fn get_house_data(jd: f64, lat: f64, lng: f64, system: char, calc_extended: bool) -> HouseData {
  HouseData::new(jd, lat, lng, system, calc_extended)
}

pub fn houses_system_chars() -> Vec<char> {
  vec![
    'W', 'E', 'O', 'P', 'K', 'B', 'C', 'M', 'R', 'T', 'A', 'X', 'G', 'H',
  ]
}

pub fn match_house_systems_chars(ref_str: String) -> Vec<char> {
  let ref_chars: Vec<char> = ref_str
    .split(",")
    .filter(|s| s.len() > 0)
    .map(|c| c.to_uppercase().chars().nth(0).unwrap())
    .collect();
  let all_chars = houses_system_chars();
  ref_chars
    .iter()
    .filter(|c| all_chars.contains(c))
    .map(|c| *c)
    .collect::<Vec<char>>()
}

pub fn get_house_systems(jd: f64, geo: GeoPos, keys: Vec<char>, aya_offset: f64) -> HouseSetData {
  let house_systems: Vec<char> = houses_system_chars();
  let match_all = keys.len() == 1 && keys[0] == 'a';
  let match_whole_only = keys.len() == 1 && keys[0] == 'W' || keys.len() < 1;
  let matched_keys = if match_whole_only { vec!['W'] } else { keys };
  let mut points: AscMc = AscMc::new([0f64; 10]);
  let mut points_matched = false;
  let mut sets: Vec<HouseSet> = Vec::new();
  for key in house_systems {
    let add_extended_asc_mc_values = !points_matched;
    let hd = get_house_data(jd, geo.lat, geo.lng, key, add_extended_asc_mc_values);
    if match_all || matched_keys.contains(&key) {
      if !points_matched {
        points = hd.points;
        points_matched = true;
      }
      let house_lngs = if aya_offset == 0f64 {
        hd.houses
      } else {
        recalc_houses_by_system(hd.houses, aya_offset, key)
      };
      sets.push(HouseSet::new(key, house_lngs))
    }
  }
  HouseSetData::new(points, sets, aya_offset)
}

pub fn get_all_house_systems(jd: f64, geo: GeoPos, aya_offset: f64) -> HouseSetData {
  get_house_systems(jd, geo, vec!['a'], aya_offset)
}
