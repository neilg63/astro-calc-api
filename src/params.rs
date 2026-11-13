//! Query-string parameters shared by every endpoint.

use astro_calc_core::{AspectOrb, Ayanamsha, Body, CoordSystem, GeoPos, HouseSystem, JdUt, Perspective, RiseSetOptions, aspect_orb, parse_body_list, parse_body_list_for};
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use chrono::Utc;
use julian_day_converter::unix_millis_to_julian_day;
use serde::Deserialize;

use crate::error::ApiError;

/// Days in a tropical year.
pub const YEAR_DAYS: f64 = 365.242_19;

/// Every recognised parameter. All are optional; each endpoint reads the ones it needs.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Params {
    /// Date-time, `YYYY-MM-DDThh:mm:ss` or any leading part of it. UTC unless an offset is given.
    pub dt: Option<String>,
    /// Julian Day (UT). Takes precedence over `dt` and ignores the offsets.
    pub jd: Option<f64>,
    /// Timezone offset of `dt` from UTC in seconds (east positive). `tzs` is an alias.
    pub tzos: Option<f64>,
    pub tzs: Option<f64>,
    /// The same offset in minutes.
    pub tzom: Option<f64>,
    /// The same offset in hours; fractions allowed.
    pub tzoh: Option<f64>,
    /// End of a range: date-time (`dt2`, same offset as `dt`) or Julian Day (`jd2`).
    pub dt2: Option<String>,
    pub jd2: Option<f64>,
    /// Location as `lat,lng` or `lat,lng,alt`.
    pub loc: Option<String>,
    /// Latitude, longitude (degrees) and altitude (metres); each overrides its part of `loc`.
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub alt: Option<f64>,
    /// Comma-separated body keys, e.g. `su,mo,ve,ma`.
    pub bodies: Option<String>,
    /// A single body key.
    pub body: Option<String>,
    /// Coordinate system: `ecliptic`/`ecl`, `equatorial`/`equ`, `horizontal`/`hor`, or `all`
    /// for the three together.
    pub cs: Option<String>,
    /// Perspective: `geo`, `topo`, `helio`, `seleno`, `hermeo`, `cythero`, `areo`, `jovi`,
    /// `crono`, `bary` or `local`, or the full names.
    pub ps: Option<String>,
    /// Ayanamsha keys, comma-separated, or `all`.
    pub aya: Option<String>,
    /// `1` to subtract the first ayanamsha from ecliptic longitudes.
    pub sid: Option<u8>,
    /// Span in days, or in years.
    pub days: Option<f64>,
    pub years: Option<f64>,
    /// Samples per day (or per year with `years`) for progress series.
    pub pd: Option<f64>,
    /// House system letters, comma-separated, or `all`.
    pub hsys: Option<String>,
    /// Rise/set mode 0–7: the older numeric presets for disc alignment and refraction.
    pub mode: Option<u8>,
    /// Rise/set: which part of the disc, `upper`, `center` or `lower`.
    pub disc: Option<String>,
    /// Rise/set: `0` or `off` for no refraction, `1` or `on` for standard refraction.
    pub refraction: Option<String>,
    /// Rise/set: air pressure in hPa and temperature in °C, to scale the refraction.
    pub pressure: Option<f64>,
    pub temp: Option<f64>,
    /// Rise/set: `1` for a fixed mean radius of the Sun and Moon.
    pub fixed: Option<u8>,
    /// Rise/set: apparent altitude of the horizon in degrees.
    pub horizon: Option<f64>,
    /// Rise/set: `civil`, `nautical` or `astronomical` twilight instead of the horizon.
    pub twilight: Option<String>,
    /// Rise/set: `1` to lower the horizon by the dip for the observer's altitude.
    pub dip: Option<u8>,
    /// Rise/set: `1` to use geocentric positions (no parallax).
    pub geoc: Option<u8>,
    /// Rise/set: `1` to ignore the body's ecliptic latitude.
    pub nolat: Option<u8>,
    /// Rise/set: `1` for the Hindu rising preset (centre, no refraction, geocentric, no latitude).
    pub hindu: Option<u8>,
    /// How many items to return, and how many before the date.
    pub num: Option<u32>,
    pub before: Option<u8>,
    /// Aspect definitions as `angle:orb` pairs, e.g. `0:8,60:6,90:8`; or one `orb` for the
    /// default aspects.
    pub aspects: Option<String>,
    pub orb: Option<f64>,
    /// Legacy aliases: `topo=1` for `ps=topo`; `eq=1` for `cs=equ`, `eq=3` for `cs=hor`.
    pub topo: Option<u8>,
    pub eq: Option<u8>,
}

