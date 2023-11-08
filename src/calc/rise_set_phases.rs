use crate::extensions::swe::rise_trans;
use super::utils::converters::get_next_prev_rise_set_polar_calc_offset;
use super::{
  core::calc_altitude_object,
  dates::julian_day_to_iso_datetime,
  models::{
    general::*,
    geo_pos::*,
    graha_pos::{PhenoItem, PhenoResult},
  },
  traits::*,
  transposed_transitions::{
    calc_transitions_from_source_refs_altitude, calc_transitions_from_source_refs_minmax
  },
};
use libswe_sys::sweconst::Bodies;
use libswe_sys::swerust::handler_swe07::pheno_ut;
use serde::{Deserialize, Serialize};

pub const MAX_POLAR_DAY: i32 = 183;
pub const MIN_JD: f64 = 1000f64;
pub const UP_DOWN_TOLERANCE: f64 = 0.5f64;

pub enum TransitionParams {
  Rise = 1,
  Set = 2,
  Mc = 4,
  Ic = 8,
  Center = 256,
  Bottom = 8192,
  Fixed = 16384,
  BitNoRefraction = 512,
  BitGeoctrNoEclLat = 128,
}

impl TransitionParams {
  pub fn center_disc_rising() -> i32 {
    TransitionParams::Center as i32
      | TransitionParams::BitNoRefraction as i32
  }

/*   pub fn bottom_disc_rising() -> i32 {
    TransitionParams::Bottom as i32
      | TransitionParams::BitNoRefraction as i32
  } */

  pub fn center_disc_rising_rise() -> i32 {
    TransitionParams::center_disc_rising() | TransitionParams::Rise as i32
  }

  pub fn rise_normal() -> i32 {
    TransitionParams::Fixed as i32 | TransitionParams::Rise as i32
  }

  pub fn set_normal() -> i32 {
    TransitionParams::Fixed as i32 | TransitionParams::Set as i32
  }

  pub fn center_disc_rising_set() -> i32 {
    TransitionParams::center_disc_rising() | TransitionParams::Set as i32
  }

  pub fn mc() -> i32 {
    TransitionParams::BitNoRefraction as i32 | TransitionParams::Mc as i32
  }

  pub fn ic() -> i32 {
    TransitionParams::BitNoRefraction as i32 | TransitionParams::Ic as i32
  }
}

pub trait TransitionGroup {
  fn period(&self) -> f64;

  fn to_key_nums(&self) -> Vec<KeyNumValue>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtendedTransitionSet {
  #[serde(rename = "prevSet")]
  pub prev_set: f64,
  pub rise: f64,
  pub mc: f64,
  pub set: f64,
  pub ic: f64,
  #[serde(rename = "nextRise")]
  pub next_rise: f64,
  pub min: f64,
  pub max: f64,
}

impl TransitionGroup for ExtendedTransitionSet {
  fn period(&self) -> f64 {
    self.set - self.rise
  }

  fn to_key_nums(&self) -> Vec<KeyNumValue> {
    let is_up = self.min >= 0f64 && self.max > 0f64;
    let prev_key = if is_up { "prev_rise" } else { "prev_set" };
    let next_key = if is_up { "next_set" } else { "next_rise" };
    vec![
      KeyNumValue::new(prev_key, self.prev_set),
      KeyNumValue::new("rise", self.rise),
      KeyNumValue::new("mc", self.mc),
      KeyNumValue::new("set", self.set),
      KeyNumValue::new("ic", self.ic),
      KeyNumValue::new(next_key, self.next_rise),
      KeyNumValue::new("min", self.min),
      KeyNumValue::new("max", self.max),
    ]
  }
}

impl ExtendedTransitionSet {

  fn is_up_by(&self, tolerance: f64) -> bool {
    (self.rise == 0f64 || self.set == 0f64) && self.min >= (0f64 - tolerance)
  }
  
