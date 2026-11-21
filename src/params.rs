//! Query-string parameters shared by every endpoint.

use astro_calc_core::{AspectOrb, ChartKind, Ayanamsha, Body, CoordSystem, GauquelinSectors, GeoPos, HouseSystem, JdUt, Perspective, RiseSetOptions, aspect_orb, parse_body_list, parse_body_list_for};
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use chrono::Utc;
use julian_day_converter::unix_millis_to_julian_day;
use std::sync::LazyLock;

use is_truthy::TruthyRuleSet;
use serde::{Deserialize, Deserializer};

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
    /// The historic moment of the transposed endpoints: a date-time with the same offset as
    /// `dt`, or a Julian Day.
    pub h_dt: Option<String>,
    pub h_jd: Option<f64>,
    /// The historic place of the transposed endpoints, in the same forms as `loc`, `lat`,
    /// `lng` and `alt`.
    pub h_loc: Option<String>,
    pub h_lat: Option<f64>,
    pub h_lng: Option<f64>,
    pub h_alt: Option<f64>,
    /// Latitude, longitude (degrees) and altitude (metres); each overrides its part of `loc`.
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub alt: Option<f64>,
    /// Comma-separated body keys, e.g. `su,mo,ve,ma`.
    pub bodies: Option<String>,
    /// A single body key.
    pub body: Option<String>,
    /// Coordinate system: `ecliptic`/`ecl`/`0`, `equatorial`/`equ`/`1`, `horizontal`/`hor`/`2`,
    /// or `all` for the three together.
    pub cs: Option<String>,
    /// Perspective: `geo`, `topo`, `helio`, `seleno`, `hermeo`, `cythero`, `areo`, `jovi`,
    /// `crono`, `bary` or `local`, or the full names. The position endpoints also take `both`:
    /// geocentric and topocentric values side by side.
    pub ps: Option<String>,
    /// Ayanamsha keys, comma-separated, or `all`.
    pub aya: Option<String>,
    /// `1` or `true` to subtract the first ayanamsha from ecliptic longitudes.
    #[serde(default, deserialize_with = "flag")]
    pub sid: Option<bool>,
    /// Span in days, or in years.
    pub days: Option<f64>,
    pub years: Option<f64>,
    /// Days before the date, for day-by-day series (`/rise-sets`).
    pub pdays: Option<f64>,
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
    #[serde(default, deserialize_with = "flag")]
    pub fixed: Option<bool>,
    /// Rise/set: apparent altitude of the horizon in degrees.
    pub horizon: Option<f64>,
    /// Rise/set: `civil`, `nautical` or `astronomical` twilight instead of the horizon.
    pub twilight: Option<String>,
    /// Rise/set: `1` to lower the horizon by the dip for the observer's altitude.
    #[serde(default, deserialize_with = "flag")]
    pub dip: Option<bool>,
    /// Rise/set: `1` to use geocentric positions (no parallax).
    #[serde(default, deserialize_with = "flag")]
    pub geoc: Option<bool>,
    /// Rise/set: `1` to ignore the body's ecliptic latitude.
    #[serde(default, deserialize_with = "flag")]
    pub nolat: Option<bool>,
    /// Rise/set: `1` for the Hindu rising preset (centre, no refraction, geocentric, no latitude).
    #[serde(default, deserialize_with = "flag")]
    pub hindu: Option<bool>,
    /// How many items to return, and how many before the date.
    pub num: Option<u32>,
    pub before: Option<u8>,
    /// Aspect definitions as `angle:orb` pairs, e.g. `0:8,60:6,90:8`; or one `orb` for the
    /// default aspects.
    pub aspects: Option<String>,
    pub orb: Option<f64>,
    /// Bulk aspect searches: the bodies of the second chart (default: the same as `bodies`).
    pub bodies2: Option<String>,
    /// Bulk requests: keep only records with these tags, as `key:value` pairs, e.g. `sex:f`.
    pub tags: Option<String>,
    /// Bulk aspect searches: which charts are secondary-progressed to the date given by
    /// `dt` or `jd` (default now): `a`, `b` or `ab`.
    pub prog: Option<String>,
    /// `/tithis`: `1` to list the karanas (half tithis) instead of the tithis.
    #[serde(default, deserialize_with = "flag")]
    pub karanas: Option<bool>,
    /// Older name for `ps=topo`.
    #[serde(default, deserialize_with = "flag")]
    pub topo: Option<bool>,
    /// Older name for `cs`, with the same values; `cs` wins if both are given.
    pub eq: Option<String>,
}

