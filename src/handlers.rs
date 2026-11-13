//! Endpoint handlers. Each resolves its parameters, calls the core library and shapes JSON.

use std::sync::Arc;

use astro_calc_core::{
    Ayanamsha, Body, BodyPos, BodyPosExt, CoordSystem, GeoPos, HouseSystem, JdUt, Perspective, Station, named_stars, to_sidereal,
};
use axum::Json;
use axum::extract::State;
use chrono::SecondsFormat;
use julian_day_converter::{julian_day_to_datetime, julian_day_to_unixtime};
use serde_json::{Value, json};

use crate::AppState;
use crate::error::ApiError;
use crate::params::{Params, Q, YEAR_DAYS};

type Reply = Result<Json<Value>, ApiError>;
type App = State<Arc<AppState>>;

/// Most samples (bodies × times) one progress request may ask for.
const MAX_SAMPLES: f64 = 200_000.0;

/// A moment as Julian Day, ISO UTC string and Unix seconds.
fn moment(jd: f64) -> Value {
    let utc = julian_day_to_datetime(jd).ok().map(|d| d.and_utc().to_rfc3339_opts(SecondsFormat::Secs, true));
    json!({ "jd": jd, "utc": utc, "unix": julian_day_to_unixtime(jd) })
}

fn opt_moment(jd: Option<JdUt>) -> Value {
    jd.map_or(Value::Null, |t| moment(t.value()))
}

fn geo_json(geo: GeoPos) -> Value {
    json!({ "lat": geo.lat, "lng": geo.lng, "alt": geo.alt })
}

/// The coordinate system of the result. Local Space is always on the horizon.
fn cs_key(cs: CoordSystem, ps: Perspective) -> &'static str {
    if ps == Perspective::LocalSpaceHorizon {
        return "horizontal";
    }
    match cs {
        CoordSystem::Ecliptic => "ecliptic",
        CoordSystem::Equatorial => "equatorial",
        CoordSystem::Horizontal => "horizontal",
    }
}

fn ps_key(ps: Perspective) -> &'static str {
    ps.name()
}

/// A position with its two angles named after the coordinate system: `lng`/`lat`,
/// `ras`/`dcl` or `azi`/`alt`. Local Space is always on the horizon.
fn body_json(p: &BodyPos, cs: CoordSystem, ps: Perspective, sidereal: Option<f64>) -> Value {
    let cs = if ps == Perspective::LocalSpaceHorizon { CoordSystem::Horizontal } else { cs };
    let (first, second) = match cs {
        CoordSystem::Ecliptic => ("lng", "lat"),
        CoordSystem::Equatorial => ("ras", "dcl"),
        CoordSystem::Horizontal => ("azi", "alt"),
    };
    let mut v = json!({ "key": p.body.to_string(), "dist": p.dist, "dist_speed": p.dist_speed });
    v[first] = json!(sidereal.map_or(p.lng, |aya| to_sidereal(p.lng, aya)));
    v[second] = json!(p.lat);
    v[format!("{first}_speed")] = json!(p.lng_speed);
    v[format!("{second}_speed")] = json!(p.lat_speed);
    v
}

/// A position in all three coordinate systems.
fn ext_json(p: &BodyPosExt, sidereal: Option<f64>) -> Value {
    json!({
        "key": p.body.to_string(),
        "lng": sidereal.map_or(p.lng, |aya| to_sidereal(p.lng, aya)), "lat": p.lat,
        "ras": p.ras, "dcl": p.dcl, "azi": p.azi, "alt": p.alt,
        "lng_speed": p.lng_speed, "lat_speed": p.lat_speed, "ras_speed": p.ras_speed, "dcl_speed": p.dcl_speed,
        "azi_speed": p.azi_speed, "alt_speed": p.alt_speed,
        "dist": p.dist, "dist_speed": p.dist_speed,
    })
}

/// The label for the coordinate system in a response.
fn cs_label(q: &Params, cs: CoordSystem, ps: Perspective) -> &'static str {
    if q.all_systems() { "all" } else { cs_key(cs, ps) }
}

fn station_json(s: &Station) -> Value {
    let mut v = moment(s.jd());
    v["type"] = json!(s.kind().key());
    v["speed"] = json!(s.speed());
    v
}

