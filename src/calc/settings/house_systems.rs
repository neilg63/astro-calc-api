use std::collections::HashMap;

pub const HOUSE_SYSTEMS: [(char, &'static str); 24] = [
  ('A', "equal"),
  ('B', "Alcabitius"),
  ('C', "Campanus"),
  ('D', "equal (MC)"),
  ('E', "equal"),
  ('F', "Carter poli-equ."),
  ('G', "Gauquelin sectors"),
  ('H', "horizon/azimut"),
  ('I', "Sunshine"),
  ('J', "Sunshine/alt."),
  ('K', "Koch"),
  ('L', "Pullen SD"),
  ('M', "Morinus"),
  ('N', "equal/1=Aries"),
  ('O', "Porphyry"),
  ('Q', "Pullen SR"),
  ('R', "Regiomontanus"),
  ('S', "Sripati"),
  ('T', "Polich/Page"),
  ('U', "Krusinski-Pisa-Goelzer"),
  ('V', "equal/Vehlow"),
  ('W', "equal/ whole sign"),
  ('X', "axial rotation system/Meridian houses"),
  ('Y', "APC houses"),
];

pub fn houses_as_key_map() -> HashMap<char, String> {
  let mut hm: HashMap<char, String> = HashMap::new();
  for pair in HOUSE_SYSTEMS {
    hm.insert(pair.0, pair.1.to_string());
  }
  hm
}