/// Extracts [`Params`], turning a malformed query string into a JSON error.
pub struct Q(pub Params);

impl<S: Send + Sync> FromRequestParts<S> for Q {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Query::<Params>::from_request_parts(parts, state).await {
            Ok(Query(p)) => Ok(Q(p)),
            Err(e) => Err(ApiError::bad(e.body_text())),
        }
    }
}

fn items(list: &str) -> impl Iterator<Item = &str> {
    list.split(',').map(str::trim).filter(|s| !s.is_empty())
}

impl Params {
    /// Offset of local date-times from UTC, in days.
    fn offset_days(&self) -> f64 {
        let secs = self.tzos.or(self.tzs).or(self.tzom.map(|m| m * 60.0)).or(self.tzoh.map(|h| h * 3600.0)).unwrap_or(0.0);
        secs / 86_400.0
    }

    fn local_to_jd(&self, text: &str) -> Result<JdUt, ApiError> {
        let jd = JdUt::from_iso(text).map_err(|_| ApiError::bad(format!("invalid date-time: {text}")))?;
        Ok(jd.add_days(-self.offset_days()))
    }

    /// The moment asked about: `jd`, else `dt` less its offset, else now.
    pub fn jd(&self) -> Result<JdUt, ApiError> {
        match (self.jd, &self.dt) {
            (Some(jd), _) if jd.is_finite() => Ok(JdUt(jd)),
            (Some(_), _) => Err(ApiError::bad("invalid jd")),
            (None, Some(dt)) => self.local_to_jd(dt),
            (None, None) => Ok(JdUt(unix_millis_to_julian_day(Utc::now().timestamp_millis()))),
        }
    }

    /// The end of a range, if one was given: `jd2`, `dt2`, or the start plus `days`/`years`.
    pub fn jd_end(&self, start: JdUt) -> Result<Option<JdUt>, ApiError> {
        if let Some(jd) = self.jd2 {
            return Ok(Some(JdUt(jd)));
        }
        if let Some(dt) = &self.dt2 {
            return self.local_to_jd(dt).map(Some);
        }
        Ok(self.span_days()?.map(|d| start.add_days(d)))
    }

    /// `days`, or `years` as days.
    pub fn span_days(&self) -> Result<Option<f64>, ApiError> {
        let span = self.days.or(self.years.map(|y| y * YEAR_DAYS));
        match span {
            Some(d) if !(d.is_finite() && d >= 0.0) => Err(ApiError::bad("days and years must not be negative")),
            other => Ok(other),
        }
    }

    /// The location, if any part of one was given.
    pub fn geo(&self) -> Result<Option<GeoPos>, ApiError> {
        let mut base = match &self.loc {
            Some(loc) => Some(loc.parse::<GeoPos>().map_err(|e| ApiError::bad(format!("invalid loc: {e}")))?),
            None => None,
        };
        if self.lat.is_some() || self.lng.is_some() || self.alt.is_some() {
            let b = base.unwrap_or_default();
            let geo = GeoPos::new(self.lat.unwrap_or(b.lat), self.lng.unwrap_or(b.lng), self.alt.unwrap_or(b.alt));
            base = Some(geo.map_err(|e| ApiError::bad(format!("invalid location: {e}")))?);
        }
        Ok(base)
    }