/// The requested ayanamshas with their values, and the one to subtract from longitudes if
/// `sid=1` and the coordinates are ecliptic.
fn ayanamsha_values(app: &AppState, q: &Params, jd: JdUt, cs: CoordSystem) -> Result<(Value, Option<f64>), ApiError> {
    let ayas = q.ayanamshas()?;
    let values = app.eph.ayanamshas(jd, &ayas)?;
    let list: Vec<Value> = ayas.iter().zip(&values).map(|(a, v)| json!({ "key": a.key(), "name": a.name(), "value": v })).collect();
    let applied = if q.sid == Some(1) && (cs == CoordSystem::Ecliptic || q.all_systems()) {
        Some(*values.first().ok_or_else(|| ApiError::bad("sid=1 needs an ayanamsha: aya=key"))?)
    } else {
        None
    };
    Ok((Value::Array(list), applied))
}

pub async fn not_found() -> ApiError {
    ApiError::not_found("no such endpoint; see / for the list")
}

/// `/`: what the service offers.
pub async fn index(State(app): App) -> Json<Value> {
    let range = app.eph.coverage(399).map(|(a, b)| json!({ "from": moment(a), "to": moment(b) }));
    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "ephemeris_range": range,
        "station_table": app.stations.is_some(),
        "endpoints": [
            "/bodies", "/stars", "/longitude", "/longitudes", "/position", "/positions", "/extended-position", "/extended-positions", "/progress", "/ascendant", "/angles", "/houses",
            "/ayanamsha", "/rise-set", "/stations", "/moon-phases", "/indian-day", "/pheno", "/aspects"
        ],
    }))
}

/// `/bodies`: the body keys, and whether each can be computed with the loaded files.
pub async fn bodies(State(app): App) -> Json<Value> {
    let jd = app.eph.coverage(399).map_or(2_451_545.0, |(a, b)| 0.5 * (a + b));
    let list: Vec<Value> = Body::NAMED
        .iter()
        .map(|b| {
            let ok = app.eph.calc_position(*b, jd, GeoPos::default(), CoordSystem::Ecliptic, Perspective::Geocentric).is_ok();
            json!({ "key": b.key(), "name": b.name(), "available": ok })
        })
        .collect();
    Json(json!({ "bodies": list, "stars": "see /stars; address a star as hip:NUMBER" }))
}

/// `/stars`: the built-in named stars.
pub async fn stars() -> Json<Value> {
    let list: Vec<Value> = named_stars()
        .iter()
        .map(|s| json!({ "key": s.body().to_string(), "name": s.name, "hip": s.hip, "magnitude": if s.magnitude.is_nan() { Value::Null } else { json!(s.magnitude) } }))
        .collect();
    Json(json!({ "stars": list }))
}

/// `/position`: one body.
pub async fn position(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let (cs, ps, geo) = q.view_topo()?;
    let body = *q.body_list(ps, &[])?.first().ok_or_else(|| ApiError::bad("a body is required: body=key"))?;
    let (ayanamshas, sid) = ayanamsha_values(&app, &q, jd, cs)?;
    let item = if q.all_systems() {
        ext_json(&app.eph.calc_position_ext(body, jd, geo, ps)?, sid)
    } else {
        body_json(&app.eph.calc_position(body, jd, geo, cs, ps)?, cs, ps, sid)
    };
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "cs": cs_label(&q, cs, ps), "ps": ps_key(ps),
        "sidereal": sid.is_some(), "ayanamshas": ayanamshas, "body": item,
    })))
}

/// `/positions`: several bodies at one moment.
pub async fn positions(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let (cs, ps, geo) = q.view_topo()?;
    let bodies = q.body_list(ps, &Body::DEFAULT_BODIES)?;
    let (ayanamshas, sid) = ayanamsha_values(&app, &q, jd, cs)?;
    let list: Vec<Value> = if q.all_systems() {
        app.eph.calc_positions_ext(&bodies, jd, geo, ps)?.iter().map(|p| ext_json(p, sid)).collect()
    } else {
        app.eph.calc_positions(&bodies, jd, geo, cs, ps)?.iter().map(|p| body_json(p, cs, ps, sid)).collect()
    };
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "cs": cs_label(&q, cs, ps), "ps": ps_key(ps),
        "sidereal": sid.is_some(), "ayanamshas": ayanamshas,
        "bodies": list,
    })))
}

