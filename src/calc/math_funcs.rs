pub fn calc_opposite(lng: f64) -> f64 {
  (lng + 180f64) % 360f64
}

pub fn adjust_lng_by_body_key(key: &str, lng: f64) -> f64 {
  match key {
    "ke" => calc_opposite(lng),
    _ => lng,
  }
}

pub fn subtract_360(lng: f64, offset: f64) -> f64 {
  (lng + 360f64 - offset) % 360f64
}

pub fn normalize_360(lng: f64) -> f64 {
  (lng + 360f64) % 360f64
}

pub fn normalize_f64(lng: f64, base: i16) -> f64 {
  let base_f64 = base as f64;
  (lng + base_f64) % base_f64
}

pub fn recalc_houses(positions: Vec<f64>, ayanamsha: f64, ascendant: Option<f64>, system: Option<char>) -> Vec<f64> {
  let is_whole = match system {
    Some('W') | None => true,
    _ => false
  };
  let has_positions = positions.len() > 0;
  let mut offsets: Vec<f64> = vec![];
  if is_whole {
    let asc_lng = match ascendant {
      Some(lng) => lng,
      _ => match has_positions {
        true => positions.get(0).unwrap_or(&0f64).to_owned(),
        _ => 0f64,
      }
    };
    let start = (subtract_360(asc_lng, ayanamsha) / 30f64).floor() * 30f64;
    for i in 0..12 {
      let next_deg = (start + (i as f64 * 30f64)) % 360f64;
      offsets.push(next_deg);
    }
  } else if has_positions {
    offsets = positions.into_iter().map(|p| subtract_360(p, ayanamsha)).collect();
  }
  offsets
}

/* pub fn recalc_houses_whole(ascendant: f64, ayanamsha: f64) -> Vec<f64> {
  recalc_houses(vec![], ayanamsha, Some(ascendant), Some('W'))
}
 */
pub fn recalc_houses_by_system(positions: Vec<f64>, ayanamsha: f64, system: char) -> Vec<f64> {
  recalc_houses(positions, ayanamsha, None, Some(system))
}