/// An on/off parameter: `1`, `true` or `on` is on, `0`, `false` or `off` is off, in any
/// letter case.
/// Anything else, including an empty value, counts as absent, so the default applies.
fn flag<'de, D: Deserializer<'de>>(de: D) -> Result<Option<bool>, D::Error> {
    static RULES: LazyLock<TruthyRuleSet> = LazyLock::new(|| TruthyRuleSet::new().add_true("on").add_false("off").use_defaults());
    Ok(Option::<String>::deserialize(de)?.and_then(|text| RULES.parse(&text)))
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

/// One entry of `hsys`: a twelve-house system, or the 36 Gauquelin sectors (`G`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HouseChoice {
    System(HouseSystem),
    Gauquelin,
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
        place("loc", self.loc.as_deref(), self.lat, self.lng, self.alt)
    }

    /// The historic place of the transposed endpoints, if any part of one was given:
    /// `h_loc`, with `h_lat`, `h_lng` and `h_alt` overriding its parts.
    pub fn historic_geo(&self) -> Result<Option<GeoPos>, ApiError> {
        place("h_loc", self.h_loc.as_deref(), self.h_lat, self.h_lng, self.h_alt)
    }

    /// The historic moment of the transposed endpoints: `h_jd`, else `h_dt` less the offset.
    pub fn historic_jd(&self) -> Result<JdUt, ApiError> {
        match (self.h_jd, &self.h_dt) {
            (Some(jd), _) if jd.is_finite() => Ok(JdUt(jd)),
            (Some(_), _) => Err(ApiError::bad("invalid h_jd")),
            (None, Some(dt)) => self.local_to_jd(dt),
            (None, None) => Err(ApiError::bad("the historic moment is required: h_dt or h_jd")),
        }
    }

    /// How the historic positions of the transposed endpoints are seen: from the historic
    /// place (`ps=topo`, the default when one is given) or from the centre of the Earth
    /// (`ps=geo`, the default without one). Returns the place only in the first case.
    pub fn historic_view(&self) -> Result<(Perspective, Option<GeoPos>), ApiError> {
        let place = self.historic_geo()?;
        let perspective = match (&self.ps, self.topo) {
            (Some(_), _) | (None, Some(_)) => self.perspective()?,
            (None, None) if place.is_some() => Perspective::Topocentric,
            (None, None) => Perspective::Geocentric,
        };
        match perspective {
            Perspective::Geocentric => Ok((perspective, None)),
            Perspective::Topocentric | Perspective::LocalSpaceHorizon => match place {
                Some(place) => Ok((Perspective::Topocentric, Some(place))),
                None => Err(ApiError::bad("ps=topo needs the historic place: h_loc=lat,lng or h_lat and h_lng")),
            },
            _ => Err(ApiError::bad("transposed points need an observer on the Earth: ps=geo or ps=topo")),
        }
    }

    /// The location, for endpoints that cannot work without one.
    pub fn require_geo(&self) -> Result<GeoPos, ApiError> {
        self.geo()?.ok_or_else(|| ApiError::bad("a location is required: loc=lat,lng or lat and lng"))
    }

    /// Whether all three coordinate systems were asked for together: `cs=all` (or `ext`).
    /// The coordinate system key in lower case: `cs`, or its older name `eq`.
    fn cs_key(&self) -> Option<String> {
        self.cs.as_deref().or(self.eq.as_deref()).map(|key| key.trim().to_ascii_lowercase())
    }

    pub fn all_systems(&self) -> bool {
        self.cs_key().is_some_and(|cs| matches!(cs.as_str(), "all" | "ext"))
    }

    /// The coordinate system. With `cs=all` this is horizontal, the most demanding of the
    /// three: on the Earth it needs a place.
    pub fn coord_system(&self) -> Result<CoordSystem, ApiError> {
        if self.all_systems() {
            return Ok(CoordSystem::Horizontal);
        }
        Ok(match self.cs_key().as_deref() {
            None | Some("ecliptic" | "ecl" | "0") => CoordSystem::Ecliptic,
            Some("equatorial" | "equ" | "eq" | "1") => CoordSystem::Equatorial,
            Some("horizontal" | "hor" | "2") => CoordSystem::Horizontal,
            Some(other) => return Err(ApiError::bad(format!("unknown cs: {other} (use ecl, equ, hor or all)"))),
        })
    }

    /// Whether geocentric and topocentric values were asked for together: `ps=both`.
    pub fn both_views(&self) -> bool {
        self.ps.as_deref().is_some_and(|ps| ps.trim().eq_ignore_ascii_case("both"))
    }

    pub fn perspective(&self) -> Result<Perspective, ApiError> {
        match self.ps.as_deref() {
            Some(key) if self.both_views() => Err(ApiError::bad(format!("ps={key} is only available for the position endpoints"))),
            Some(key) => key.parse().map_err(|_| {
                let keys: Vec<&str> = Perspective::ALL.iter().map(|p| p.key()).collect();
                ApiError::bad(format!("unknown ps: {key} (use one of {})", keys.join(", ")))
            }),
            None if self.topo == Some(true) => Ok(Perspective::Topocentric),
            None => Ok(Perspective::Geocentric),
        }
    }

    /// The coordinate system, perspective and location together. A location is required
    /// for observers on the Earth's surface and for horizontal coordinates, optional otherwise.
    pub fn view(&self) -> Result<(CoordSystem, Perspective, GeoPos), ApiError> {
        let (cs, ps) = (self.coord_system()?, self.perspective()?);
        Ok((cs, ps, self.view_geo(cs, ps)?))
    }

    /// The location for a view: required for observers on the Earth's surface and for
    /// horizontal coordinates, optional otherwise. `cs=all` away from the Earth has no
    /// horizon to compute, so it needs no place.
    fn view_geo(&self, cs: CoordSystem, ps: Perspective) -> Result<GeoPos, ApiError> {
        let horizon = cs == CoordSystem::Horizontal && !(self.all_systems() && !ps.is_earth_based());
        if horizon || ps.needs_location() { self.require_geo() } else { Ok(self.geo()?.unwrap_or_default()) }
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
        Ok((cs, ps, self.view_geo(cs, ps)?))
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

    /// House divisions named in `hsys`; `all` gives every one, the Gauquelin sectors last.
    /// Defaults to whole sign.
    pub fn house_choices(&self) -> Result<Vec<HouseChoice>, ApiError> {
        let Some(list) = self.hsys.as_deref() else { return Ok(vec![HouseChoice::System(HouseSystem::WholeSign)]) };
        if list.trim().eq_ignore_ascii_case("all") {
            return Ok(HouseSystem::ALL.into_iter().map(HouseChoice::System).chain([HouseChoice::Gauquelin]).collect());
        }
        items(list)
            .map(|k| {
                let mut chars = k.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.eq_ignore_ascii_case(&GauquelinSectors::CODE) => Ok(HouseChoice::Gauquelin),
                    (Some(c), None) => HouseSystem::from_code(c).map(HouseChoice::System).ok_or_else(|| ApiError::bad(format!("unknown house system: {k}"))),
                    _ => Err(ApiError::bad(format!("unknown house system: {k}"))),
                }
            })
            .collect()
    }

    /// Rise/set options: a preset (`mode` or `hindu`), then each explicit parameter on top.
    pub fn rise_set_options(&self) -> Result<RiseSetOptions, ApiError> {
        use astro_calc_core::{DiscAlignment, Horizon, Refraction};
        let mut opts = match (self.hindu, self.mode) {
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
        if let Some(fixed) = self.fixed {
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
        if let Some(dip) = self.dip {
            opts.dip = dip;
        }
        if let Some(geocentric) = self.geoc {
            opts.geocentric = geocentric;
        }
        if let Some(ignore) = self.nolat {
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

    /// `bodies2`, if given.
    pub fn bodies2(&self) -> Result<Option<Vec<Body>>, ApiError> {
        match self.bodies2.as_deref() {
            Some(list) if !list.trim().is_empty() => parse_body_list(list).map(Some).map_err(|e| ApiError::bad(e.to_string())),
            _ => Ok(None),
        }
    }

    /// The `key:value` pairs of `tags`.
    pub fn tag_filter(&self) -> Result<Vec<(String, String)>, ApiError> {
        let Some(list) = self.tags.as_deref() else { return Ok(Vec::new()) };
        items(list)
            .map(|pair| match pair.split_once(':') {
                Some((key, value)) if !key.trim().is_empty() => Ok((key.trim().to_string(), value.trim().to_string())),
                _ => Err(ApiError::bad(format!("invalid tag filter: {pair} (use key:value)"))),
            })
            .collect()
    }

    /// The chart kinds of A and B named by `prog`, if it was given.
    pub fn progressed(&self) -> Result<Option<(ChartKind, ChartKind)>, ApiError> {
        use ChartKind::{Natal, Progressed};
        Ok(match self.prog.as_deref().map(str::to_ascii_lowercase).as_deref() {
            None | Some("" | "0" | "none") => None,
            Some("a") => Some((Progressed, Natal)),
            Some("b") => Some((Natal, Progressed)),
            Some("ab" | "both" | "1") => Some((Progressed, Progressed)),
            Some(other) => return Err(ApiError::bad(format!("unknown prog: {other} (use a, b or ab)"))),
        })
    }
}


/// A place from its `lat,lng[,alt]` text and the separate numbers that override its parts.
/// `name` is the text parameter's name, for error messages.
fn place(name: &str, text: Option<&str>, lat: Option<f64>, lng: Option<f64>, alt: Option<f64>) -> Result<Option<GeoPos>, ApiError> {
    let mut base = match text {
        Some(loc) => Some(loc.parse::<GeoPos>().map_err(|e| ApiError::bad(format!("invalid {name}: {e}")))?),
        None => None,
    };
    if lat.is_some() || lng.is_some() || alt.is_some() {
        let b = base.unwrap_or_default();
        let geo = GeoPos::new(lat.unwrap_or(b.lat), lng.unwrap_or(b.lng), alt.unwrap_or(b.alt));
        base = Some(geo.map_err(|e| ApiError::bad(format!("invalid location: {e}")))?);
    }
    Ok(base)
}