/// The ayanamsha to subtract in the longitude endpoints: the first one named in `aya`.
fn longitude_ayanamsha(app: &AppState, q: &Params, jd: JdUt) -> Result<(Option<Ayanamsha>, Value), ApiError> {
    match q.ayanamshas()?.first().copied() {
        Some(aya) => Ok((Some(aya), json!({ "key": aya.key(), "name": aya.name(), "value": app.eph.ayanamsha(jd, aya)? }))),
        None => Ok((None, Value::Null)),
    }
}

/// `/longitude`: the ecliptic longitude of one body, and nothing else. With `aya` the
/// longitude is sidereal. Topocentric when a place is given.
pub async fn longitude(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let ps = q.perspective_or_topo()?;
    let geo = if ps.needs_location() { q.require_geo()? } else { q.geo()?.unwrap_or_default() };
    let body = *q.body_list(ps, &[])?.first().ok_or_else(|| ApiError::bad("a body is required: body=key"))?;
    let (aya, ayanamsha) = longitude_ayanamsha(&app, &q, jd)?;
    let lng = app.eph.calc_longitude(body, jd, geo, ps, aya)?;
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "ps": ps_key(ps),
        "sidereal": aya.is_some(), "ayanamsha": ayanamsha, "key": body.to_string(), "lng": lng,
    })))
}

/// `/longitudes`: the ecliptic longitudes of several bodies, the ascendant (`as`) among
/// them when a place is given. Defaults to the `default` set.
pub async fn longitudes(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let ps = q.perspective_or_topo()?;
    let geo = if ps.needs_location() { q.require_geo()? } else { q.geo()?.unwrap_or_default() };
    let bodies = q.body_list(ps, &Body::DEFAULT_BODIES)?;
    let (aya, ayanamsha) = longitude_ayanamsha(&app, &q, jd)?;
    let list = app.eph.calc_longitudes(&bodies, jd, geo, ps, aya)?;
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "ps": ps_key(ps),
        "sidereal": aya.is_some(), "ayanamsha": ayanamsha,
        "longitudes": list.iter().map(|l| json!({ "key": l.body.to_string(), "lng": l.lng })).collect::<Vec<_>>(),
    })))
}

/// `/extended-position`: one body in all three coordinate systems at once (ecliptic,
/// equatorial and horizontal), whatever `cs` says. Needs a place.
pub async fn extended_position(app: App, Q(mut q): Q) -> Reply {
    q.cs = Some("all".into());
    position(app, Q(q)).await
}

/// `/extended-positions`: several bodies in all three coordinate systems at once.
pub async fn extended_positions(app: App, Q(mut q): Q) -> Reply {
    q.cs = Some("all".into());
    positions(app, Q(q)).await
}

