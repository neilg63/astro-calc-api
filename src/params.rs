//! Query-string parameters shared by every endpoint.

use astro_calc_core::{AspectOrb, Ayanamsha, Body, CoordSystem, GeoPos, HouseSystem, JdUt, Perspective, RiseSetOptions, aspect_orb, parse_body_list};
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
    /// Coordinate system: `ecliptic`/`ecl`, `equatorial`/`equ` or `horizontal`/`hor`.
    pub cs: Option<String>,
    /// Perspective: `geocentric`/`geo` or `topocentric`/`topo`.
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
    /// Rise/set mode 0–7 (disc alignment and refraction).
    pub mode: Option<u8>,
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

    pub fn coord_system(&self) -> Result<CoordSystem, ApiError> {
        let key = self.cs.as_deref().map(str::to_ascii_lowercase);
        Ok(match key.as_deref() {
            Some("ecliptic" | "ecl" | "0") => CoordSystem::Ecliptic,
            Some("equatorial" | "equ" | "eq" | "1") => CoordSystem::Equatorial,
            Some("horizontal" | "hor" | "2") => CoordSystem::Horizontal,
            Some(other) => return Err(ApiError::bad(format!("unknown cs: {other} (use ecl, equ or hor)"))),
            None => match self.eq {
                Some(1) => CoordSystem::Equatorial,
                Some(3) => CoordSystem::Horizontal,
                _ => CoordSystem::Ecliptic,
            },
        })
    }

    pub fn perspective(&self) -> Result<Perspective, ApiError> {
        let key = self.ps.as_deref().map(str::to_ascii_lowercase);
        Ok(match key.as_deref() {
            Some("geocentric" | "geo" | "0") => Perspective::Geocentric,
            Some("topocentric" | "topo" | "1") => Perspective::Topocentric,
            Some(other) => return Err(ApiError::bad(format!("unknown ps: {other} (use geo or topo)"))),
            None if self.topo == Some(1) => Perspective::Topocentric,
            None => Perspective::Geocentric,
        })
    }

    /// The coordinate system, perspective and location together. A location is required
    /// for topocentric positions and horizontal coordinates and optional otherwise.
    pub fn view(&self) -> Result<(CoordSystem, Perspective, GeoPos), ApiError> {
        let (cs, ps) = (self.coord_system()?, self.perspective()?);
        let geo = if cs == CoordSystem::Horizontal || ps == Perspective::Topocentric {
            self.require_geo()?
        } else {
            self.geo()?.unwrap_or_default()
        };
        Ok((cs, ps, geo))
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

    pub fn rise_set_options(&self) -> Result<RiseSetOptions, ApiError> {
        match self.mode {
            None => Ok(RiseSetOptions::default()),
            Some(mode) if mode <= 7 => Ok(RiseSetOptions::from_legacy_mode(mode)),
            Some(mode) => Err(ApiError::bad(format!("mode must be 0 to 7, not {mode}"))),
        }
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
