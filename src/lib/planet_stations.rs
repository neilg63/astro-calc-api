use ::serde::{Serialize, Deserialize};
use super::{julian_date::julian_day_to_iso_datetime, data::body_speeds::*};

pub enum PlanetStation {
	RetroStart = 0,
	RetroPeak = 1,
	RetroEnd = 2,
	Peak = 3,
	Sample = 4
}

impl PlanetStation {
	fn new(num: u8) -> PlanetStation {
		match num {
			0 => PlanetStation::RetroStart,
			1 => PlanetStation::RetroPeak,
			2 => PlanetStation::RetroEnd,
			3 => PlanetStation::Peak,
			_ => PlanetStation::Sample,
		}
	}

	fn as_string(&self) -> String {
		match self {
			PlanetStation::RetroStart => "retro-start",
			PlanetStation::RetroPeak => "retro-peak",
			PlanetStation::RetroEnd => "retro-end",
			PlanetStation::Peak => "peak",
			_ => "sample",
		}.to_string()
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlanetSample {
	pub jd: f64,
	#[serde(skip_serializing_if = "String::is_empty")]
	pub dt: String,
	pub lng: f64,
	pub speed: f64,
	pub r#type: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BodySpeed {
	pub key: String,
	pub jd: f64,
	pub lng: f64,
	pub speed: f64,
	pub station: String
}

impl BodySpeed {
	pub fn new(key: &str, jd: f64, lng: f64, speed: f64, station: u8) -> BodySpeed {
		BodySpeed{
			key: key.to_string(),
			jd,
			lng,
			speed,
      station: PlanetStation::new(station).as_string()
		}
	}

	pub fn as_sample(&self, iso_mode: bool) -> PlanetSample {
		let dt = if iso_mode { julian_day_to_iso_datetime(self.jd) } else { "".to_string() };
		PlanetSample {
			jd: self.jd,
			dt,
			lng: self.lng,
			speed: self.speed,
			r#type: self.station.clone(),
		}
	}
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BodySpeedSet {
	pub key: String,
	pub stations: Vec<PlanetSample>
}

impl BodySpeedSet {
	pub fn new(key: &str, body_speeds: Vec<BodySpeed>, iso_mode: bool) -> BodySpeedSet {
		BodySpeedSet{
			key: key.to_string(),
      stations: body_speeds.into_iter().map(|bs| bs.as_sample(iso_mode)).collect(),
		}
	}
}

pub fn match_planet_stations_range(key: &str, ref_jd: f64, end_jd: f64) -> Vec<BodySpeed> {
    let mut items:Vec<BodySpeed> = vec![];
	let ref_rows:Vec<(f64, f64, f64, u8)> = match key {
        "me" => PLANETARY_STATIONS_ME,
        "ve" => PLANETARY_STATIONS_VE,
        "ma" => PLANETARY_STATIONS_MA,
        "ju" => PLANETARY_STATIONS_JU,
        "sa" => PLANETARY_STATIONS_SA,
        "ur" => PLANETARY_STATIONS_UR,
        "ne" => PLANETARY_STATIONS_NE,
				"pl" => PLANETARY_STATIONS_PL,
        _ => PLANETARY_STATIONS_EA,
    }.iter().cloned().collect();
		// NB data sources are in descending chronological order
    let num_rows = ref_rows.len();
			if ref_rows.len() > 0 {
			let target_jd = if end_jd < ref_jd { ref_jd } else { end_jd };
			let index_match = ref_rows.clone().into_iter().position(|r| r.0 <= target_jd);
			if let Some(index) = index_match {
					let start_pos = index as i32 - 4i32;
					
					let start_index = if start_pos < 0 { 0usize } else { start_pos as usize };
					let mut end_index = index + 4;
					if end_jd > ref_jd {
						let end_index_match = ref_rows.clone().into_iter().position(|r| r.0 < ref_jd);
						if let Some(end_im) = end_index_match {
							end_index = end_im;
						} else {
							end_index = num_rows - 1;
						}
					}
					if end_index >= num_rows {
							end_index = num_rows - 1;
					}
					for i in start_index..end_index {
							if let Some(row) = ref_rows.get(i) {
									items.push(BodySpeed::new(key, row.0, row.1, row.2, row.3));
							}
					}
			}
		}
    items.into_iter().rev().collect()
}

pub fn match_nextprev_planet_stations(key: &str, ref_jd: f64) -> Vec<BodySpeed> {
	match_planet_stations_range(key, ref_jd, 0f64)
}

pub fn match_all_nextprev_planet_stations(ref_jd: f64, bodies: Vec<&str>, iso_mode: bool) -> Vec<BodySpeedSet> {
	let mut items: Vec<BodySpeedSet> = vec![];
	for key in bodies {
		let rows = match_nextprev_planet_stations(key, ref_jd);
		items.push(BodySpeedSet::new(key, rows, iso_mode));
	}
	items
}

pub fn match_all_planet_stations_range(ref_jd: f64, end_jd: f64, bodies: Vec<&str>, iso_mode: bool) -> Vec<BodySpeedSet> {
	let mut items: Vec<BodySpeedSet> = vec![];
	for key in bodies {
		let rows = match_planet_stations_range(key, ref_jd, end_jd);
		items.push(BodySpeedSet::new(key, rows, iso_mode));
	}
	items
}