/// `/progress`: several bodies over a series of times from the date.
///
/// With `days`, positions are sampled `pd` times a day over that many days. With `years`,
/// the series is a secondary progression: one day of motion for each year, sampled `pd`
/// times a year, each sample labelled with the age and the real date it stands for.
pub async fn progress(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let (cs, ps, geo) = q.view()?;
    let bodies = q.body_list(ps, &Body::DEFAULT_BODIES)?;
    if q.all_systems() {
        return Err(ApiError::bad("cs=all is not available for /progress; ask for one coordinate system"));
    }
    let per = q.pd.unwrap_or(1.0);
    if !(per.is_finite() && per > 0.0) {
        return Err(ApiError::bad("pd must be positive"));
    }
    let (span, progressed) = match (q.years, q.days) {
        (Some(years), _) => (years, true),
        (None, Some(days)) => (days, false),
        (None, None) => return Err(ApiError::bad("days or years is required")),
    };
    if !(span.is_finite() && span >= 0.0) {
        return Err(ApiError::bad("days and years must not be negative"));
    }
    if (span * per).ceil().max(1.0) * bodies.len() as f64 > MAX_SAMPLES {
        return Err(ApiError::bad(format!("too many samples; the limit is {MAX_SAMPLES} positions per request")));
    }
    let interval = 1.0 / per;
    let (ayanamshas, sid) = ayanamsha_values(&app, &q, jd, cs)?;
    // A long series is CPU work: keep it off the async threads.
    let worker = app.clone();
    let sets = tokio::task::spawn_blocking(move || worker.eph.calc_progress_positions(&bodies, jd, span, interval, geo, cs, ps))
        .await
        .map_err(|e| ApiError { status: axum::http::StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })??;
    let series: Vec<Value> = sets
        .iter()
        .map(|set| {
            let items: Vec<Value> = set
                .items
                .iter()
                .map(|item| {
                    let mut v = body_json(&item.pos, cs, ps, sid);
                    v.as_object_mut().expect("object").remove("key");
                    v["jd"] = json!(item.jd.value());
                    if progressed {
                        let age = item.jd.value() - jd.value();
                        v["age"] = json!(age);
                        v["date"] = moment(jd.value() + age * YEAR_DAYS);
                    }
                    v
                })
                .collect();
            json!({ "key": set.body.to_string(), "items": items })
        })
        .collect();
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "cs": cs_key(cs, ps), "ps": ps_key(ps),
        "progressed": progressed, "span": span, "unit": if progressed { "years" } else { "days" }, "per_unit": per,
        "sidereal": sid.is_some(), "ayanamshas": ayanamshas, "bodies": series,
    })))
}

/// `/ascendant`: the ascendant alone.
pub async fn ascendant(Q(q): Q) -> Reply {
    let (jd, geo) = (q.jd()?, q.require_geo()?);
    Ok(Json(json!({ "date": moment(jd.value()), "geo": geo_json(geo), "ascendant": astro_calc_core::ascendant(jd, geo) })))
}

fn angles_json(a: &astro_calc_core::Angles) -> Value {
    json!({
        "ascendant": a.ascendant, "descendant": a.descendant(), "mc": a.mc, "ic": a.ic(), "armc": a.armc,
        "vertex": a.vertex, "equasc": a.equasc, "coasc1": a.coasc1, "coasc2": a.coasc2, "polasc": a.polasc,
    })
}

/// `/angles`: ascendant, MC and the other chart angles.
pub async fn angles(Q(q): Q) -> Reply {
    let (jd, geo) = (q.jd()?, q.require_geo()?);
    Ok(Json(json!({ "date": moment(jd.value()), "geo": geo_json(geo), "angles": angles_json(&astro_calc_core::angles(jd, geo)) })))
}

/// `/houses`: angles and the cusps of each requested house system.
pub async fn houses(Q(q): Q) -> Reply {
    let (jd, geo, systems) = (q.jd()?, q.require_geo()?, q.house_systems()?);
    let (points, sets) = astro_calc_core::house_sets(jd, geo, &systems);
    let sets: Vec<Value> = sets
        .iter()
        .map(|h| json!({ "system": h.system.code().to_string(), "name": h.system.name(), "cusps": h.cusps, "fallback": h.fallback.map(|f: HouseSystem| f.code().to_string()) }))
        .collect();
    Ok(Json(json!({ "date": moment(jd.value()), "geo": geo_json(geo), "angles": angles_json(&points), "sets": sets })))
}

/// `/ayanamsha`: the requested ayanamshas (all by default).
pub async fn ayanamsha(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let mut ayas = q.ayanamshas()?;
    if ayas.is_empty() {
        ayas = Ayanamsha::ALL.into_iter().filter(|a| a.key().is_some()).collect();
    }
    let values = app.eph.ayanamshas(jd, &ayas)?;
    let list: Vec<Value> = ayas.iter().zip(values).map(|(a, v)| json!({ "key": a.key(), "name": a.name(), "value": v })).collect();
    Ok(Json(json!({ "date": moment(jd.value()), "ayanamshas": list })))
}