    /// The location, for endpoints that cannot work without one.
    pub fn require_geo(&self) -> Result<GeoPos, ApiError> {
        self.geo()?.ok_or_else(|| ApiError::bad("a location is required: loc=lat,lng or lat and lng"))
    }

    /// Whether all three coordinate systems were asked for together: `cs=all` (or `ext`).
    pub fn all_systems(&self) -> bool {
        self.cs.as_deref().is_some_and(|cs| matches!(cs.to_ascii_lowercase().as_str(), "all" | "ext"))
    }

    /// The coordinate system. With `cs=all` this is horizontal, the most demanding of the
    /// three: it needs a place and an observer on the Earth.
    pub fn coord_system(&self) -> Result<CoordSystem, ApiError> {
        if self.all_systems() {
            return Ok(CoordSystem::Horizontal);
        }
        let key = self.cs.as_deref().map(str::to_ascii_lowercase);
        Ok(match key.as_deref() {
            Some("ecliptic" | "ecl" | "0") => CoordSystem::Ecliptic,
            Some("equatorial" | "equ" | "eq" | "1") => CoordSystem::Equatorial,
            Some("horizontal" | "hor" | "2") => CoordSystem::Horizontal,
            Some(other) => return Err(ApiError::bad(format!("unknown cs: {other} (use ecl, equ, hor or all)"))),
            None => match self.eq {
                Some(1) => CoordSystem::Equatorial,
                Some(3) => CoordSystem::Horizontal,
                _ => CoordSystem::Ecliptic,
            },
        })
    }

    pub fn perspective(&self) -> Result<Perspective, ApiError> {
        match self.ps.as_deref() {
            Some(key) => key.parse().map_err(|_| {
                let keys: Vec<&str> = Perspective::ALL.iter().map(|p| p.key()).collect();
                ApiError::bad(format!("unknown ps: {key} (use one of {})", keys.join(", ")))
            }),
            None if self.topo == Some(1) => Ok(Perspective::Topocentric),
            None => Ok(Perspective::Geocentric),
        }
    }

    /// The coordinate system, perspective and location together. A location is required
    /// for observers on the Earth's surface and for horizontal coordinates, optional otherwise.
    pub fn view(&self) -> Result<(CoordSystem, Perspective, GeoPos), ApiError> {
        let (cs, ps) = (self.coord_system()?, self.perspective()?);
        let geo = if cs == CoordSystem::Horizontal || ps.needs_location() {
            self.require_geo()?
        } else {
            self.geo()?.unwrap_or_default()
        };
        Ok((cs, ps, geo))
    }

    /// The perspective for the position and longitude endpoints, where the default is
    /// topocentric whenever a place is given (and geocentric when none is).
    pub fn perspective_or_topo(&self) -> Result<Perspective, ApiError> {
        match (&self.ps, self.topo) {
            (Some(_), _) | (None, Some(_)) => self.perspective(),
            (None, None) if self.geo()?.is_some() => Ok(Perspective::Topocentric),
            (None, None) => Ok(Perspective::Geocentric),
        }
    }

    /// As [`view`](Self::view), with the topocentric default of the position endpoints.
    pub fn view_topo(&self) -> Result<(CoordSystem, Perspective, GeoPos), ApiError> {
        let (cs, ps) = (self.coord_system()?, self.perspective_or_topo()?);
        let geo = if cs == CoordSystem::Horizontal || ps.needs_location() {
            self.require_geo()?
        } else {
            self.geo()?.unwrap_or_default()
        };
        Ok((cs, ps, geo))
    }