  /*
  * celestial object sets after an extended period of being up all day.
  * it will thus rise again soon
  */
  pub fn up_period_over(&self) -> bool {
    (self.rise == 0f64 && self.set > 0f64) && self.min < 0f64
  }

   /*
  * celestial object rise after an extended period of being down all day.
  * it will thus set again soon
  */
  /* pub fn down_period_over(&self) -> bool {
    (self.set == 0f64 && self.rise > 0f64) && self.max > 0f64
  } */

  pub fn is_up(&self) -> bool {
    self.is_up_by(UP_DOWN_TOLERANCE)
  }

 /*  pub fn is_all_up(&self) -> bool {
    self.is_up_by(0f64)
  } */

  fn is_down_by(&self, tolerance: f64) -> bool {
    (self.rise == 0f64 || self.set == 0f64) && self.max <= tolerance
  }

  pub fn is_down(&self) -> bool {
    self.is_down_by(UP_DOWN_TOLERANCE)
  }

  /* pub fn is_all_down(&self) -> bool {
    self.is_down_by(0f64)
  }

  pub fn start_mode(&self) -> i8 {
    if self.is_up() {
      1
    } else if self.is_down() {
      -1
    } else {
      0
    }
  } */

  pub fn as_iso_datetime(&self) -> ExtendedTransitionIsoSet {
    let prev_rise_val = if self.is_up() { self.prev_set } else { 0f64 };
    let prev_set_val = if self.is_up() { 0f64 } else { self.prev_set };
    let next_rise_val = if self.is_up() { 0f64 } else { self.next_rise };
    let next_set_val = if self.is_up() { self.next_rise } else { 0f64 };
    ExtendedTransitionIsoSet {
      min: self.min,
      prev_rise: julian_day_to_iso_datetime(prev_rise_val),
      prev_set: julian_day_to_iso_datetime(prev_set_val),
      rise: julian_day_to_iso_datetime(self.rise),
      mc: julian_day_to_iso_datetime(self.mc),
      set: julian_day_to_iso_datetime(self.set),
      ic: julian_day_to_iso_datetime(self.ic),
      next_rise: julian_day_to_iso_datetime(next_rise_val),
      next_set: julian_day_to_iso_datetime(next_set_val),
      max: self.max,
    }
  }

  pub fn to_value_set(&self, iso_mode: bool) -> AltTransitionValueSet {
    match iso_mode {
      true => AltTransitionValueSet::ExtendedIsoValues(self.as_iso_datetime()),
      _ => AltTransitionValueSet::ExtendedJdValues(self.to_owned()),
    }
  }

  /*
  * set previous rise / set in a loop when the object is up or down for the whole day
  * and previous value has been calculated earlier in the loop
  */
  pub fn set_prev(&mut self, prev_jd: f64) {
    self.prev_set = prev_jd;
  }

  /*
  * set next rise / set in a loop when the object is up or down for the whole day
  * and previous value has been calculated earlier in the loop
  */
  pub fn set_next(&mut self, next_jd: f64) {
    self.next_rise = next_jd;
  }

/*   pub fn set_rise(&mut self) {
    let jd_diff = self.mc - self.ic;
    let fraction = (self.max + self.min) / self.min;
    let progress = if fraction != 0f64 { 1f64 / fraction } else { 1f64 };
    self.rise = self.set - jd_diff * progress;
  } */

