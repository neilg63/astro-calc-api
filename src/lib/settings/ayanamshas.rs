use serde::{Serialize, Deserialize};
use std::fmt::{Display, Formatter, Result};
use super::super::traits::*;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Ayanamsha {
  Tropical = 0,
  TrueCitra = 27,
  Lahiri = 1,
  Krishnamurti = 5,
  Yukteshwar = 7,
  Raman = 3,
  ValensMoon = 42,
  TrueMula = 35,
  TrueRevati = 28,
  TruePushya = 29,
  TrueSheoran = 39,
  Aldebaran15Tau = 14,
  GalcentMulaWilhelm = 36,
  GalcentCochrane = 40,
  Hipparchos = 15,
  Sassanian = 16,
  Ushashashi = 4,
  JnBhasin = 8,
}

impl Ayanamsha {
  
  pub fn as_string(&self) -> String {
    match self {
      Ayanamsha::TrueCitra => "true_citra",
      Ayanamsha::Lahiri => "lahiri",
      Ayanamsha::Krishnamurti => "krishnamurti",
      Ayanamsha::Yukteshwar => "yukteshwar",
      Ayanamsha::Raman => "raman",
      Ayanamsha::ValensMoon => "valensmoon",
      Ayanamsha::TrueMula => "true_mula",
      Ayanamsha::TrueRevati => "true_revati",
      Ayanamsha::TruePushya => "true_pushya",
      Ayanamsha::TrueSheoran => "true_sheoran",
      Ayanamsha::Aldebaran15Tau => "aldebaran_15_tau",
      Ayanamsha::GalcentMulaWilhelm => "galcent_mula_wilhelm",
      Ayanamsha::GalcentCochrane => "galcent_cochrane",
      Ayanamsha::Hipparchos => "hipparchos",
      Ayanamsha::Sassanian => "sassanian",
      Ayanamsha::Ushashashi => "ushashashi",
      Ayanamsha::JnBhasin => "jnbhasin",
      _ => "tropical",
    }.to_string()
  }

  pub fn as_i32(self) -> i32 {
    self as i32
  }

  pub fn as_u8(self) -> u8 {
    self as u8
  }

  fn from_num(num: u8) -> Self {
    match num {
      27 => Ayanamsha::TrueCitra,
      1 => Ayanamsha::Lahiri,
      5 => Ayanamsha::Krishnamurti,
      7 => Ayanamsha::Yukteshwar,
      3 => Ayanamsha::Raman,
      42 => Ayanamsha::ValensMoon,
      35 => Ayanamsha::TrueMula,
      28 => Ayanamsha::TrueRevati,
      29 => Ayanamsha::TruePushya,
      39 => Ayanamsha::TrueSheoran,
      14 => Ayanamsha::Aldebaran15Tau,
      36 => Ayanamsha::GalcentMulaWilhelm,
      40 => Ayanamsha::GalcentCochrane,
      15 => Ayanamsha::Hipparchos,
      16 => Ayanamsha::Sassanian,
      4 => Ayanamsha::Ushashashi,
      8 => Ayanamsha::JnBhasin,
      _ => Ayanamsha::Tropical,
    }
  }

}

impl Display for Ayanamsha {
  fn fmt(&self, f: &mut Formatter) -> Result {
    write!(f, "{}", self.as_string())
  } 
}

impl FromKey<Ayanamsha> for Ayanamsha {
  fn from_key(key: &str) -> Self {
    let simple_str = key.to_lowercase().replace("_", "");
    let is_numeric = simple_str.chars().into_iter().all(|s| s.is_numeric());
    let mut num: u8 = 0;
    if is_numeric {
      if let Ok(n) = simple_str.parse::<u8>() {
        if n < 43 {
          num = n;
        }
      }
    }
    if is_numeric && num > 0 {
      Ayanamsha::from_num(num)
    } else {
      match simple_str.as_str() {
        "tc" | "truecitra" | "true_citra" | "citra" | "chitra" => Ayanamsha::TrueCitra,
        "lh" | "lahiri" => Ayanamsha::Lahiri,
        "kr" | "krishnamurti" => Ayanamsha::Krishnamurti,
        "yu" | "yukteshwar" => Ayanamsha::Yukteshwar,
        "ra" | "raman" => Ayanamsha::Raman,
        "vm" | "valensmoon" => Ayanamsha::ValensMoon,
        "tm" | "truemula" => Ayanamsha::TrueMula,
        "tr" | "truerevati" => Ayanamsha::TrueRevati,
        "tp" | "truepushya" | "pushya" => Ayanamsha::TruePushya,
        "ts" | "truesheoran" => Ayanamsha::TrueSheoran,
        "at" | "aldebaran15tau" => Ayanamsha::Aldebaran15Tau,
        "gm" | "galcenmulawilhelm" => Ayanamsha::GalcentMulaWilhelm,
        "gc" | "galcentcochrane" => Ayanamsha::GalcentCochrane,
        "hi" | "hipparchos" => Ayanamsha::Hipparchos,
        "sa" | "sassanian" => Ayanamsha::Sassanian,
        "us" | "ushashashi" => Ayanamsha::Ushashashi,
        "jb" | "jnbhasin" => Ayanamsha::JnBhasin,
        _ => Ayanamsha::Tropical,
      }
    }
  }

}

impl ToKey<Ayanamsha> for Ayanamsha {
  fn to_key(&self) -> &str {
    match self {
      Ayanamsha::TrueCitra => "tc",
      Ayanamsha::Lahiri => "lh",
      Ayanamsha::Krishnamurti => "kr",
      Ayanamsha::Yukteshwar => "yu",
      Ayanamsha::Raman => "ra",
      Ayanamsha::ValensMoon => "vm",
      Ayanamsha::TrueMula => "tm",
      Ayanamsha::TrueRevati => "tr",
      Ayanamsha::TruePushya => "tp",
      Ayanamsha::TrueSheoran => "ts",
      Ayanamsha::Aldebaran15Tau => "at",
      Ayanamsha::GalcentMulaWilhelm => "gm",
      Ayanamsha::GalcentCochrane => "gc",
      Ayanamsha::Hipparchos => "hi",
      Ayanamsha::Sassanian => "sa",
      Ayanamsha::Ushashashi => "us",
      Ayanamsha::JnBhasin => "jb",
      _ => ""
    }
  }
}

pub fn all_ayanamsha_keys() -> Vec<&'static str> {
  vec![
    "true_citra",
    "lahiri",
    "krishnamurti",
    "yukteshwar",
    "raman",
    "valensmoon",
    "true_mula",
    "true_revati",
    "true_pushya",
    "true_sheoran",
    "aldebaran_15_tau",
    "galcent_mula_wilhelm",
    "galcent_cochrane",
    "hipparchos",
    "sassanian",
    "ushashashi",
    "jnbhasin",
  ]
}

pub fn match_ayanamsha_key(key: &str) -> String {
  let ref_key = if let Some(key_start) = key.split(",").nth(0) {
    key_start
  } else {
    key
  };
  Ayanamsha::from_key(ref_key).as_string()
}

pub fn match_ayanamsha_num(key: &str) -> u8 {
  Ayanamsha::from_key(key).as_u8()
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AynamshaInfo {
  name: String,
  key: String,
  num: u16,
}


impl AynamshaInfo {
  pub fn new(key: &str) -> Self {
    let aya = Ayanamsha::from_key(key);
    let name = aya.as_string();
    let key = aya.to_key();
    let num = aya as u16;
    AynamshaInfo { name, key: key.to_string(), num }
  }
}