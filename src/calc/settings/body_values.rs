use libswe_sys::sweconst::Bodies;
use crate::calc::traits::*;

impl FromKey<Bodies> for Bodies {
  fn from_key(key: &str) -> Bodies {
    let simple_key = key.to_lowercase();
    match simple_key.as_str() {
      "su" => Bodies::Sun,
      "mo" => Bodies::Moon,
      "me" => Bodies::Mercury,
      "ve" => Bodies::Venus,
      "ea" => Bodies::Earth,
      "ma" => Bodies::Mars,
      "ju" => Bodies::Jupiter,
      "sa" => Bodies::Saturn,
      "ne" => Bodies::Neptune,
      "ur" => Bodies::Uranus,
      "pl" => Bodies::Pluto,
      "ke" | "ra" => Bodies::TrueNode,
      "mn" => Bodies::MeanNode,
      "kr" => Bodies::Kronos,
      "is" => Bodies::Isis,
      "jn" => Bodies::Juno,
      "ce" => Bodies::Ceres,
      "ch" => Bodies::Chiron,
      "sn" => Bodies::SouthNode,
      _ => Bodies::Earth,
    }
  }
}

impl ToKey<Bodies> for Bodies {

  fn to_key(&self) -> &str {
    match self {
      Bodies::Sun => "su",
      Bodies::Moon => "mo",
      Bodies::Mercury => "me",
      Bodies::Venus => "ve",
      Bodies::Earth => "ea",
      Bodies::Mars => "ma",
      Bodies::Jupiter => "ju",
      Bodies::Saturn => "sa",
      Bodies::Neptune => "ne",
      Bodies::Uranus => "ur",
      Bodies::Pluto => "pl",
      Bodies::TrueNode => "ra",
      Bodies::MeanNode => "mn",
      Bodies::Kronos => "kr",
      Bodies::Isis => "is",
      Bodies::Juno => "jn",
      Bodies::Ceres => "ce",
      Bodies::Chiron => "ch",
      Bodies::SouthNode => "sn",
      _ => "ea",
    }
  }
}

pub trait PlanetNum {
  fn to_num(&self) -> u16;
  fn from_num(num: u16) -> Self;
}

impl PlanetNum for Bodies {
  fn to_num(&self) -> u16 {
    *self as u16
  }

  fn from_num(num: u16) -> Self {
    Bodies::from_key(planet_num_to_graha_key(num))
  }

}

/*
* convert body number to key
*/
pub fn planet_num_to_graha_key(num: u16) -> &'static str {
  match num {
    0 => "su",
    1 => "mo",
    2 => "me",
    3 => "ve",
    4 => "ma",
    5 => "ju",
    6 => "sa",
    7 => "ur",
    8 => "ne",
    9 => "pl",
    //101 => "ra",
    11 => "ra",
    102 => "ke",
    _ => ""
  }
}


