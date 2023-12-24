use crate::calc::rise_set_phases::{UP_DOWN_TOLERANCE, MIN_JD};
use super::models::general::{KeyNumValue, KeyNumValueSet, CoordinateSystem};
use super::models::{geo_pos::*, graha_pos::*};
use super::{
  core::{calc_altitude, calc_body_jd_geo, calc_body_jd_topo},
  rise_set_phases::{get_pheno_result, AltTransitionSet, TransitionSet},
};
use serde::{Deserialize, Serialize};

const MINS_PER_DAY: usize = 1440;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AltitudeSample {
  pub mode: String,
  pub mins: f64,
  pub jd: f64,
  pub value: f64,
}

impl AltitudeSample {
  pub fn new(mode: &str, mins: f64, jd: f64, value: f64) -> Self {
    AltitudeSample {
      mode: mode.to_string(),
      mins: mins,
      jd: jd,
      value: value,
    }
  }

  pub fn basic_var(mode: &str, value: f64) -> Self {
    AltitudeSample {
      mode: mode.to_string(),
      mins: 0f64,
      jd: 0f64,
      value,
    }
  }
  pub fn basic(mode: &str) -> Self {
    AltitudeSample::basic_var(mode, 0f64)
  }

  pub fn basic_low(mode: &str) -> Self {
    AltitudeSample::basic_var(mode, -90f64)
  }

  pub fn basic_high(mode: &str) -> Self {
    AltitudeSample::basic_var(mode, 90f64)
  }

  pub fn set_mode(&mut self, mode: &str) {
    self.mode = mode.to_string();
  }

  // micro-adjustment to account for altitude speed
  pub fn set_frac_jd_diff(&mut self, prev_val: f64, max_mode: bool) {
    let frac_diff = if max_mode { self.value.abs() - prev_val.abs() } else { prev_val.abs() - self.value.abs() };
    // estimate approx. fractional difference in jd based on altitude difference
    // this serves as the base for subsequent more accurate recalculation over a narrower time range
    let frac_diff_jd = frac_diff / (MINS_PER_DAY / 3) as f64;
    self.jd = self.jd + frac_diff_jd;
  }

