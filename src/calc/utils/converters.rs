use julian_day_converter::julian_day_to_datetime;
use crate::calc::models::geo_pos::GeoPos;

pub fn to_str_refs(strings: &Vec<String>) -> Vec<&str> {
  let strs: Vec<&str> = strings.iter().map(|s| s.as_ref()).collect();
  strs
}

pub fn body_keys_str_to_keys(key_string: String) -> Vec<String> {
  key_string.split(",").into_iter().filter(|p| p.len() == 2).map(|p| p.to_string()).collect()
}

pub fn body_keys_str_to_keys_or(key_string: String, default_keys: Vec<&str>) -> Vec<String> {
  let keys: Vec<String> = body_keys_str_to_keys(key_string);
  if keys.len() > 0 { keys.into_iter().filter(|k| k.len() == 2 && !k.contains("as")).collect() } else { default_keys.into_iter().map(|p| p.to_string() ).collect() }
}

pub fn loc_string_to_geo(loc: &str) -> Option<GeoPos> {
  let parts: Vec<f64> = loc.split(",").into_iter().map(|p| p.parse::<f64>()).filter(|p| match p { Ok(_n) => true, _ => false } ).map(|p| p.unwrap()).collect();
  if parts.len() >= 2 {
    let alt = if parts.len() > 2 { parts[2] } else { 0f64 };
    Some(GeoPos::new(parts[0], parts[1], alt))
  } else {
    None
  }
}

pub fn calc_circular_diff(ref_int: i32, target_int: i32, base: i32) -> i32 {
  if ref_int < target_int {
		target_int - ref_int
	} else {
		target_int + base - ref_int
	}
}

pub fn calc_days_to_next_prev_equinox(jd: f64) -> i32 {
  let yd = if let Ok(dt) = julian_day_to_datetime(jd) { dt.format("%j").to_string().parse::<i32>().unwrap_or(0) } else { 0 };
  let half_year_days = 183;
  let autumn_eq = 266;
  let spring_eq = autumn_eq - half_year_days;
  let first = calc_circular_diff(yd, spring_eq, 366);
  let mut second = half_year_days * 2;
  if first >= half_year_days {
    second = calc_circular_diff(yd, autumn_eq, 365);
  }
  if first < second {
    first
  } else {
    second
  }
}

fn logarithmic_progress_to_pole(lat: f64) -> f64 {
	let pc = 200f64/3f64;
	let abs_lng = lat.abs();
	if abs_lng > pc {
    let linear_val = 1f64 - (90f64 - abs_lng) / (90f64 - pc);
		return linear_val.powf(0.5f64);
	} else {
		return 0f64;
	}
}

fn subtract_to_zero(num: i32, sub_num: i32) -> i32 {
  let new_val = num - sub_num;
  if new_val > 0 {
    new_val
  } else {
    0
  }
}

pub fn get_next_prev_rise_set_polar_calc_offset(ref_jd: f64, lat: f64) -> (i32, i32) {
  let subtract_val = 18;
  let next_equinox = calc_days_to_next_prev_equinox(ref_jd);
  let min_progress = min_progress_to_end_of_light_period(lat);
  let counter_next = (next_equinox as f64 * min_progress).floor() as i32;
  let counter_prev = ((183 - next_equinox) as f64 * min_progress).floor() as i32;
  (subtract_to_zero(counter_next, subtract_val), subtract_to_zero(counter_prev, subtract_val))
}

pub fn min_progress_to_end_of_light_period(lat: f64) -> f64 {
	(logarithmic_progress_to_pole(lat) * 120f64).floor() / 183f64
}

pub fn time_interval_format(increment: f64) -> String {
  let hours_f64 = increment * 24f64;
  let mins_f64 = (hours_f64 % 1.0) * 60.0;
  let secs_f64 = (mins_f64 % 1.0) * 60.0;
  let hrs = hours_f64.floor() as u32;
  let mut parts = vec![];
  if hrs > 0 {
    parts.push(format!("{}h", hrs));
  }
  if mins_f64 > 0.0 {
    let mins = mins_f64.floor() as u8; 
    parts.push(format!("{}m", mins));
    let secs = secs_f64.floor() as u8; 
    if secs > 0 {
      parts.push(format!("{}s", secs));
    }
  }
  parts.join(" ")
}

pub fn calc_angle(lng1: f64, lng2: f64) -> f64 {
  (lng1 + 360f64 - lng2) % 360f64
}

pub fn calc_sun_moon_angle(moon_lng: f64, sun_lng: f64) -> (f64, bool, u8) {
  let angle = calc_angle(moon_lng, sun_lng);
  let waxing = angle <= 180f64;
  let phase = (angle / 90.0).floor() as u8 + 1;
  (angle, waxing, phase)
}