    /// The bodies to compute for a perspective: `bodies` (or `body`), which may include the
    /// set names `default` and `basic`, else the given default list. Sets are adapted to the
    /// perspective: away from the Earth they gain the Earth and lose what cannot be seen.
    /// The ascendant needs a place: without one it is left out of a set, and asking for it
    /// by name is an error.
    pub fn body_list(&self, ps: Perspective, default: &[Body]) -> Result<Vec<Body>, ApiError> {
        let has_place = self.geo()?.is_some();
        let mut list = match self.bodies.as_deref().or(self.body.as_deref()) {
            Some(text) if !text.trim().is_empty() => {
                let named = items(text).any(|t| matches!(t.to_ascii_lowercase().as_str(), "as" | "asc" | "ascendant"));
                if named && !has_place {
                    return Err(ApiError::bad("the ascendant needs a location: loc=lat,lng or lat and lng"));
                }
                parse_body_list_for(text, ps).map_err(|e| ApiError::bad(e.to_string()))?
            }
            _ => Body::set_for(default, ps),
        };
        if !has_place {
            list.retain(|b| *b != Body::Ascendant);
        }
        Ok(list)
    }

    /// `bodies` (or `body`), else the given default list.
    pub fn bodies(&self, default: &[Body]) -> Result<Vec<Body>, ApiError> {
        match self.bodies.as_deref().or(self.body.as_deref()) {
            Some(list) if !list.trim().is_empty() => parse_body_list(list).map_err(|e| ApiError::bad(e.to_string())),
            _ => Ok(default.to_vec()),
        }
    }

    /// `body` (or the first of `bodies`).
    pub fn body(&self) -> Result<Body, ApiError> {
        let key = self.body.as_deref().or(self.bodies.as_deref()).ok_or_else(|| ApiError::bad("a body is required: body=key"))?;
        let first = items(key).next().ok_or_else(|| ApiError::bad("a body is required: body=key"))?;
        first.parse().map_err(|_| ApiError::bad(format!("unknown body: {first}")))
    }

    /// Ayanamshas named in `aya`; `all` gives every one. Empty if the parameter is absent.
    pub fn ayanamshas(&self) -> Result<Vec<Ayanamsha>, ApiError> {
        let Some(list) = self.aya.as_deref() else { return Ok(Vec::new()) };
        if list.trim().eq_ignore_ascii_case("all") {
            return Ok(Ayanamsha::ALL.into_iter().filter(|a| a.key().is_some()).collect());
        }
        items(list).map(|k| k.parse().map_err(|_| ApiError::bad(format!("unknown ayanamsha: {k}")))).collect()
    }

    /// House systems named in `hsys`; `all` gives every one. Defaults to whole sign.
    pub fn house_systems(&self) -> Result<Vec<HouseSystem>, ApiError> {
        let Some(list) = self.hsys.as_deref() else { return Ok(vec![HouseSystem::WholeSign]) };
        if list.trim().eq_ignore_ascii_case("all") {
            return Ok(HouseSystem::ALL.to_vec());
        }
        items(list)
            .map(|k| {
                let mut chars = k.chars();
                match (chars.next().and_then(HouseSystem::from_code), chars.next()) {
                    (Some(system), None) => Ok(system),
                    _ => Err(ApiError::bad(format!("unknown house system: {k}"))),
                }
            })
            .collect()
    }