  pub fn to_key_num(&self) -> KeyNumValue {
    KeyNumValue::new(self.mode.as_str(), self.jd)
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum SamplePos {
  Position(BodyPos),
  Samples(Vec<BodyPos>)
}

impl SamplePos {

  pub fn new_pos(body_pos: BodyPos) -> SamplePos {
    SamplePos::Position(body_pos)
  }

  pub fn body_pos_sample(&self, index: usize) -> BodyPos {
    match self {
      SamplePos::Position(bp) => bp.to_owned(),
      SamplePos::Samples(bps) => if bps.len() > index { bps[index].to_owned() } else if index > 0 && bps.len() > 0 { bps.last().unwrap().to_owned() } else { BodyPos::empty() },
    }
  }

  pub fn first_body_pos(&self) -> BodyPos {
    self.body_pos_sample(0)
  }

  pub fn sample_mode(&self) -> bool {
    match self {
      SamplePos::Samples(_bps) => true,
      _ => false,
    }
  }

}

/**
 * Calculate progress to zero between two sample altitudes mapped to the JD difference
 * Occasionally both may be positive or negative or the second sample value be very close to zero
 * In most set and rise cases, one sample is < 0 and the other > 0, taking into account the object's disc radius
 */
pub fn calc_mid_point(first: &AltitudeSample, second: &AltitudeSample) -> f64 {
  let value_diff = second.value - first.value;
  let progress = second.value.abs() / value_diff.abs();
  let jd_diff = (second.jd - first.jd).abs();
  second.jd - (jd_diff * progress)
}

/*
* If the rise / set times have not been captured as the switch between negative and position altitude
* within five minute smaples, but we have positive MC and negative IC, the rise and set times can be approximated
* from the MC and IC time and altitudes. This may happen to the sun near poles or to other celestial objects
* that only rise or set briefly in a 24 hour period
* 
*/
fn calc_jd_from_min_max(mc: &AltitudeSample, ic: &AltitudeSample, set_mode: bool) -> f64 {
  let diff_alt = mc.value - ic.value;
  let diff_jd = ic.jd - mc.jd;
  let progress = if diff_alt >= 0f64  { mc.value / diff_alt } else { 0f64 };
  let rel_diff = if set_mode { diff_jd.abs() } else { 0f64 - diff_jd.abs() };
  mc.jd + (rel_diff * progress)
}

/*
* Calculate the porjected transition time between two samples for rise/set based on altitude differences
*/
fn calc_mid_sample(
  item: &AltitudeSample,
  prev_min: f64,
  prev_value: f64,
  prev_jd: f64,
  mode: &str,
) -> AltitudeSample {
  let prev_sample = AltitudeSample::new(mode, prev_min, prev_jd, prev_value);
  let mid_point = calc_mid_point(&prev_sample, item);
  AltitudeSample::new(mode, prev_min, mid_point, 0f64)
}

fn recalc_min_max_transit_sample(
  sample: &AltitudeSample,
  geo: &GeoPos,
  adjusted: (f64, f64),
  max_mode: bool,
  multiplier: u8,
) -> AltitudeSample {
  let (lng, lat ) = adjusted;
  let sample_rate = 1f64 / 60f64;
  let mins_per_day = MINS_PER_DAY as f64;
  let mut new_sample = sample.to_owned();
  let num_sub_samples = multiplier as f64 * 2.25f64 * (1f64 / sample_rate);
  let sample_start_jd =
    new_sample.jd - (num_sub_samples / (2f64 / sample_rate) / mins_per_day);

  let sample_start_min = new_sample.mins - num_sub_samples / (2f64 / sample_rate);
  let mode = if max_mode { "mc" } else { "ic" };
  let max = num_sub_samples as i32 + multiplier as i32;
  for i in 0..max {
    let increment = i as f64;
    let mins = sample_start_min + increment * sample_rate;
    let increment_jd = (increment * sample_rate) / mins_per_day;
    let jd = sample_start_jd + increment_jd;
    let value = calc_altitude(jd, false, geo.lat, geo.lng, lng, lat);
    let item = AltitudeSample::new(mode, mins, jd, value);
    if max_mode && item.value > new_sample.value {
      new_sample = item;
    } else if !max_mode && item.value < new_sample.value {
      new_sample = item;
    }
  }
  new_sample
}

pub fn calc_transposed_object_transitions(
  jd_start: f64,
  geo: GeoPos,
  sample_pos: SamplePos,
  multiplier: u8,
  sample_key: &str,
  rise_set_minmax: bool,
) -> Vec<AltitudeSample> {
  let max = MINS_PER_DAY / multiplier as usize + 1;
  let body_pos = sample_pos.first_body_pos();
  let lng = body_pos.lng;
  let lat = body_pos.lat;
  let lng_speed = body_pos.lng_speed; 
  let sample_mode = sample_pos.sample_mode();
  let mut items: Vec<AltitudeSample> = Vec::with_capacity(max);
  let mut ic = AltitudeSample::basic_high("ic");
  let mut rise = AltitudeSample::basic("rise");
  let mut set = AltitudeSample::basic("set");
  let mut mc = AltitudeSample::basic_low("mc");
  let mut prev_value = 0f64;
  let mut prev_min = 0f64;
  let mut prev_jd = 0f64;
  // resample the longitude and latitude speed for the moon only
  let mut disc_offset = 0f64;
  if sample_key == "su" || sample_key == "mo" {
    let pheno = get_pheno_result(jd_start, sample_key, 0i32);
    disc_offset = pheno.apparent_diameter_of_disc / 4f64;
  }
  let resample_speed = sample_key == "mo" && lng_speed != 0f64;
  let mut obj_sets = false;
  let mut obj_rises = false;
  let mut mc_adjusted = (lng, lat);
  let mut ic_adjusted = (lng, lat);
  let mut prev_mc_val = 0f64; // previous MC value
  let mut prev_ic_val = 0f64; // previous IC
  // let mut mc_diff_set = false; //
  let mins_per_day = MINS_PER_DAY as f64;
  for i in 0..max {
    let n = i as f64 * multiplier as f64;
    let day_frac = n / mins_per_day;
    let jd = jd_start + day_frac;
    let mut sample_spd = lng_speed;
    let mut lat_spd = 0f64;
    if resample_speed {
      let sample_body = calc_body_jd_topo(jd, sample_key, geo, 0f64);
      sample_spd = sample_body.lng_speed;
      lat_spd = sample_body.lat_speed;
    }
    let adjusted_lng = if sample_mode {
      sample_pos.body_pos_sample(i as usize).lng
    } else if lng_speed != 0f64 {
      lng + sample_spd * day_frac
    } else {
      lng
    };
    let adjusted_lat = if sample_mode {
      sample_pos.body_pos_sample(i as usize).lat
    } else if lat_spd != 0f64 {
      lat + lat_spd * day_frac
    } else {
      lat
    };
    let value = calc_altitude(jd, false, geo.lat, geo.lng, adjusted_lng, adjusted_lat);

    let mut item = AltitudeSample::new("", n, jd, value);

    if value > mc.value {
      item.set_mode("mc");
      mc = item.clone();
      if prev_mc_val != 0f64 {
        mc.set_frac_jd_diff(prev_mc_val, true);
        // mc_diff_set = true;
      }
      mc_adjusted = (adjusted_lng, adjusted_lat);
      prev_mc_val = mc.value;
    }
    if value < ic.value {
      item.set_mode("ic");
      // ic = item.clone();
      ic = item.clone();
      if prev_ic_val != 0f64 {
        ic.set_frac_jd_diff(prev_ic_val, false);
      }
      prev_ic_val = ic.value;
      ic_adjusted = (adjusted_lng, adjusted_lat);
      
    }
    let offset_pv = prev_value + disc_offset;
    let offset_v = value + disc_offset;
    let offset_pv2 = prev_value - disc_offset;
    let offset_v2 = value - disc_offset;
    if offset_pv < 0f64 && offset_v > 0f64 {
      rise = calc_mid_sample(&item, prev_min, offset_pv, prev_jd, "rise");
    } else if offset_pv2 > 0f64 && offset_v2 < 0f64 {
      set = calc_mid_sample(&item, prev_min, offset_pv2, prev_jd, "set");
    }
    if value > 0f64 && prev_value < 0f64 {
      obj_rises = true;
    }
    if value < 0f64 && prev_value > 0f64 {
      obj_sets = true;
    }
    items.push(item);
    prev_value = value;
    prev_min = n;
    prev_jd = jd;
  }
  if mc.jd > 0f64 {
    mc = recalc_min_max_transit_sample(&mc, &geo, mc_adjusted, true, multiplier);
    
  }
  if ic.jd > 0f64 {
    ic = recalc_min_max_transit_sample(&ic, &geo, ic_adjusted, false, multiplier);
  }
  if rise_set_minmax {
    let diff = if mc.jd > rise.jd { mc.jd - rise.jd } else { rise.jd - mc.jd };
   
    if rise.jd <= 0f64 {
      let rise_jd = if mc.value > 0f64 { 0f64 } else { mc.jd - diff };
      rise = AltitudeSample::new("rise", 0f64, rise_jd, 0f64);      
    }
    if set.jd <= 0f64 {
      let set_jd = if ic.value < 0f64 { ic.jd - diff } else { 0f64 };
      set = AltitudeSample::new("set", 0f64, set_jd, 0f64);
    }
  }
  if mc.value >= 0f64 && mc.value < UP_DOWN_TOLERANCE && set.jd <= MIN_JD && rise.jd > MIN_JD {
    let target_jd = calc_jd_from_min_max(&mc, &ic, true);
    set = AltitudeSample::new("set", prev_min, target_jd, 0f64);
  }
  if set.jd == 0f64 && rise.jd > 0f64 && mc.jd > 0f64 {
    let diff_mc = if mc.jd > rise.jd { mc.jd - rise.jd } else { rise.jd - mc.jd };
    if diff_mc < UP_DOWN_TOLERANCE && obj_sets {
      set = AltitudeSample::new("set", prev_min, mc.jd + diff_mc, 0f64);
    }
  } else if rise.jd == 0f64 && set.jd > 0f64 && mc.jd > 0f64 {
    let diff_mc = if set.jd > mc.jd { set.jd - mc.jd } else { mc.jd - set.jd };
    if diff_mc < UP_DOWN_TOLERANCE && obj_rises {
      rise = AltitudeSample::new("rise", prev_min, mc.jd - diff_mc, 0f64);
    }
  }
  
  vec![rise, set, mc, ic]
}


pub fn build_transposed_transition_set_from_pos(
  jd_start: f64,
  geo: GeoPos,
  pos: SamplePos,
  days: u16,
) -> KeyNumValueSet {
  let mut items: Vec<KeyNumValue> = Vec::new();
  for i in 0..days {
    let ref_jd = jd_start + i as f64;
    let tr_samples: Vec<AltitudeSample> = calc_transposed_object_transitions(
      ref_jd,
      geo,
      pos.clone(),
      5,
      pos.first_body_pos().key.as_str(),
      true,
    );
    let mut new_items: Vec<KeyNumValue> = tr_samples.iter().map(|tr| tr.to_key_num()).collect();
    items.append(&mut new_items);
  }
  KeyNumValueSet::new(pos.first_body_pos().key.as_str(), items)
}

/*
  Calculate transposed transitions from a set of historic body references with a different time and place
*/
pub fn calc_transposed_graha_transitions_from_source_refs(
  mode: &str,
  jd_start: f64,
  geo: GeoPos,
  jd_historic: f64,
  geo_historic: GeoPos,
  keys: Vec<String>,
  days: u16,
) -> Vec<KeyNumValueSet> {
  let mut key_num_sets: Vec<KeyNumValueSet> = Vec::new();
  for key in keys {
    let graha_pos = match mode {
      "topo" => calc_body_jd_topo(jd_historic, key.as_str(), geo_historic, 0f64),
      _ => calc_body_jd_geo(jd_historic, key.as_str(), 0f64),
    };
    let tr_key_set =
      build_transposed_transition_set_from_pos(jd_start, geo, SamplePos::new_pos(graha_pos.to_body(CoordinateSystem::Ecliptic)), days);
    key_num_sets.push(tr_key_set);
  }
  key_num_sets
}

fn extract_from_alt_samples(alt_samples: &Vec<AltitudeSample>, key: &str) -> AltitudeSample {
  alt_samples
    .into_iter()
    .find(|sample| sample.mode.as_str() == key)
    .unwrap_or(&AltitudeSample::basic(key))
    .to_owned()
}

/**
 * Alternative method to fetch transitions for near polar latitudes (> +60 and < -60) based on altitudes
*/
pub fn calc_transitions_from_source_refs_altitude(
  jd: f64,
  key: &str,
  geo: GeoPos,
) -> TransitionSet {
  let pos = calc_body_jd_topo(jd, key, geo, 0f64);
  calc_transition_set_from_lng_lat_speed(jd, key, geo, pos.lng, pos.lat, pos.lng_speed)
}

pub fn calc_transition_set_from_lng_lat_speed(
  jd: f64,
  key: &str,
  geo: GeoPos,
  lng: f64,
  lat: f64,
  lng_speed: f64,
) -> TransitionSet {
  let sample_pos = SamplePos::new_pos(BodyPos::new(key, CoordinateSystem::Ecliptic, lng, lat, lng_speed, 0f64));
  let alt_samples = calc_transposed_object_transitions(
    jd,
    geo,
    sample_pos,
    5,
    key,
    true,
  );
  let rise = extract_from_alt_samples(&alt_samples, "rise");
  let set = extract_from_alt_samples(&alt_samples, "set");
  let mc = extract_from_alt_samples(&alt_samples, "mc");
  let ic = extract_from_alt_samples(&alt_samples, "ic");
  TransitionSet {
    rise: rise.jd,
    mc: mc.jd,
    set: set.jd,
    ic: ic.jd,
  }
}

/**
 * Alternative method to fetch transitions for near polar latitudes (> +60 and < -60) with min and max altitudes
*/
pub fn calc_transitions_from_source_refs_minmax(
  jd: f64,
  key: &str,
  geo: GeoPos,
) -> AltTransitionSet {
  let graha_pos = calc_body_jd_topo(jd, key, geo, 0f64);
  let sample_pos = SamplePos::new_pos(graha_pos.to_body(CoordinateSystem::Ecliptic));
  let alt_samples = calc_transposed_object_transitions(
    jd,
    geo,
    sample_pos,
    5,
    key,
    false,
  );
  let rise = extract_from_alt_samples(&alt_samples, "rise");
  let set = extract_from_alt_samples(&alt_samples, "set");
  let mc = extract_from_alt_samples(&alt_samples, "mc");
  let ic = extract_from_alt_samples(&alt_samples, "ic");
  AltTransitionSet {
    min: ic.value,
    rise: rise.jd,
    mc: mc.jd,
    set: set.jd,
    ic: ic.jd,
    max: mc.value,
  }
}

pub fn calc_transposed_graha_transitions_from_source_refs_topo(
  jd_start: f64,
  geo: GeoPos,
  jd_historic: f64,
  geo_historic: GeoPos,
  keys: Vec<String>,
  days: u16,
) -> Vec<KeyNumValueSet> {
  calc_transposed_graha_transitions_from_source_refs(
    "topo",
    jd_start,
    geo,
    jd_historic,
    geo_historic,
    keys,
    days,
  )
}

/*
Calculate transposed transitions from a set of real body positions with a different time with geocentric positions
*/
pub fn calc_transposed_graha_transitions_from_source_refs_geo(
  jd_start: f64,
  geo: GeoPos,
  jd_historic: f64,
  geo_historic: GeoPos,
  keys: Vec<String>,
  days: u16,
) -> Vec<KeyNumValueSet> {
  calc_transposed_graha_transitions_from_source_refs(
    "geo",
    jd_start,
    geo,
    jd_historic,
    geo_historic,
    keys,
    days,
  )
}

#[cfg(test)]
mod tests {

  use super::*;
  /*
  * Test the projected julian day value for rise and set transition lies within the expected 5 minute window
  * based on the differences between sample altitudes taken at 5 minute intervals
  */
  #[test]
  fn test_midpoint_calculation() {
    let sample_pairs: Vec<(AltitudeSample, AltitudeSample)> = vec![
      (AltitudeSample::new("set", 620.5, 2460269.9496062486f64, -0.1375207470177369f64), AltitudeSample::new("set", 620.5, 2460269.9496062486f64, -0.13176826910947412)),
      (AltitudeSample::new("rise",760.0, 2460271.9393263613f64, 0.059815943129908664f64  ), AltitudeSample::new("rise",765.0, 2460271.9374574847f64, 0.0f64  ))
    ];
    let mut num_ok = 0;
    let five_min = 1f64 / 288f64;
    let num = sample_pairs.len();
    for pair in sample_pairs {
      let new_val = calc_mid_point(&pair.0, &pair.1);
      if (new_val - pair.0.jd).abs() < five_min {
        num_ok += 1;
      }
    }
    assert_eq!(num, num_ok);
  }
}