/// `/rise-set`: rise, set and meridian transits of each body for the local day.
pub async fn rise_set(State(app): App, Q(q): Q) -> Reply {
    let (jd, geo, opts) = (q.jd()?, q.require_geo()?, q.rise_set_options()?);
    let bodies = q.bodies(&[Body::Sun, Body::Moon])?;
    let mut list = Vec::with_capacity(bodies.len());
    for body in bodies {
        let d = app.eph.day_transits(body, jd, geo, opts)?;
        list.push(json!({
            "key": body.to_string(), "day_start": moment(d.day_start.value()),
            "prev_set": opt_moment(d.prev_set), "rise": opt_moment(d.rise), "mc": opt_moment(d.mc),
            "set": opt_moment(d.set), "ic": opt_moment(d.ic), "next_rise": opt_moment(d.next_rise),
            "min_alt": d.min_alt, "max_alt": d.max_alt,
        }));
    }
    Ok(Json(json!({ "date": moment(jd.value()), "geo": geo_json(geo), "options": rise_set_json(&opts), "bodies": list })))
}

/// The rise/set options in force, so a client can see what its parameters resolved to.
fn rise_set_json(o: &astro_calc_core::RiseSetOptions) -> Value {
    use astro_calc_core::{DiscAlignment, Horizon, Refraction};
    let (horizon, twilight) = match o.horizon {
        Horizon::Level => (json!(0.0), Value::Null),
        Horizon::Altitude(a) => (json!(a), Value::Null),
        Horizon::CivilTwilight => (json!(-6.0), json!("civil")),
        Horizon::NauticalTwilight => (json!(-12.0), json!("nautical")),
        Horizon::AstronomicalTwilight => (json!(-18.0), json!("astronomical")),
    };
    let (refraction, pressure, temp) = match o.refraction {
        Refraction::Off => (false, Value::Null, Value::Null),
        Refraction::Standard => (true, Value::Null, Value::Null),
        Refraction::Atmosphere { pressure, temperature } => (true, json!(pressure), json!(temperature)),
    };
    let disc = match o.disc {
        DiscAlignment::UpperLimb => "upper",
        DiscAlignment::Center => "center",
        DiscAlignment::LowerLimb => "lower",
    };
    json!({
        "disc": disc, "refraction": refraction, "pressure": pressure, "temp": temp, "fixed": o.fixed_disc,
        "horizon": horizon, "twilight": twilight, "dip": o.dip, "geocentric": o.geocentric, "ignoreLatitude": o.ignore_latitude,
    })
}

/// `/stations`: each body's current speed and its stations: the next `num` (default 4), or
/// all those up to an end date, preceded by the last `before`.
pub async fn stations(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let bodies = q.bodies(&[Body::Mercury, Body::Venus, Body::Mars, Body::Jupiter, Body::Saturn, Body::Uranus, Body::Neptune, Body::Pluto])?;
    let (end, before, num) = (q.jd_end(jd)?, q.before.unwrap_or(0), q.num.unwrap_or(4).min(1_000) as usize);
    let mut list = Vec::with_capacity(bodies.len());
    for body in bodies {
        let set = match (end, &app.stations) {
            (Some(end), _) => app.eph.planetary_stations(body, jd, end.value() - jd.value(), before)?,
            (None, Some(table)) => table.station_set(&app.eph, body, jd, num, before)?,
            (None, None) => app.eph.next_stations(body, jd, num, before)?,
        };
        list.push(json!({
            "key": body.to_string(), "speed": set.speed, "retrograde": set.is_retrograde(),
            "stations": set.stations.iter().map(station_json).collect::<Vec<_>>(),
        }));
    }
    Ok(Json(json!({ "date": moment(jd.value()), "end": end.map(|e| moment(e.value())), "bodies": list })))
}

/// `/moon-phases`: the next `num` principal phases (default 4), or all those up to an end
/// date. Topocentric if the perspective is on the Earth's surface; geocentric otherwise.
pub async fn moon_phases(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let num = q.num.unwrap_or(4).clamp(1, 1_000) as usize;
    // Phases are about 7.4 days apart: this span always holds `num` of them.
    let end = q.jd_end(jd)?.unwrap_or_else(|| jd.add_days(num as f64 * 7.5 + 8.0));
    let topo = q.perspective()?.needs_location();
    let mut phases = if topo { app.eph.moon_phases_topocentric(jd, end, q.require_geo()?)? } else { app.eph.moon_phases(jd, end)? };
    if q.jd_end(jd)?.is_none() {
        phases.truncate(num);
    }
    let (elongation, speed) = app.eph.moon_elongation(jd)?;
    let list: Vec<Value> = phases
        .iter()
        .map(|p| {
            let mut v = moment(p.jd.value());
            v["num"] = json!(p.phase.num());
            v["phase"] = json!(format!("{:?}", p.phase));
            v
        })
        .collect();
    Ok(Json(json!({
        "date": moment(jd.value()), "ps": if topo { "topocentric" } else { "geocentric" },
        "elongation": elongation, "elongation_speed": speed, "waxing": elongation < 180.0, "phases": list,
    })))
}

