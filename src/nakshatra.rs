//! Nakshatras: the 27 lunar mansions, equal 13°20′ divisions of the sidereal zodiac from
//! 0° Aries, each of four padas of 3°20′.

use serde_json::{Value, json};

/// Degrees in one nakshatra.
pub const SPAN: f64 = 360.0 / 27.0;

const NAMES: [&str; 27] = [
    "Ashwini",
    "Bharani",
    "Krittika",
    "Rohini",
    "Mrigashira",
    "Ardra",
    "Punarvasu",
    "Pushya",
    "Ashlesha",
    "Magha",
    "Purva Phalguni",
    "Uttara Phalguni",
    "Hasta",
    "Chitra",
    "Swati",
    "Vishakha",
    "Anuradha",
    "Jyeshtha",
    "Mula",
    "Purva Ashadha",
    "Uttara Ashadha",
    "Shravana",
    "Dhanishta",
    "Shatabhisha",
    "Purva Bhadrapada",
    "Uttara Bhadrapada",
    "Revati",
];

/// The rulers in Vimshottari order, as body keys; the sequence runs three times through the 27.
const LORDS: [&str; 9] = ["ke", "ve", "su", "mo", "ma", "ra", "ju", "sa", "me"];

/// Where a sidereal longitude falls among the nakshatras.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nakshatra {
    /// 1 (Ashwini) to 27 (Revati).
    pub num: u8,
    /// The quarter within it, 1 to 4.
    pub pada: u8,
    /// Degrees travelled within it, from 0 up to 13°20′.
    pub degrees: f64,
}

impl Nakshatra {
    pub fn from_sidereal(lng: f64) -> Self {
        let lng = lng.rem_euclid(360.0);
        // Multiplying first keeps the boundaries exact: 180° is quarter 54 of 108, not 53.99….
        let quarter = ((lng * 108.0 / 360.0) as u8).min(107);
        let index = quarter / 4;
        let degrees = (lng - index as f64 * SPAN).clamp(0.0, SPAN);
        Self { num: index + 1, pada: quarter % 4 + 1, degrees }
    }

    pub fn name(self) -> &'static str {
        NAMES[(self.num - 1) as usize]
    }

    /// The ruling body's key.
    pub fn lord(self) -> &'static str {
        LORDS[((self.num - 1) % 9) as usize]
    }

    /// `num`, `name`, `pada`, `lord`, and how far through it the longitude is: `degrees`, and
    /// `fraction` from 0 to 1.
    pub fn json(self) -> Value {
        json!({
            "num": self.num, "name": self.name(), "pada": self.pada, "lord": self.lord(),
            "degrees": self.degrees, "fraction": self.degrees / SPAN,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divisions_names_and_lords() {
        let at = |lng: f64| Nakshatra::from_sidereal(lng);
        assert_eq!((at(0.0).num, at(0.0).pada, at(0.0).name(), at(0.0).lord()), (1, 1, "Ashwini", "ke"));
        assert_eq!((at(13.33).num, at(13.33).pada), (1, 4));
        assert_eq!((at(13.34).num, at(13.34).pada, at(13.34).name(), at(13.34).lord()), (2, 1, "Bharani", "ve"));
        // Spica at 180° sidereal is the middle of Chitra.
        let chitra = at(180.0);
        assert_eq!((chitra.num, chitra.pada, chitra.name(), chitra.lord()), (14, 3, "Chitra", "ma"));
        assert!((chitra.degrees - SPAN / 2.0).abs() < 1e-9);
        assert_eq!((at(359.999).num, at(359.999).pada, at(359.999).name(), at(359.999).lord()), (27, 4, "Revati", "me"));
        assert_eq!((at(360.0).num, at(-0.001).num, at(246.7).name()), (1, 27, "Mula"));
    }
}
