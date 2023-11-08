use crate::calc::{dates::*, traits::*};
use crate::calc::models::general::{LngLat, LngLatKey, ToLngLat, ToLngLatKey, CoordinateSystem};
use libswe_sys::swerust::handler_swe07::PhenoUtResult;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BodyPos {
  pub key: String,
  pub lng: f64,
  pub lat: f64,
  #[serde(rename = "lngSpeed")]
  pub lng_speed: f64,
  #[serde(rename = "latSpeed")]
  pub lat_speed: f64,
  #[serde(skip_serializing)]
  pub mode: String,
}

impl BodyPos {
  pub fn new(key: &str, mode: CoordinateSystem, lng: f64, lat: f64, lng_speed: f64, lat_speed: f64) -> Self {
    BodyPos {
      key: key.to_string(),
      mode: mode.to_key(),
      lng: lng,
      lat: lat,
      lng_speed: lng_speed,
      lat_speed: lat_speed,
    }
  }

  pub fn empty() -> Self {
    BodyPos {
      key: "".to_string(),
      mode: "ec".to_string(),
      lng: 0f64,
      lat: 0f64,
      lng_speed: 0f64,
      lat_speed: 0f64,
    }
  }

}

impl ToLngLat for BodyPos {
  fn to_lng_lat(&self) -> LngLat {
    LngLat {
      lng: self.lng,
      lat: self.lat,
    }
  }
}