/// `/indian-day`: the sunrise-to-sunrise day containing the date, and its time units.
pub async fn indian_day(State(app): App, Q(q): Q) -> Reply {
    let (jd, geo, opts) = (q.jd()?, q.require_geo()?, q.rise_set_options()?);
    let d = app.eph.indian_day(jd, geo, opts)?;
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "options": rise_set_json(&opts),
        "rise": moment(d.rise_jd), "set": moment(d.set_jd), "next_rise": moment(d.next_rise_jd),
        "kind": format!("{:?}", d.kind), "is_day_time": d.is_day_time(),
        "day_length": d.day_length(), "daylight_length": d.daylight_length(), "night_length": d.night_length(),
        "progress": d.progress(), "muhurta": d.muhurta(), "ghati": d.ghati(), "vighati": d.vighati(), "lipta": d.lipta(),
        "half_day_muhurta": d.half_day_muhurta(), "weekday": d.weekday_index(geo.solar_offset_secs()),
        "display": d.to_ghati_string(),
    })))
}

/// `/pheno`: phase, elongation, apparent diameter and magnitude of each body.
pub async fn pheno(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let bodies = q.bodies(&[Body::Sun, Body::Moon, Body::Mercury, Body::Venus, Body::Mars, Body::Jupiter, Body::Saturn, Body::Uranus, Body::Neptune, Body::Pluto])?;
    let list: Vec<Value> = app
        .eph
        .phenomena_set(&bodies, jd)?
        .iter()
        .map(|p| {
            json!({
                "key": p.body.to_string(), "phase_angle": p.phase_angle, "illuminated": p.illuminated, "elongation": p.elongation,
                "diameter": p.diameter, "magnitude": if p.magnitude.is_nan() { Value::Null } else { json!(p.magnitude) },
            })
        })
        .collect();
    Ok(Json(json!({ "date": moment(jd.value()), "bodies": list })))
}

/// `/aspects`: every aspect between the bodies at one moment, by ecliptic longitude.
pub async fn aspects(State(app): App, Q(q): Q) -> Reply {
    let (jd, ps) = (q.jd()?, q.perspective()?);
    // Aspects are measured along the ecliptic, so Local Space falls back to topocentric.
    let ps = if ps == Perspective::LocalSpaceHorizon { Perspective::Topocentric } else { ps };
    let geo = if ps.needs_location() { q.require_geo()? } else { q.geo()?.unwrap_or_default() };
    let bodies = q.body_list(ps, &Body::DEFAULT_BODIES)?;
    let targets = q.aspect_orbs()?;
    let list = app.eph.calc_positions(&bodies, jd, geo, CoordSystem::Ecliptic, ps)?;
    let mut found = Vec::new();
    for (i, a) in list.iter().enumerate() {
        for b in &list[i + 1..] {
            for m in astro_calc_core::match_aspects(a.lng, b.lng, &targets) {
                found.push(json!({
                    "a": a.body.to_string(), "b": b.body.to_string(),
                    "aspect": m.target(), "orb": m.orb(), "divergence": m.divergence(),
                    "separation": astro_calc_core::separation(a.lng, b.lng),
                }));
            }
        }
    }
    Ok(Json(json!({
        "date": moment(jd.value()), "ps": ps_key(ps),
        "targets": targets.iter().map(|t| json!({ "angle": t.target(), "orb": t.orb() })).collect::<Vec<_>>(),
        "bodies": list.iter().map(|p| json!({ "key": p.body.to_string(), "lng": p.lng })).collect::<Vec<_>>(),
        "aspects": found,
    })))
}