    /// Rise/set options: a preset (`mode` or `hindu`), then each explicit parameter on top.
    pub fn rise_set_options(&self) -> Result<RiseSetOptions, ApiError> {
        use astro_calc_core::{DiscAlignment, Horizon, Refraction};
        let flag = |name: &str, v: Option<u8>| match v {
            None => Ok(None),
            Some(0) => Ok(Some(false)),
            Some(1) => Ok(Some(true)),
            Some(other) => Err(ApiError::bad(format!("{name} must be 0 or 1, not {other}"))),
        };
        let mut opts = match (flag("hindu", self.hindu)?, self.mode) {
            (Some(true), _) => RiseSetOptions::hindu_rising(),
            (_, None) => RiseSetOptions::default(),
            (_, Some(mode)) if mode <= 7 => RiseSetOptions::from_legacy_mode(mode),
            (_, Some(mode)) => return Err(ApiError::bad(format!("mode must be 0 to 7, not {mode}"))),
        };
        if let Some(disc) = self.disc.as_deref() {
            opts.disc = match disc.to_ascii_lowercase().as_str() {
                "upper" | "top" | "u" => DiscAlignment::UpperLimb,
                "center" | "centre" | "c" => DiscAlignment::Center,
                "lower" | "bottom" | "l" => DiscAlignment::LowerLimb,
                other => return Err(ApiError::bad(format!("unknown disc: {other} (use upper, center or lower)"))),
            };
        }
        if let Some(refraction) = self.refraction.as_deref() {
            opts.refraction = match refraction.to_ascii_lowercase().as_str() {
                "0" | "off" | "no" | "false" => Refraction::Off,
                "1" | "on" | "yes" | "true" | "standard" => Refraction::Standard,
                other => return Err(ApiError::bad(format!("unknown refraction: {other} (use 0 or 1)"))),
            };
        }
        if self.pressure.is_some() || self.temp.is_some() {
            let (pressure, temperature) = (self.pressure.unwrap_or(1010.0), self.temp.unwrap_or(10.0));
            if !(pressure.is_finite() && pressure >= 0.0 && temperature.is_finite() && temperature > -273.0) {
                return Err(ApiError::bad("invalid pressure or temp"));
            }
            if opts.refraction != Refraction::Off {
                opts.refraction = Refraction::Atmosphere { pressure, temperature };
            }
        }
        if let Some(fixed) = flag("fixed", self.fixed)? {
            opts.fixed_disc = fixed;
        }
        if let Some(altitude) = self.horizon {
            if !(altitude.is_finite() && altitude.abs() < 90.0) {
                return Err(ApiError::bad("horizon must be an altitude between -90 and 90 degrees"));
            }
            opts.horizon = Horizon::Altitude(altitude);
        }
        if let Some(twilight) = self.twilight.as_deref() {
            opts.horizon = match twilight.to_ascii_lowercase().as_str() {
                "civil" | "c" => Horizon::CivilTwilight,
                "nautical" | "nautic" | "n" => Horizon::NauticalTwilight,
                "astronomical" | "astro" | "a" => Horizon::AstronomicalTwilight,
                other => return Err(ApiError::bad(format!("unknown twilight: {other} (use civil, nautical or astronomical)"))),
            };
        }
        if let Some(dip) = flag("dip", self.dip)? {
            opts.dip = dip;
        }
        if let Some(geocentric) = flag("geoc", self.geoc)? {
            opts.geocentric = geocentric;
        }
        if let Some(ignore) = flag("nolat", self.nolat)? {
            opts.ignore_latitude = ignore;
        }
        Ok(opts)
    }

    /// Aspect angles and orbs from `aspects`, or the major aspects with `orb` (default 8°,
    /// 6° for the sextile).
    pub fn aspect_orbs(&self) -> Result<Vec<AspectOrb>, ApiError> {
        if let Some(orb) = self.orb
            && !(orb.is_finite() && orb > 0.0)
        {
            return Err(ApiError::bad("orb must be positive"));
        }
        let Some(list) = self.aspects.as_deref() else {
            let orb = |default: f64| self.orb.unwrap_or(default);
            return Ok([(0.0, 8.0), (60.0, 6.0), (90.0, 8.0), (120.0, 8.0), (180.0, 8.0)].map(|(a, o)| aspect_orb(a, orb(o))).to_vec());
        };
        items(list)
            .map(|pair| {
                let bad = || ApiError::bad(format!("invalid aspect: {pair} (use angle:orb)"));
                let (angle, orb) = match pair.split_once(':') {
                    Some((a, o)) => (a.trim().parse::<f64>().map_err(|_| bad())?, o.trim().parse::<f64>().map_err(|_| bad())?),
                    None => (pair.parse::<f64>().map_err(|_| bad())?, self.orb.unwrap_or(8.0)),
                };
                if !(angle.is_finite() && orb.is_finite() && orb > 0.0) {
                    return Err(bad());
                }
                Ok(aspect_orb(angle, orb))
            })
            .collect()
    }
}