impl ToLngLatKey for BodyPos {
  fn to_lng_lat_key(&self) -> LngLatKey {
    LngLatKey {
      key: self.key.clone(),
      lng: self.lng,
      lat: self.lat,
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PhenoResult {
  #[serde(rename = "phaseAngle")]
  pub phase_angle: f64,
  #[serde(rename = "phaseIlluminated")]
  pub phase_illuminated: f64,
  #[serde(rename = "elongationOfPlanet")]
  pub elongation_of_planet: f64,
  #[serde(rename = "apparentDiameterOfDisc")]
  pub apparent_diameter_of_disc: f64,
  #[serde(rename = "apparentMagnitude")]
  pub apparent_magnitude: f64,
}

impl PhenoResult {

  pub fn new_from_result(result: PhenoUtResult) -> PhenoResult {
    PhenoResult {
      phase_angle: result.phase_angle,
      phase_illuminated: result.phase_illuminated,
      elongation_of_planet: result.elongation_of_planet,
      apparent_diameter_of_disc: result.apparent_dimaeter_of_disc,
      apparent_magnitude: result.apparent_magnitude,
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PhenoItem {
  pub key: String,
  #[serde(rename = "phaseAngle")]
  pub phase_angle: f64,
  #[serde(rename = "phaseIlluminated")]
  pub phase_illuminated: f64,
  #[serde(rename = "elongationOfPlanet")]
  pub elongation_of_planet: f64,
  #[serde(rename = "apparentDiameterOfDisc")]
  pub apparent_diameter_of_disc: f64,
  #[serde(rename = "apparentMagnitude")]
  pub apparent_magnitude: f64,
}

impl PhenoItem {

  pub fn new_from_result(key: &str, result: PhenoUtResult) -> PhenoItem {
    PhenoItem {
      key: key.to_string(),
      phase_angle: result.phase_angle,
      phase_illuminated: result.phase_illuminated,
      elongation_of_planet: result.elongation_of_planet,
      apparent_diameter_of_disc: result.apparent_dimaeter_of_disc,
      apparent_magnitude: result.apparent_magnitude,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GrahaPos {
  pub key: String,
  pub lng: f64,
  pub lat: f64,
  #[serde(rename = "lngSpeed")]
  pub lng_speed: f64,
  #[serde(rename = "latSpeed")]
  pub lat_speed: f64,
  #[serde(rename = "rectAscension")]
  pub rect_ascension: f64,
  pub declination: f64,
  #[serde(rename = "lngSpeedEq")]
  pub lng_speed_eq: f64,
  #[serde(rename = "latSpeedEq")]
  pub lat_speed_eq: f64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pheno: Option<PhenoResult>,
  #[serde(skip_serializing_if = "Option::is_none")]
  altitude: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  azimuth: Option<f64>,
}

impl GrahaPos {
  /**
   * Default constructor for the ecliptic coordinate system without equatorial coordinates
   * The lng/lat speeds are ecliptic
   */
  pub fn new(key: &str, lng: f64, lat: f64, lng_speed: f64, lat_speed: f64) -> Self {
    GrahaPos {
      key: key.to_string(),
      lng,
      lat,
      lng_speed,
      lat_speed,
      rect_ascension: 0f64,
      declination: 0f64,
      lng_speed_eq: 0f64,
      lat_speed_eq: 0f64,
      pheno: None,
      altitude: None,
      azimuth: None,
    }
  }



  /**
   * Default constructor for the equatorial coordinate system without ecliptic coordinates
   * The lng/lat speeds are equatorial
   */
  pub fn new_eq(
    key: &str,
    rect_ascension: f64,
    declination: f64,
    lng_speed: f64,
    lat_speed: f64,
  ) -> Self {
    GrahaPos {
      key: key.to_string(),
      lng: 0f64,
      lat: 0f64,
      lng_speed: 0f64,
      lat_speed: 0f64,
      rect_ascension,
      declination,
      lng_speed_eq: lng_speed,
      lat_speed_eq: lat_speed,
      pheno: None,
      altitude: None,
      azimuth: None,
    }
  }

  pub fn new_extended(
    key: &str,
    lng: f64,
    lat: f64,
    rect_ascension: f64,
    declination: f64,
    lng_speed: f64,
    lat_speed: f64,
    lng_speed_eq: f64,
    lat_speed_eq: f64,
    pheno: Option<PhenoResult>,
    altitude: Option<f64>,
    azimuth: Option<f64>,
  ) -> Self {
    GrahaPos {
      key: key.to_string(),
      lng,
      lat,
      lng_speed,
      lat_speed,
      rect_ascension,
      declination,
      lng_speed_eq,
      lat_speed_eq,
      pheno,
      altitude,
      azimuth,
    }
  }

  pub fn to_body(&self, mode: CoordinateSystem) -> BodyPos {
    let lng = match mode {
      CoordinateSystem::Equatorial => self.rect_ascension,
      CoordinateSystem::Horizontal => self.azimuth.unwrap_or(0f64),
      _ => self.lng,
    };
    let lat = match mode {
      CoordinateSystem::Equatorial => self.declination,
      CoordinateSystem::Horizontal => self.altitude.unwrap_or(0f64),
      _ => self.lat,
    };
    let lng_speed = match mode {
      CoordinateSystem::Equatorial => self.lng_speed_eq,
      _ => self.lng_speed,
    };
    let lat_speed = match mode {
      CoordinateSystem::Equatorial => self.lat_speed_eq,
      _ => self.lat_speed,
    };
    BodyPos::new(self.key.as_str(), mode, lng, lat, lng_speed, lat_speed)
  }
}

impl MatchVecKey<GrahaPos> for Vec<GrahaPos> {
    fn match_by_key(&self, key: &str) -> Option<GrahaPos> {
        self.to_owned().into_iter().find(|row| row.to_owned().key.as_str() == key)
    }
}

impl ToLngLat for GrahaPos {
  fn to_lng_lat(&self) -> LngLat {
    LngLat {
      lng: self.lng,
      lat: self.lat,
    }
  }
}

impl ToLngLatKey for GrahaPos {
  fn to_lng_lat_key(&self) -> LngLatKey {
    LngLatKey {
      key: self.key.clone(),
      lng: self.lng,
      lat: self.lat,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GrahaPosItem {
  pub jd: f64,
  pub position: GrahaPos,
}

/* impl GrahaPosItem {
  pub fn new(jd: f64, pos: GrahaPos) -> GrahaPosItem {
    GrahaPosItem { jd, position: pos }
  }
} */

impl ToISODateString for GrahaPosItem {
  fn iso_date_string(&self) -> String {
    julian_day_to_iso_datetime(self.jd)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GrahaPosSet {
  pub jd: f64,
  #[serde(skip_serializing_if = "String::is_empty")]
  pub dt: String,
  pub bodies: Vec<BodyPos>,
}

impl GrahaPosSet {
  pub fn new(jd: f64, bodies: Vec<GrahaPos>, mode: CoordinateSystem, iso: bool) -> GrahaPosSet {
    let dt = if iso {
      julian_day_to_iso_datetime(jd)
    } else {
      "".to_string()
    };
    GrahaPosSet {
      jd,
      dt,
      bodies: bodies.into_iter().map(|g| g.to_body(mode)).collect(),
    }
  }
}

impl ToISODateString for GrahaPosSet {
  fn iso_date_string(&self) -> String {
    julian_day_to_iso_datetime(self.jd)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum FlexiBodyPos {
  LngLat(Vec<LngLat>),
  LngLatKey(Vec<LngLatKey>),
  Simple(Vec<BodyPos>),
  Extended(Vec<GrahaPos>),
}
