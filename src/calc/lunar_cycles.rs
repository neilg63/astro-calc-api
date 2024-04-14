use serde::Serialize;
use ring360::*;
use crate::calc::models::geo_pos::GeoPos;
use crate::calc::core::calc_body_jd_topo;
use crate::calc::dates::julian_day_to_iso_datetime;

const SHORT_LUNAR_MONTH: f64 = 24.0;
const MEDIAN_LUNAR_MONTH: f64 = 29.53059;

#[derive(Debug, Clone, Serialize)]
pub struct MoonPhase {
  jd: f64,
  utc: String,
  angle: f64,
  num: u8,
  waxing: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  days: Option<f64>,
}

impl MoonPhase {
  pub fn new(ref_jd: f64, angle: f64, num: u8, days: Option<f64>) -> Self {
    let target_angle = if num > 1 { (num - 1) as f64 * 90.0 } else { 360.0 };
    let rem_angle = target_angle - angle;
    let angle = if rem_angle > 0.05 { angle } else { target_angle % 360.0 };
    let extra_jd = rem_angle / 90.0 * (MEDIAN_LUNAR_MONTH / 4.0);
    let jd = if extra_jd < 0.0005 { ref_jd + extra_jd } else { ref_jd };
    MoonPhase {
      jd,
      utc: julian_day_to_iso_datetime(jd),
      angle,
      waxing: num <= 2,
      num,
      days
    }
  }
}

pub fn calc_sun_moon_angle(moon_lng: f64, sun_lng: f64) -> (f64, bool, u8) {
  let angle = sun_lng.angle_360_abs(moon_lng);
  let waxing = angle <= 180f64;
  let phase = (angle / 90.0).floor() as u8 + 1;
  (angle, waxing, phase)
}

pub fn calc_sun_moon_angle_and_phase(jd: f64, geo: GeoPos) -> (f64, bool, u8, f64) {
  let su = calc_body_jd_topo(jd, "su", geo, 0.0);
  let mo = calc_body_jd_topo(jd, "mo", geo, 0.0);
  let spd = (su.lng_speed + mo.lng_speed) / 2.0;
  let (a, b ,c) = calc_sun_moon_angle(mo.lng, su.lng);
  (a, b ,c, spd)
}

fn adjusted_micro_increment(rem_val: f64, increment: &f64) -> f64 {
  if rem_val < 0.0078125 {
    1.0 / 15360.0
  } else if rem_val < 0.015625 {
    1.0 / 3840.0
  } else if rem_val < 0.03125 {
    1.0 / 3840.0
  } else  if rem_val < 0.0625 {
    1.0 / 1920.0
  } else if rem_val < 0.125 {
    1.0 / 960.0
  } else if rem_val < 0.25 {
    1.0 / 480.0
  } else if rem_val < 0.50 {
    1.0 / 240.0
  } else if rem_val < 1.0 {
    1.0 / 96.0
  } else if rem_val < 2.0 {
    1.0 / 48.0
  } else if rem_val < 4.0 {
    1.0 / 24.0
  } else {
    increment.to_owned().clone()
  }
}

pub fn calc_next_phase(start_jd_target: f64, geo: GeoPos, current_angle: f64, phase: u8, prev_jd: f64) -> MoonPhase {
  let mut rem_val = 90.0 - (current_angle % 90.0);
  let mut increment = 1.0 / 12.0;
  let mut counter = 0;
  let mut nx_jd = 0.0;
  let mut nx_angle = 0.0;
  // let mut nx_speed = 0.0;
  let next_phase = if phase < 4 { phase + 1 } else { 1 };
  let min_tolerance = 0.005;

  let mut next_jd_target = start_jd_target + increment;
  while rem_val > min_tolerance {
    let (next_angle, _waxing, _next_phase, _speed) = calc_sun_moon_angle_and_phase(next_jd_target, geo);
    rem_val = 90.0 - (next_angle % 90.0);
    increment = adjusted_micro_increment(rem_val, &increment);
    if rem_val < min_tolerance {
      nx_angle = next_angle;
      nx_jd = next_jd_target;
      //nx_speed = speed;
    }
    next_jd_target += increment;
    counter += 1;
    if counter > 500 {
      break;
    }
  }
  let days_f64 = nx_jd - prev_jd;
  let days = if days_f64 < SHORT_LUNAR_MONTH { Some(days_f64 )} else { None };
  MoonPhase::new(nx_jd, nx_angle, next_phase, days)
}

pub fn calc_moon_phases(jd: f64, geo: GeoPos, cycles: u8) -> Vec<MoonPhase> {
  let mut phases: Vec<MoonPhase> = Vec::new();
  let (current_angle, _w, phase, _speed) = calc_sun_moon_angle_and_phase(jd, geo);
  let next_target_angle = phase as f64 * 90.0;
  let distance = next_target_angle - current_angle;
  let min_lunar_month_quarter = SHORT_LUNAR_MONTH / 4.0;
  let progress = distance / 90.0;
  let mut start_jd_target = jd + (min_lunar_month_quarter * progress);
  let next_phase = calc_next_phase(start_jd_target, geo, current_angle, phase, 0.0);
  let mut phase = next_phase.num;
  phases.push(next_phase);
  let min_cycles = if cycles < 1 { 1 } else { cycles } as usize;
  let num_extra_phases = 3 + ((min_cycles - 1) * 4);
  
  for _index in 0..num_extra_phases {
    let prev_jd = start_jd_target;
    start_jd_target = prev_jd + min_lunar_month_quarter;
    let ref_angle = (phase as f64 * 90.0) % 360.0;
    let next_phase = calc_next_phase(start_jd_target, geo, ref_angle, phase, prev_jd);
    phase = next_phase.num;
    start_jd_target = next_phase.jd;
    phases.push(next_phase);
  }
  phases
}