  pub fn needs_rise(&self) -> bool {
    self.rise < MIN_JD && self.max > 0f64 && self.min < 0f64
  }

}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtendedTransitionIsoSet {
  #[serde(rename = "prevSet", skip_serializing_if = "String::is_empty")]
  pub prev_set: String,
  #[serde(rename = "prevRise", skip_serializing_if = "String::is_empty")]
  pub prev_rise: String,
  #[serde(skip_serializing_if = "String::is_empty")]
  pub rise: String,
  #[serde(skip_serializing_if = "String::is_empty")]
  pub mc: String,
  #[serde(skip_serializing_if = "String::is_empty")]
  pub set: String,
  pub ic: String,
  #[serde(rename = "nextRise", skip_serializing_if = "String::is_empty")]
  pub next_rise: String,
  #[serde(rename = "nextSet", skip_serializing_if = "String::is_empty")]
  pub next_set: String,
  pub min: f64,
  pub max: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AltTransitionSet {
  pub min: f64,
  pub rise: f64,
  pub mc: f64,
  pub set: f64,
  pub ic: f64,
  pub max: f64,
}

impl AltTransitionSet {

  pub fn is_up(&self) -> bool {
    (self.rise == 0f64 || self.set == 0f64) && self.min >= (0f64 - UP_DOWN_TOLERANCE)
  }

  pub fn is_down(&self) -> bool {
    (self.rise == 0f64 || self.set == 0f64) && self.max <= UP_DOWN_TOLERANCE
  }

  pub fn start_mode(&self) -> i8 {
    if self.is_up() {
      1
    } else if self.is_down() {
      -1
    } else {
      0
    }
  }

  pub fn as_iso_datetime(&self) -> AltTransitionIsoSet {
    AltTransitionIsoSet {
      min: self.min,
      rise: julian_day_to_iso_datetime(self.rise),
      mc: julian_day_to_iso_datetime(self.mc),
      set: julian_day_to_iso_datetime(self.set),
      ic: julian_day_to_iso_datetime(self.ic),
      max: self.max,
    }
  }

  pub fn to_value_set(&self, iso_mode: bool) -> AltTransitionValueSet {
    match iso_mode {
      true => AltTransitionValueSet::IsoValues(self.as_iso_datetime()),
      _ => AltTransitionValueSet::JdValues(self.to_owned()),
    }
  }
}

impl TransitionGroup for AltTransitionSet {
  fn period(&self) -> f64 {
    self.set - self.rise
  }

  fn to_key_nums(&self) -> Vec<KeyNumValue> {
    vec![
      KeyNumValue::new("min", self.min),
      KeyNumValue::new("rise", self.rise),
      KeyNumValue::new("mc", self.mc),
      KeyNumValue::new("set", self.set),
      KeyNumValue::new("ic", self.ic),
      KeyNumValue::new("max", self.max),
    ]
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum AltTransitionValueSet {
  JdValues(AltTransitionSet),
  IsoValues(AltTransitionIsoSet),
  ExtendedJdValues(ExtendedTransitionSet),
  ExtendedIsoValues(ExtendedTransitionIsoSet),
}
/*
 This serves only show rise, set, mc and ic times as ISO UTC strings.
*/
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AltTransitionIsoSet {
  pub min: f64,
  #[serde(skip_serializing_if = "String::is_empty")]
  pub rise: String,
  pub mc: String,
  #[serde(skip_serializing_if = "String::is_empty")]
  pub set: String,
  pub ic: String,
  pub max: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransitionSet {
  pub rise: f64,
  pub mc: f64,
  pub set: f64,
  pub ic: f64,
}

impl TransitionGroup for TransitionSet {
  fn period(&self) -> f64 {
    self.set - self.rise
  }

  fn to_key_nums(&self) -> Vec<KeyNumValue> {
    let rise_key = if self.rise < 100f64 { "max" } else { "rise" };
    let set_key = if self.set < 100f64 { "min" } else { "set" };
    vec![
      KeyNumValue::new(rise_key, self.rise),
      KeyNumValue::new("mc", self.mc),
      KeyNumValue::new(set_key, self.set),
      KeyNumValue::new("ic", self.ic),
    ]
  }
}

impl TransitionSet {
  pub fn empty() -> Self {
    TransitionSet{
      rise: 0f64,
      mc: 0f64,
      set: 0f64,
      ic: 0f64,
    }
  }

}

pub fn is_near_poles(lat: f64) -> bool {
  lat >= 60f64 || lat <= -60f64
}

pub fn calc_transition_set_extended_fast(
  jd: f64,
  ipl: Bodies,
  lat: f64,
  lng: f64,
) -> ExtendedTransitionSet {
  let ref_jd = start_jd_geo(jd, lng);
  let prev_set = next_set(ref_jd - 1f64, ipl, lat, lng);
  let rise = next_rise(ref_jd, ipl, lat, lng);
  let set = next_set(ref_jd, ipl, lat, lng);
  //let mc = next_mc_q(ref_jd, ipl, lat, lng, rise);
  let mc = next_mc_normal(ref_jd, ipl, lat, lng);
  let ic = next_ic_normal(ref_jd, ipl, lat, lng);
  let next_rise = next_rise(set, ipl, lat, lng);
  let min = calc_altitude_object(ic, false, lat, lng, ipl.to_key());
  let max = calc_altitude_object(mc, false, lat, lng, ipl.to_key());
  ExtendedTransitionSet {
    prev_set,
    rise,
    mc,
    set,
    ic,
    next_rise,
    min,
    max,
  }
}

pub fn calc_transition_set_alt_fast(jd: f64, ipl: Bodies, lat: f64, lng: f64) -> AltTransitionSet {
  let ref_jd = start_jd_geo(jd, lng);
  let rise = next_rise(ref_jd, ipl, lat, lng);
  let set = next_set(ref_jd, ipl, lat, lng);
  let mc = next_mc_normal(ref_jd, ipl, lat, lng);
  let ic = next_ic_normal(ref_jd, ipl, lat, lng);
  let min = calc_altitude_object(ic, false, lat, lng, ipl.to_key());
  let max = calc_altitude_object(mc, false, lat, lng, ipl.to_key());
  AltTransitionSet {
    min,
    rise,
    mc,
    set,
    ic,
    max,
  }
}


pub fn calc_transition_set_extended_azalt(
  jd: f64,
  ipl: Bodies,
  lat: f64,
  lng: f64,
  get_prev_next: bool
) -> ExtendedTransitionSet {
  let ref_jd = start_jd_geo(jd, lng);
  let geo = GeoPos::simple(lat, lng);
  let ref_key = ipl.to_key();
  let base = calc_transitions_from_source_refs_minmax(ref_jd, ref_key, geo);
  let prev = if get_prev_next { calc_transitions_from_source_refs_altitude(ref_jd - 1f64, ref_key, geo) } else { TransitionSet::empty() };
  let next = if get_prev_next { calc_transitions_from_source_refs_altitude(ref_jd + 1f64, ref_key, geo) } else { TransitionSet::empty() };
  let mut prev_set = prev.set;
  let mut next_rise = next.rise;
  
  if get_prev_next && (next.rise < MIN_JD || prev.set < MIN_JD) {

    let (counter_start_next, counter_start_prev) = get_next_prev_rise_set_polar_calc_offset(ref_jd, geo.lat);
    //let start_counter = 0;
    let mut counter = counter_start_next;
    if next_rise < MIN_JD {
      while next_rise < MIN_JD && counter < MAX_POLAR_DAY {
        let nx = calc_transitions_from_source_refs_altitude(ref_jd + counter as f64, ref_key, geo);
        if nx.rise >= MIN_JD && ( counter_start_prev < 1 || nx.rise > base.rise) {
          next_rise = nx.rise;
        }
        counter += 1;
      }
    }
    if prev_set < MIN_JD {
      counter = counter_start_prev;
      while prev_set < MIN_JD && counter < MAX_POLAR_DAY {
        let pv = calc_transitions_from_source_refs_altitude(ref_jd - counter as f64, ref_key, geo);
        if pv.set >= MIN_JD {
          prev_set = pv.set;
        }
        counter += 1;
      }
    }
  }
  ExtendedTransitionSet {
    prev_set,
    rise: base.rise,
    mc: base.mc,
    set: base.set,
    ic: base.ic,
    next_rise,
    min: base.min,
    max: base.max,
  }
}

pub fn calc_next_set_or_prev_rise_jd(ref_jd: f64, geo: GeoPos, ipl:Bodies, next: bool) -> f64 {
  let ref_key = ipl.to_key();
  let (counter_start_next, _counter_start_prev) = get_next_prev_rise_set_polar_calc_offset(ref_jd, geo.lat);
  let mut target_jd = 0f64;
  let mut counter = counter_start_next;
  while target_jd < MIN_JD && counter < MAX_POLAR_DAY {
    let sample_jd = if next { ref_jd + counter as f64 } else { ref_jd - counter as f64 };
    let sample = calc_transitions_from_source_refs_altitude(sample_jd, ref_key, geo);
    let new_target_jd = if next { sample.set } else { sample.rise };
    if new_target_jd >= MIN_JD {
      target_jd = new_target_jd;
    }
    counter += 1;
  }
  target_jd
}

pub fn calc_transition_set_alt_azalt(jd: f64, ipl: Bodies, lat: f64, lng: f64) -> AltTransitionSet {
  let ref_jd = start_jd_geo(jd, lng);
  let geo = GeoPos::simple(lat, lng);
  let ref_key = ipl.to_key();
  calc_transitions_from_source_refs_minmax(ref_jd, ref_key, geo)
}

pub fn calc_transition_set_extended(
  jd: f64,
  ipl: Bodies,
  lat: f64,
  lng: f64,
  get_prev_next: bool
) -> ExtendedTransitionSet {
  if is_near_poles(lat) {
    calc_transition_set_extended_azalt(jd, ipl, lat, lng, get_prev_next)
  } else {
    calc_transition_set_extended_fast(jd, ipl, lat, lng)
  }
}

pub fn calc_transition_set_alt(jd: f64, ipl: Bodies, lat: f64, lng: f64) -> AltTransitionSet {
  if is_near_poles(lat) {
    calc_transition_set_alt_azalt(jd, ipl, lat, lng)
  } else {
    calc_transition_set_alt_fast(jd, ipl, lat, lng)
  }
}

pub fn calc_transition_set_fast(jd: f64, ipl: Bodies, lat: f64, lng: f64) -> TransitionSet {
  let ref_jd = start_jd_geo(jd, lng);
  let rise = next_rise(ref_jd, ipl, lat, lng);
  let set = next_set(rise, ipl, lat, lng);
  /* let mc = next_mc(ref_jd, ipl, lat, lng);
  let ic = next_ic(ref_jd, ipl, lat, lng); */
  // MC/IC flags have issues via alc_mer_trans when compiled with gcc
  // use median of rise/set with fixed disc instead
  let mc = next_mc_normal(ref_jd, ipl, lat, lng);
  let ic = next_ic_normal(mc, ipl, lat, lng);
  TransitionSet { rise, mc, set, ic }
}

pub fn calc_transition_set(jd: f64, ipl: Bodies, lat: f64, lng: f64) -> TransitionSet {
  if is_near_poles(lat) {
    let ref_jd = start_jd_geo(jd, lng);
    calc_transitions_from_source_refs_altitude(ref_jd, ipl.to_key(), GeoPos::simple(lat, lng))
  } else {
    calc_transition_set_fast(jd, ipl, lat, lng)
  }
}

pub fn calc_transition_sun(jd: f64, geo: GeoPos, get_prev_next: bool) -> ExtendedTransitionSet {
  calc_transition_set_extended(jd, Bodies::Sun, geo.lat, geo.lng, get_prev_next)
}

pub fn calc_transition_sets_sun(jd: f64, days: u16, geo: GeoPos) -> Vec<ExtendedTransitionSet> {
  let mut sets: Vec<ExtendedTransitionSet> = Vec::new();
  let mut prev_set_jd = 0f64;
  let mut next_rise_jd = 0f64;
  let mut prev_rise_jd = 0f64;
  let mut next_set_jd = 0f64;
  let mut get_prev_next = true;
  let mut prev_up_over = false;
  for i in 0..days {
    let ref_jd = jd + i as f64;
    let mut row = calc_transition_sun(ref_jd, geo, get_prev_next);
    if row.next_rise > 0f64 {
      next_rise_jd = row.next_rise;
    }
    if row.set > 0f64 {
      prev_set_jd = row.set;
    } else if row.prev_set > MIN_JD {
      prev_set_jd = row.prev_set;
    }
    if row.rise > 0f64 {
      prev_rise_jd = row.rise;
    } 
    if row.is_down() || row.is_up() {
      if get_prev_next {
        get_prev_next = false;
      }
      if row.is_down() {
        row.set_next(next_rise_jd);
        row.set_prev(prev_set_jd);
      } else {
        if prev_rise_jd < MIN_JD  {
          prev_rise_jd = calc_next_set_or_prev_rise_jd(ref_jd, geo, Bodies::Sun, false);
        }
        row.set_prev(prev_rise_jd);
        if next_set_jd < MIN_JD {
          next_set_jd = calc_next_set_or_prev_rise_jd(ref_jd, geo, Bodies::Sun, true);
        }
        row.set_next(next_set_jd);
      }
    } else {
      if row.is_down() {
        next_set_jd = 0f64;
      }
    }
    if row.set > MIN_JD || row.rise > MIN_JD {
      if !get_prev_next {
        get_prev_next = true;
      }
    }
    if prev_up_over && row.needs_rise() {
     // row.set_rise();
    }
    prev_up_over = row.up_period_over();
    sets.push(row);
  }
  sets
}

pub fn calc_transitions_sun(jd: f64, days: u16, geo: GeoPos) -> Vec<KeyNumValue> {
  let mut sets: Vec<KeyNumValue> = Vec::new();
  for i in 0..days {
    let ref_jd = jd + i as f64;
    let items = calc_transition_set_alt(ref_jd, Bodies::Sun, geo.lat, geo.lng).to_key_nums();
    for item in items {
      sets.push(item);
    }
  }
  sets
}

pub fn calc_transition_moon(jd: f64, geo: GeoPos, get_prev_next: bool) -> ExtendedTransitionSet {
  calc_transition_set_extended(jd, Bodies::Moon, geo.lat, geo.lng, get_prev_next)
}

/* pub fn calc_transition_body(jd: f64, ipl: Bodies, geo: GeoPos) -> TransitionSet {
  calc_transition_set(jd, ipl, geo.lat, geo.lng)
} */

pub fn next_rise(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64) -> f64 {
  rise_trans(
    tjd_ut,
    ipl,
    lat,
    lng,
    TransitionParams::center_disc_rising_rise(),
  )
}

pub fn next_set(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64) -> f64 {
  rise_trans(
    tjd_ut,
    ipl,
    lat,
    lng,
    TransitionParams::center_disc_rising_set(),
  )
}

pub fn next_mc_normal(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64) -> f64 {
  let value = next_mc(tjd_ut, ipl, lat, lng);
  if value >= 1f64 {
    value
  } else {
    let rise_n = rise_trans(tjd_ut, ipl, lat, lng, TransitionParams::rise_normal());
    let set_n = rise_trans(rise_n, ipl, lat, lng, TransitionParams::set_normal());
    (set_n + rise_n) / 2f64
  }
}

pub fn next_ic_normal(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64) -> f64 {
  let value = next_ic(tjd_ut, ipl, lat, lng);
  if value >= 1f64 {
    value
  } else {
    let set_n = rise_trans(tjd_ut, ipl, lat, lng, TransitionParams::set_normal());
    let next_rise_n = rise_trans(set_n, ipl, lat, lng, TransitionParams::rise_normal());
    (next_rise_n + set_n) / 2f64
  }
}

pub fn next_mc(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64) -> f64 {
  rise_trans(tjd_ut, ipl, lat, lng, TransitionParams::mc())
}

pub fn next_ic(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64) -> f64 {
  rise_trans(tjd_ut, ipl, lat, lng, TransitionParams::ic())
}

pub fn next_rise_normal(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64, center_disc: bool) -> f64 {
  let tp = if center_disc { TransitionParams::center_disc_rising_rise() } else { TransitionParams::rise_normal() };
  rise_trans(tjd_ut, ipl, lat, lng, tp)
}

pub fn next_set_normal(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64, center_disc: bool) -> f64 {
  let tp = if center_disc { TransitionParams::center_disc_rising_set() } else { TransitionParams::set_normal() };
  rise_trans(tjd_ut, ipl, lat, lng, tp)
}

pub fn longitude_to_solar_time_offset_jd(lng: f64) -> f64 {
  (0f64 - lng / 15f64) / 24f64
}

pub fn start_jd_geo(jd: f64, lng: f64) -> f64 {
  let offset = longitude_to_solar_time_offset_jd(lng);
  let jd_progress = jd % 1f64;
  let adjusted_progress = offset - jd_progress;
  let start_offset = if adjusted_progress >= 0.5 {
    0.5f64
  } else {
    -0.5f64
  };
  let start = jd.floor() + start_offset;
  let ref_jd = start + offset;
  let diff = jd - ref_jd;
  if diff > 1f64 {
    ref_jd + 1f64
  } else if diff < -1f64 {
    ref_jd - 1f64
  } else {
    ref_jd
  }
}

/**
 * Used by test function to estimaye time zone offset based on longitude
 */
pub fn start_jd_geo_tz(jd: f64, lng: f64, tz_offset: Option<i32>) -> f64 {
  let lng_offset = match tz_offset {
    Some(tzs) => tzs as f64 / 240f64,
    _ => lng,
  };
  start_jd_geo(jd, lng_offset)
}

pub fn get_transition_sets(jd: f64, keys: Vec<&str>, geo: GeoPos) -> Vec<KeyNumValueSet> {
  let mut transit_sets: Vec<KeyNumValueSet> = Vec::new();
  for key in keys {
    let tr_set: Vec<KeyNumValue> = match key {
      "su" | "mo" => {
        calc_transition_set_extended(jd, Bodies::from_key(key), geo.lat, geo.lng, true).to_key_nums()
      }
      _ => calc_transition_set(jd, Bodies::from_key(key), geo.lat, geo.lng).to_key_nums(),
    };
    transit_sets.push(KeyNumValueSet::new(key, tr_set));
  }
  transit_sets
}

pub fn get_transition_sets_extended(
  jd: f64,
  keys: Vec<String>,
  geo: GeoPos,
  days: u16,
) -> Vec<KeyNumValueSet> {
  let mut transit_sets: Vec<KeyNumValueSet> = Vec::new();
  for key in keys {
    let mut tr_set: Vec<KeyNumValue> = Vec::new();
    for i in 0..days {
      let ref_jd = jd + i as f64;
      if key.len() == 2 {
        let mut tr_set_day =
        calc_transition_set_alt(ref_jd, Bodies::from_key(key.as_str()), geo.lat, geo.lng)
          .to_key_nums();
        tr_set.append(&mut tr_set_day);
      }
    }
    transit_sets.push(KeyNumValueSet::new(key.as_str(), tr_set));
  }
  transit_sets
}

pub fn get_pheno_result(jd: f64, key: &str, iflag: i32) -> PhenoResult {
  let ipl = Bodies::from_key(key);
  let result = pheno_ut(jd, ipl, iflag);
  PhenoResult::new_from_result(result)
}

pub fn get_pheno_results(jd: f64, keys: Vec<&str>) -> Vec<PhenoItem> {
  let mut items: Vec<PhenoItem> = Vec::new();
  for key in keys {
    let ipl = Bodies::from_key(key);
    let result = pheno_ut(jd, ipl, 0i32);
    let item = PhenoItem::new_from_result(key, result);
    items.push(item);
  }
  items
}

pub fn to_sun_rise_sets(
  jd: f64,
  geo: GeoPos,
  offset_tz_secs: Option<i32>,
  iso_mode: bool,
) -> (
  AltTransitionValueSet,
  AltTransitionValueSet,
  AltTransitionValueSet,
  i32,
) {
  let sun = Bodies::from_key("su");
  let current = calc_transition_set_extended(jd, sun, geo.lat, geo.lng, false);
  let prev = calc_transition_set_alt(jd - 1f64, sun, geo.lat, geo.lng);
  let next = calc_transition_set_alt(jd + 1f64, sun, geo.lat, geo.lng);
  let offset_secs = if offset_tz_secs != None {
    offset_tz_secs.unwrap()
  } else {
    (geo.lng * 240f64) as i32
  };
  (
    prev.to_value_set(iso_mode),
    current.to_value_set(iso_mode),
    next.to_value_set(iso_mode),
    offset_secs,
  )
}



#[cfg(test)]
mod tests {
  use super::{start_jd_geo, start_jd_geo_tz};
  #[test]
  fn has_correct_geo_day_start_offset() {
    let ref_jd = 2459731.875;
    let day_start_utc_jd = 2459731.5;
    let lng1 = 90f64; // 90ºE
    let lng2 = -90f64; // 90ºW
    let start_jd1 = start_jd_geo(ref_jd, lng1);
    let start_jd2 = start_jd_geo(ref_jd, lng2);
    let expected_start_1 = day_start_utc_jd - 0.25;
    let expected_start_2 = day_start_utc_jd + 0.25;
    assert_eq!(start_jd1, expected_start_1);
    assert_eq!(start_jd2, expected_start_2);
  }

  #[test]
  fn has_correct_geo_day_start_tz_offset_east() {
    let ref_jd = 2459731.875;
    let day_start_utc_jd = 2459731.5;
    let lng1 = 84f64; // 84ºE
    let tzs1 = 18000i32;
    let start_jd1 = start_jd_geo_tz(ref_jd, lng1, Some(tzs1));
    let expected_start_1 = day_start_utc_jd - (5f64 / 24f64);
    assert_eq!(start_jd1, expected_start_1);
  }

  #[test]
  fn has_correct_geo_day_start_tz_offset_west() {
    let ref_jd = 2459731.875;
    let day_start_utc_jd = 2459731.5;
    let lng2 = -84f64;
    let tzs2 = -18000i32;
    let start_jd2 = start_jd_geo_tz(ref_jd, lng2, Some(tzs2));
    let expected_start_2 = day_start_utc_jd + (5f64 / 24f64);
    assert_eq!(start_jd2, expected_start_2);
  }

  #[test]
  fn has_correct_geo_day_start_tz_offset_east_none() {
    let ref_jd = 2459731.875;
    let day_start_utc_jd = 2459731.5;
    let lng1 = 84f64; // 84ºE
    let start_jd3 = start_jd_geo_tz(ref_jd, lng1, None);
    let expected_start_3 = day_start_utc_jd - (5.6f64 / 24f64);
    assert_eq!(start_jd3, expected_start_3);
  }

  #[test]
  fn has_correct_geo_day_start_tz_offset_west_none() {
    let ref_jd = 2459731.875;
    let day_start_utc_jd = 2459731.5;
    let lng2 = -84f64; // 84ºW
    let start_jd4 = start_jd_geo_tz(ref_jd, lng2, None);
    let expected_start_4 = day_start_utc_jd + (5.6f64 / 24f64);
    assert_eq!(start_jd4, expected_start_4);
  }
}
