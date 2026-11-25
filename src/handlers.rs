//! Endpoint handlers. Each resolves its parameters, calls the core library and shapes JSON.

use std::sync::Arc;

use astro_calc_core::{
    Ayanamsha, Body, BodyPos, BodyPosExt, CoordSystem, GauquelinSectors, GeoPos, HouseSystem, JdUt, Karana, Perspective, Station, Tithi, named_stars,
    to_sidereal,
};
use axum::Json;
use axum::extract::State;
use chrono::SecondsFormat;
use julian_day_converter::{julian_day_to_datetime, julian_day_to_unixtime};
use serde_json::{Value, json};

use crate::AppState;
use crate::error::ApiError;
use crate::nakshatra::Nakshatra;
use crate::params::{HouseChoice, Params, Q, perspective_key};

type Reply = Result<Json<Value>, ApiError>;
type App = State<Arc<AppState>>;

/// Most samples (bodies × times) one progress request may ask for.
const MAX_SAMPLES: f64 = 200_000.0;

/// A moment as Julian Day, ISO UTC string and Unix seconds: the one place a date object is
/// rendered, so every date in every response is converted the same way.
pub(crate) fn moment(jd: f64) -> Value {
    let utc = julian_day_to_datetime(jd).ok().map(|d| d.and_utc().to_rfc3339_opts(SecondsFormat::Secs, true));
    json!({ "jd": jd, "utc": utc, "unix": julian_day_to_unixtime(jd) })
}

fn opt_moment(jd: Option<JdUt>) -> Value {
    jd.map_or(Value::Null, |t| moment(t.value()))
}

/// Adds the dates of a request over a span of time to its response: the moment asked about
/// as `date`, and the two ends of the span as `start_date` and `end_date`. An open-ended
/// span has a `null` `end_date`.
fn with_dates(mut response: Value, jd: JdUt, start: f64, end: Option<f64>) -> Value {
    response["date"] = moment(jd.value());
    response["start_date"] = moment(start);
    response["end_date"] = end.map_or(Value::Null, moment);
    response
}

pub(crate) fn geo_json(geo: GeoPos) -> Value {
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

pub(crate) fn ps_key(ps: Perspective) -> &'static str {
    ps.name()
}

/// A position with its two angles named after the coordinate system: `lng`/`lat`,
/// `ras`/`dcl` or `azi`/`alt`. Local Space is always on the horizon.
pub(crate) fn body_json(p: &BodyPos, cs: CoordSystem, ps: Perspective, sidereal: Option<f64>) -> Value {
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

/// Positions in every coordinate system the perspective allows. An observer on the Earth
/// gets all three. From any other centre there is no horizon, so the ecliptic and
/// equatorial angles are given and `azi`, `alt` and their speeds are null.
fn ext_list(app: &AppState, bodies: &[Body], jd: JdUt, geo: GeoPos, ps: Perspective, sidereal: Option<f64>) -> Result<Vec<Value>, ApiError> {
    if ps.is_earth_based() {
        return Ok(app.eph.calc_positions_ext(bodies, jd, geo, ps)?.iter().map(|p| ext_json(p, sidereal)).collect());
    }
    let ecliptic = app.eph.calc_positions(bodies, jd, geo, CoordSystem::Ecliptic, ps)?;
    let equatorial = app.eph.calc_positions(bodies, jd, geo, CoordSystem::Equatorial, ps)?;
    let none = Value::Null;
    Ok(ecliptic
        .iter()
        .zip(&equatorial)
        .map(|(ecl, equ)| {
            json!({
                "key": ecl.body.to_string(),
                "lng": sidereal.map_or(ecl.lng, |aya| to_sidereal(ecl.lng, aya)), "lat": ecl.lat,
                "ras": equ.lng, "dcl": equ.lat, "azi": none, "alt": none,
                "lng_speed": ecl.lng_speed, "lat_speed": ecl.lat_speed, "ras_speed": equ.lng_speed, "dcl_speed": equ.lat_speed,
                "azi_speed": none, "alt_speed": none,
                "dist": ecl.dist, "dist_speed": ecl.dist_speed,
            })
        })
        .collect())
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
    let applied = if q.sid == Some(true) && (cs == CoordSystem::Ecliptic || q.all_systems()) {
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
            "/bodies", "/stars", "/longitude", "/longitudes", "/position", "/positions", "/extended-position", "/extended-positions", "/progress", "/ascendant", "/ascendant-progress", "/angles", "/houses",
            "/ayanamsha", "/rise-set", "/rise-sets", "/transposed-positions", "/transposed-transits", "/stations", "/moon-phases", "/tithis", "/nakshatra", "/nakshatras", "/indian-day", "/pheno", "/aspects"
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

/// In topocentric mode (an observer at a place on the Earth: `topo` or `local`), adds
/// `points`: the ascendant, MC and the other chart angles of the moment and place. They are
/// tropical longitudes, whatever `sid` says.
fn add_points(v: &mut Value, jd: JdUt, geo: GeoPos, ps: Perspective) {
    if ps.needs_location() {
        v["points"] = angles_json(&astro_calc_core::angles(jd, geo));
    }
}

/// Whether a body can be seen from a perspective: not the body the observer sits on, and,
/// away from the Earth, not the ascendant or the Earth's lunar points.
fn seen_from(body: Body, ps: Perspective) -> bool {
    if ps.is_earth_based() { body != Body::Earth } else { Body::set_for(&[body], ps).contains(&body) }
}

/// Several perspectives returned together: the key that names them (`tg`, `hg`, `htg` or
/// `both`) and the perspectives in the order of their blocks.
struct Combined {
    key: &'static str,
    views: Vec<Perspective>,
}

impl Combined {
    /// Takes a combined `ps` out of the request, which is then resolved as its leading
    /// perspective: topocentric if that is one of them (so a place is required), geocentric
    /// otherwise.
    fn take(q: &mut Params) -> Result<Option<Self>, ApiError> {
        let Some((key, views)) = q.combined_views() else { return Ok(None) };
        let single_horizon = !q.all_systems() && q.coord_system()? == CoordSystem::Horizontal;
        if single_horizon && views.iter().any(|v| !v.is_earth_based()) {
            return Err(ApiError::bad(format!("cs=hor needs an observer on the Earth: it is not available with ps={key}")));
        }
        let lead = if views.contains(&Perspective::Topocentric) { Perspective::Topocentric } else { Perspective::Geocentric };
        q.ps = Some(perspective_key(lead).into());
        Ok(Some(Self { key, views }))
    }

    /// The bodies of the request: those of its leading perspective, then any that only
    /// another of the perspectives adds to a set (the Earth, seen from the Sun).
    fn bodies(&self, q: &Params, mut list: Vec<Body>, default: &[Body]) -> Result<Vec<Body>, ApiError> {
        for view in &self.views {
            for body in q.body_list(*view, default)? {
                if !list.contains(&body) {
                    list.push(body);
                }
            }
        }
        Ok(list)
    }

    /// One object from a block for each perspective, each under its short key (`helio`,
    /// `topo`, `geo`). A body that cannot be seen from a perspective has no block for it.
    fn nest(&self, mut into: Value, mut block: impl FnMut(usize, Perspective) -> Option<Value>) -> Value {
        for (i, view) in self.views.iter().enumerate() {
            if let Some(value) = block(i, *view) {
                into[perspective_key(*view)] = value;
            }
        }
        into
    }
}

/// The key of the perspective in a response: the full name, or the combined key.
fn ps_label(ps: Perspective, combined: &Option<Combined>) -> &'static str {
    combined.as_ref().map_or(ps_key(ps), |c| c.key)
}

/// A position block without its body key, for nesting under a perspective.
fn keyless(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.shift_remove("key");
    }
    v
}

/// What a position request asks for, apart from the perspective.
struct PositionQuery<'a> {
    app: &'a AppState,
    q: &'a Params,
    bodies: &'a [Body],
    jd: JdUt,
    geo: GeoPos,
    cs: CoordSystem,
    sid: Option<f64>,
}

impl PositionQuery<'_> {
    /// Bodies as JSON from one perspective, in one coordinate system or in all three.
    fn seen(&self, bodies: &[Body], ps: Perspective) -> Result<Vec<Value>, ApiError> {
        if self.q.all_systems() {
            ext_list(self.app, bodies, self.jd, self.geo, ps, self.sid)
        } else {
            Ok(self.app.eph.calc_positions(bodies, self.jd, self.geo, self.cs, ps)?.iter().map(|p| body_json(p, self.cs, ps, self.sid)).collect())
        }
    }

    /// The bodies as JSON. With several perspectives each body holds one block for each,
    /// with the same fields, under the perspective's short key.
    fn list(&self, ps: Perspective, combined: &Option<Combined>) -> Result<Vec<Value>, ApiError> {
        let Some(combined) = combined else { return self.seen(self.bodies, ps) };
        let mut blocks = Vec::with_capacity(combined.views.len());
        for view in &combined.views {
            let visible: Vec<Body> = self.bodies.iter().copied().filter(|b| seen_from(*b, *view)).collect();
            blocks.push(self.seen(&visible, *view)?.into_iter());
        }
        Ok(self
            .bodies
            .iter()
            .map(|body| {
                let item = json!({ "key": body.to_string() });
                combined.nest(item, |i, view| if seen_from(*body, view) { blocks[i].next().map(keyless) } else { None })
            })
            .collect())
    }
}

/// `/position`: one body.
pub async fn position(State(app): App, Q(mut q): Q) -> Reply {
    let combined = Combined::take(&mut q)?;
    let jd = q.jd()?;
    let (cs, ps, geo) = q.view_topo()?;
    let body = *q.body_list(ps, &[])?.first().ok_or_else(|| ApiError::bad("a body is required: body=key"))?;
    let (ayanamshas, sid) = ayanamsha_values(&app, &q, jd, cs)?;
    let item = PositionQuery { app: &app, q: &q, bodies: &[body], jd, geo, cs, sid }.list(ps, &combined)?.remove(0);
    let mut v = json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "cs": cs_label(&q, cs, ps), "ps": ps_label(ps, &combined),
        "sidereal": sid.is_some(), "ayanamshas": ayanamshas, "body": item,
    });
    add_points(&mut v, jd, geo, ps);
    Ok(Json(v))
}

/// `/positions`: several bodies at one moment.
pub async fn positions(State(app): App, Q(mut q): Q) -> Reply {
    let combined = Combined::take(&mut q)?;
    let jd = q.jd()?;
    let (cs, ps, geo) = q.view_topo()?;
    let mut bodies = q.body_list(ps, &Body::DEFAULT_BODIES)?;
    if let Some(combined) = &combined {
        bodies = combined.bodies(&q, bodies, &Body::DEFAULT_BODIES)?;
    }
    let (ayanamshas, sid) = ayanamsha_values(&app, &q, jd, cs)?;
    let list = PositionQuery { app: &app, q: &q, bodies: &bodies, jd, geo, cs, sid }.list(ps, &combined)?;
    let mut v = json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "cs": cs_label(&q, cs, ps), "ps": ps_label(ps, &combined),
        "sidereal": sid.is_some(), "ayanamshas": ayanamshas,
        "bodies": list,
    });
    add_points(&mut v, jd, geo, ps);
    Ok(Json(v))
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
        "sidereal": aya.is_some(), "ayanamsha": ayanamsha, "key": body.to_string(), "lng": lng.value,
        "engine": lng.engine.key(),
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
/// equatorial and horizontal), whatever `cs` says. An observer on the Earth needs a place;
/// from any other centre the horizontal angles are null. With several perspectives
/// (`ps=tg`, `hg` or `htg`) the body holds a block for each, with every field.
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
/// Positions are sampled `pd` times a day over the span, which is given in `days` or in
/// `years`, which is shorthand for that many years of 365.24219 days rounded up to a whole
/// day. Every sample has the same fields whichever is used.
pub async fn progress(State(app): App, Q(mut q): Q) -> Reply {
    let combined = Combined::take(&mut q)?;
    let jd = q.jd()?;
    let (cs, ps, geo) = q.view()?;
    let mut bodies = q.body_list(ps, &Body::DEFAULT_BODIES)?;
    if let Some(combined) = &combined {
        bodies = combined.bodies(&q, bodies, &Body::DEFAULT_BODIES)?;
    }
    let views = combined.as_ref().map_or(vec![ps], |c| c.views.clone());
    if q.all_systems() {
        return Err(ApiError::bad("cs=all is not available for /progress; ask for one coordinate system"));
    }
    let per = q.pd.unwrap_or(1.0);
    if !(per.is_finite() && per > 0.0) {
        return Err(ApiError::bad("pd must be positive"));
    }
    let span = q.span_days()?.ok_or_else(|| ApiError::bad("days or years is required"))?;
    if (span * per).ceil().max(1.0) * (bodies.len() * views.len()) as f64 > MAX_SAMPLES {
        return Err(ApiError::bad(format!("too many samples; the limit is {MAX_SAMPLES} positions per request")));
    }
    let interval = 1.0 / per;
    let (ayanamshas, sid) = ayanamsha_values(&app, &q, jd, cs)?;
    // A long series is CPU work: keep it off the async threads. One run for each perspective,
    // over the bodies that can be seen from it.
    let (worker, wanted) = (app.clone(), bodies.clone());
    let runs = tokio::task::spawn_blocking(move || {
        views
            .iter()
            .map(|view| {
                let visible: Vec<Body> = wanted.iter().copied().filter(|b| seen_from(*b, *view)).collect();
                worker.eph.calc_progress_positions(&visible, jd, span, interval, geo, cs, *view).map(|sets| (*view, sets))
            })
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| ApiError { status: axum::http::StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })??;
    let block = |item: &astro_calc_core::BodyJdPos, view: Perspective| keyless(body_json(&item.pos, cs, view, sid));
    let series: Vec<Value> = match &combined {
        // One perspective: each item is the position itself, with its Julian Day.
        None => runs[0]
            .1
            .iter()
            .map(|set| {
                let items: Vec<Value> = set
                    .items
                    .iter()
                    .map(|item| {
                        let mut v = block(item, ps);
                        v["jd"] = json!(item.jd.value());
                        v
                    })
                    .collect();
                json!({ "key": set.body.to_string(), "items": items })
            })
            .collect(),
        // Several: each item has the Julian Day and one block for each perspective.
        Some(combined) => bodies
            .iter()
            .map(|body| {
                let sets: Vec<Option<&astro_calc_core::BodyPosSet>> = runs.iter().map(|(_, sets)| sets.iter().find(|s| s.body == *body)).collect();
                let times = sets.iter().flatten().next().map_or(&[][..], |s| &s.items[..]);
                let items: Vec<Value> = times
                    .iter()
                    .enumerate()
                    .map(|(n, at)| combined.nest(json!({ "jd": at.jd.value() }), |i, view| sets[i].map(|s| block(&s.items[n], view))))
                    .collect();
                json!({ "key": body.to_string(), "items": items })
            })
            .collect(),
    };
    let response = json!({
        "geo": geo_json(geo), "cs": cs_key(cs, ps), "ps": ps_label(ps, &combined), "span": span, "unit": "days", "per_unit": per,
        "sidereal": sid.is_some(), "ayanamshas": ayanamshas, "bodies": series,
    });
    Ok(Json(with_dates(response, jd, jd.value(), Some(jd.value() + span))))
}

/// `/ascendant`: the chart angles of the moment and place as `points`, the ascendant first.
/// The separate `ascendant` field repeats `points.ascendant` and is deprecated: it is kept
/// for older clients.
pub async fn ascendant(Q(q): Q) -> Reply {
    let (jd, geo) = (q.jd()?, q.require_geo()?);
    let points = astro_calc_core::angles(jd, geo);
    Ok(Json(json!({ "date": moment(jd.value()), "geo": geo_json(geo), "ascendant": points.ascendant, "points": angles_json(&points) })))
}

/// `/ascendant-progress`: the ascendant at even steps, from `pdays` days before the moment
/// (default 1) to `days` days after it (default 8), `pd` samples per day (default 24). The
/// moment itself is a sample whenever `pdays × pd` is a whole number. `points` are the chart
/// angles of the moment.
pub async fn ascendant_progress(Q(q): Q) -> Reply {
    let (jd, geo) = (q.jd()?, q.require_geo()?);
    let span = |name: &str, value: Option<f64>, default: f64| match value.unwrap_or(default) {
        d if d.is_finite() && d >= 0.0 => Ok(d),
        _ => Err(ApiError::bad(format!("{name} must not be negative"))),
    };
    let (days, pdays, per) = (span("days", q.days, 8.0)?, span("pdays", q.pdays, 1.0)?, q.pd.unwrap_or(24.0));
    if !(per.is_finite() && per > 0.0) {
        return Err(ApiError::bad("pd must be positive: the number of samples per day"));
    }
    // The small allowance keeps the last sample when the count is whole but for rounding.
    let steps = ((days + pdays) * per + 1e-9).floor();
    if steps + 1.0 > MAX_SAMPLES {
        return Err(ApiError::bad(format!("too many samples; the limit is {MAX_SAMPLES} per request")));
    }
    let start = jd.value() - pdays;
    let ascendants: Vec<Value> = (0..=steps as u64)
        .map(|i| {
            let at = start + i as f64 / per;
            json!({ "jd": at, "lng": astro_calc_core::ascendant(at, geo) })
        })
        .collect();
    let response = json!({
        "geo": geo_json(geo), "days": days, "pdays": pdays, "pd": per,
        "points": angles_json(&astro_calc_core::angles(jd, geo)), "ascendants": ascendants,
    });
    Ok(Json(with_dates(response, jd, start, Some(jd.value() + days))))
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

/// `/houses`: angles and the cusps of each requested house system; `G` gives the 36 Gauquelin
/// sectors. Naming an ayanamsha in `aya` makes the cusps sidereal; the angles stay tropical,
/// as everywhere else.
pub async fn houses(State(app): App, Q(q): Q) -> Reply {
    let (jd, geo, choices) = (q.jd()?, q.require_geo()?, q.house_choices()?);
    let (aya, ayanamsha) = longitude_ayanamsha(&app, &q, jd)?;
    let offset = ayanamsha["value"].as_f64();
    let points = astro_calc_core::angles(jd, geo);
    let code = |s: HouseSystem| s.code().to_string();
    let sets: Vec<Value> = choices
        .iter()
        .map(|&choice| {
            // Whole-sign houses are signs of the zodiac itself: they follow it when it is sidereal.
            let (system, name, mut cusps, fallback, by_sign) = match choice {
                HouseChoice::System(system) => {
                    let h = astro_calc_core::houses(jd, geo, system);
                    (code(system), system.name(), h.cusps.to_vec(), h.fallback, system == HouseSystem::WholeSign)
                }
                HouseChoice::Gauquelin => {
                    let g = astro_calc_core::gauquelin_sectors(jd, geo);
                    (GauquelinSectors::CODE.to_string(), GauquelinSectors::NAME, g.cusps.to_vec(), g.fallback, false)
                }
            };
            if let Some(offset) = offset {
                sidereal_cusps(&mut cusps, by_sign, points.ascendant, offset);
            }
            json!({ "system": system, "name": name, "cusps": cusps, "fallback": fallback.map(code) })
        })
        .collect();
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": geo_json(geo), "sidereal": aya.is_some(), "ayanamsha": ayanamsha,
        "angles": angles_json(&points), "sets": sets,
    })))
}

/// Moves tropical cusps into the sidereal zodiac of an ayanamsha of `offset` degrees. Systems
/// that divide the sky only move back by the offset. Houses that are signs (`by_sign`) start
/// at the sidereal sign the ascendant is in.
fn sidereal_cusps(cusps: &mut [f64], by_sign: bool, ascendant: f64, offset: f64) {
    let first = ((ascendant - offset).rem_euclid(360.0) / 30.0).floor() * 30.0;
    for (i, cusp) in cusps.iter_mut().enumerate() {
        *cusp = if by_sign { (first + 30.0 * i as f64).rem_euclid(360.0) } else { (*cusp - offset).rem_euclid(360.0) };
    }
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

/// Most local days (bodies × days) one `/rise-sets` request may ask for.
const MAX_DAYS: f64 = 20_000.0;

/// `/rise-sets`: rise, upper transit, set and lower transit of each body, with its lowest
/// and highest altitude, for each local day from `pdays` days before the date (default 1) up
/// to `days` days after it (default 8, exclusive): nine days by default. Times are Julian
/// Days (UT); altitudes are degrees.
pub async fn rise_sets(State(app): App, Q(q): Q) -> Reply {
    let (jd, geo, opts) = (q.jd()?, q.require_geo()?, q.rise_set_options()?);
    let bodies = q.bodies(&[Body::Sun, Body::Moon, Body::Mercury, Body::Venus, Body::Mars, Body::Jupiter, Body::Saturn])?;
    let whole = |name: &str, v: Option<f64>, default: f64| match v.unwrap_or(default) {
        n if n.is_finite() && n >= 0.0 && n.fract() == 0.0 => Ok(n),
        n => Err(ApiError::bad(format!("{name} must be a whole number of days, not {n}"))),
    };
    let (days, pdays) = (whole("days", q.days, 8.0)?, whole("pdays", q.pdays, 1.0)?);
    if days + pdays == 0.0 {
        return Err(ApiError::bad("days and pdays are both 0: there are no days to show"));
    }
    if (days + pdays) * bodies.len() as f64 > MAX_DAYS {
        return Err(ApiError::bad(format!("too many days; the limit is {MAX_DAYS} (bodies × days) per request")));
    }
    let (first, n) = (-pdays as i64, (days + pdays) as i64);
    // Each day is several root searches: keep a long span off the async threads.
    let worker = app.clone();
    let list = tokio::task::spawn_blocking(move || -> Result<Vec<Value>, ApiError> {
        let mut list = Vec::with_capacity(bodies.len());
        for body in bodies {
            let mut items = Vec::with_capacity(n as usize);
            for i in first..first + n {
                let d = worker.eph.day_transits(body, jd.add_days(i as f64), geo, opts)?;
                let t = |e: Option<JdUt>| e.map(JdUt::value);
                items.push(json!({
                    "day_start": d.day_start.value(), "rise": t(d.rise), "mc": t(d.mc), "set": t(d.set), "ic": t(d.ic),
                    "alt_min": d.min_alt, "alt_max": d.max_alt,
                }));
            }
            list.push(json!({ "key": body.to_string(), "days": items }));
        }
        Ok(list)
    })
    .await
    .map_err(|e| ApiError { status: axum::http::StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })??;
    let response = json!({ "geo": geo_json(geo), "days": days, "pdays": pdays, "options": rise_set_json(&opts), "bodies": list });
    Ok(Json(with_dates(response, jd, jd.value() - pdays, Some(jd.value() + days))))
}

/// The bodies transposed by default: the Sun, the Moon and the five classical planets.
const TRANSPOSED_BODIES: [Body; 7] = [Body::Sun, Body::Moon, Body::Mercury, Body::Venus, Body::Mars, Body::Jupiter, Body::Saturn];

/// What the two transposed endpoints share: the target moment and place, the historic
/// moment and how it is seen, and the opening fields of the response.
struct Transposed {
    jd: JdUt,
    geo: GeoPos,
    h_jd: JdUt,
    h_geo: Option<GeoPos>,
    ps: Perspective,
    bodies: Vec<Body>,
}

impl Transposed {
    fn new(q: &Params) -> Result<Self, ApiError> {
        let (ps, h_geo) = q.historic_view()?;
        Ok(Self { jd: q.jd()?, geo: q.require_geo()?, h_jd: q.historic_jd()?, h_geo, ps, bodies: q.bodies(&TRANSPOSED_BODIES)? })
    }

    /// `date`, `geo`, `historic_date`, `historic_geo` and `ps`, and in topocentric mode the
    /// chart angles of both moments as `points` and `historic_points`.
    fn head(&self) -> Value {
        let mut v = json!({
            "date": moment(self.jd.value()), "geo": geo_json(self.geo),
            "historic_date": moment(self.h_jd.value()), "historic_geo": self.h_geo.map(geo_json), "ps": ps_key(self.ps),
        });
        if let Some(h_geo) = self.h_geo {
            v["points"] = angles_json(&astro_calc_core::angles(self.jd, self.geo));
            v["historic_points"] = angles_json(&astro_calc_core::angles(self.h_jd, h_geo));
        }
        v
    }
}

/// `/transposed-positions`: where each body stood at the historic moment (`h_dt` or `h_jd`,
/// seen from `h_loc` if given, else from the centre of the Earth), held at that ecliptic
/// longitude and latitude and placed in the sky of the date and place asked about, in all
/// three coordinate systems. Each body is keyed `tp:` and its source, as `tp:su`.
pub async fn transposed_positions(State(app): App, Q(q): Q) -> Reply {
    let t = Transposed::new(&q)?;
    let list: Vec<Value> = app
        .eph
        .calc_transposed_positions(&t.bodies, t.jd, t.h_jd, t.geo, t.h_geo.unwrap_or_default(), t.ps)?
        .iter()
        .map(|p| {
            let mut v = ext_json(p, None);
            v["source"] = json!(p.body.source().map(|source| source.to_string()));
            v
        })
        .collect();
    let mut v = t.head();
    v["bodies"] = json!(list);
    Ok(Json(v))
}

/// `/transposed-transits`: the same fixed points as `/transposed-positions`, with the rise,
/// upper transit, set and lower transit of each and its lowest and highest altitude, for
/// each local day as in `/rise-sets`. `days` defaults to 1 and `pdays` to 0.
///
/// A fixed point has no disc and no parallax, so of the rise/set options only the horizon,
/// refraction, dip and `nolat` apply.
pub async fn transposed_transits(State(app): App, Q(q): Q) -> Reply {
    let (t, opts) = (Transposed::new(&q)?, q.rise_set_options()?);
    let whole = |name: &str, v: Option<f64>, default: f64| match v.unwrap_or(default) {
        n if n.is_finite() && n >= 0.0 && n.fract() == 0.0 => Ok(n),
        n => Err(ApiError::bad(format!("{name} must be a whole number of days, not {n}"))),
    };
    let (days, pdays) = (whole("days", q.days, 1.0)?, whole("pdays", q.pdays, 0.0)?);
    if days + pdays == 0.0 {
        return Err(ApiError::bad("days and pdays are both 0: there are no days to show"));
    }
    if (days + pdays) * t.bodies.len() as f64 > MAX_DAYS {
        return Err(ApiError::bad(format!("too many days; the limit is {MAX_DAYS} (bodies × days) per request")));
    }
    let positions = app.eph.transposed_sources(&t.bodies, t.h_jd, t.h_geo)?;
    let horizon = opts.point_altitude(t.geo.alt);
    let first = astro_calc_core::local_day_start(t.jd.value(), t.geo.lng) - pdays;
    let list: Vec<Value> = positions
        .iter()
        .map(|pos| {
            let pos = astro_calc_core::BodyPos { lat: if opts.ignore_latitude { 0.0 } else { pos.lat }, ..*pos };
            let items: Vec<Value> = astro_calc_core::transposed_point_transit_days(&pos, first, t.geo, horizon, (days + pdays) as u32)
                .iter()
                .map(|d| {
                    let time = |e: Option<JdUt>| e.map(JdUt::value);
                    json!({
                        "day_start": d.start.value(), "rise": time(d.rise), "mc": d.mc.value(), "set": time(d.set), "ic": d.ic.value(),
                        "alt_min": d.min_alt, "alt_max": d.max_alt,
                    })
                })
                .collect();
            json!({ "key": Body::transposed(pos.body).to_string(), "source": pos.body.to_string(), "lng": pos.lng, "lat": pos.lat, "days": items })
        })
        .collect();
    let mut v = t.head();
    v["days"] = json!(days);
    v["pdays"] = json!(pdays);
    v["horizon_alt"] = json!(horizon);
    v["bodies"] = json!(list);
    Ok(Json(v))
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
        "horizon": horizon, "twilight": twilight, "dip": o.dip, "geocentric": o.geocentric, "ignore_latitude": o.ignore_latitude,
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
    Ok(Json(with_dates(json!({ "bodies": list }), jd, jd.value(), end.map(JdUt::value))))
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
    let (elongation, speed) = app.eph.moon_elongation(jd)?.value;
    let list: Vec<Value> = phases
        .iter()
        .map(|p| {
            let mut v = moment(p.jd.value());
            v["num"] = json!(p.phase.num());
            v["phase"] = json!(format!("{:?}", p.phase));
            v
        })
        .collect();
    let response = json!({
        "ps": if topo { "topocentric" } else { "geocentric" },
        "elongation": elongation, "elongation_speed": speed, "waxing": elongation < 180.0, "phases": list,
    });
    Ok(Json(with_dates(response, jd, jd.value(), q.jd_end(jd)?.map(JdUt::value))))
}

fn tithi_json(t: Tithi) -> Value {
    json!({
        "num": t.num(), "paksha": t.paksha().name(), "day": t.day(), "name": t.name(),
        "phase": t.phase_at_start().map(|p| format!("{p:?}")),
    })
}

fn karana_json(k: Karana) -> Value {
    json!({
        "num": k.num(), "name": k.name().name(), "tithi": k.tithi().num(), "half": if k.is_first_half() { 1 } else { 2 },
        "phase": k.phase_at_start().map(|p| format!("{p:?}")),
    })
}

/// A moment with the fields of what begins then.
fn begins(jd: JdUt, what: Value) -> Value {
    let mut v = moment(jd.value());
    for (key, value) in what.as_object().into_iter().flatten() {
        v[key] = value.clone();
    }
    v
}

/// `/tithis`: the lunar days of the Indian calendar, each with the moment it begins. The
/// list opens with the tithi in force at the date and runs for `num` tithis (default 30, a
/// lunar month), or up to an end date. With `karanas=1` it lists the sixty half tithis
/// instead. Always geocentric, so the times are the same everywhere; no ayanamsha is involved.
pub async fn tithis(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let halves = q.karanas == Some(true);
    let num = q.num.unwrap_or(30).clamp(1, 1_000) as usize;
    // The longest tithi is under 27 hours and the longest karana half that: the search opens
    // far enough back to catch the one in force, and by default runs far enough to hold `num`.
    let longest = if halves { 0.57 } else { 1.14 };
    let end = q.jd_end(jd)?;
    let (from, to) = (jd.add_days(-longest), end.unwrap_or_else(|| jd.add_days(num as f64 * longest)));
    let mut list: Vec<(f64, Value)> = if halves {
        app.eph.karanas(from, to)?.iter().map(|e| (e.jd.value(), begins(e.jd, karana_json(e.karana)))).collect()
    } else {
        app.eph.tithis(from, to)?.iter().map(|e| (e.jd.value(), begins(e.jd, tithi_json(e.tithi)))).collect()
    };
    let first = list.iter().rposition(|(start, _)| *start <= jd.value()).unwrap_or(0);
    list.drain(..first);
    if end.is_none() {
        list.truncate(num);
    }
    let (elongation, speed) = app.eph.moon_elongation(jd)?.value;
    let mut v = json!({
        "ps": "geocentric",
        "elongation": elongation, "elongation_speed": speed,
        "tithi": tithi_json(Tithi::from_elongation(elongation)), "karana": karana_json(Karana::from_elongation(elongation)),
    });
    v[if halves { "karanas" } else { "tithis" }] = list.into_iter().map(|(_, item)| item).collect();
    Ok(Json(with_dates(v, jd, jd.value(), end.map(JdUt::value))))
}

/// How far either side of a date a body's entry into its nakshatra and its exit are looked
/// for, in days: forty years, longer than Pluto ever spends in one.
const STAY_SEARCH_DAYS: f64 = 14_610.0;
/// The most nakshatras `/nakshatras` lists for one body.
const MAX_NAKSHATRAS: usize = 2_000;

/// `/nakshatras`: the nakshatras a body passes through, each with the moment it enters,
/// `start_jd`. The Moon unless `bodies` names others. Each list opens with the nakshatra in
/// force at the date and runs for `num` of them (default 27, once round for the Moon), or up
/// to an end date. Geocentric; the ayanamsha is the first in `aya`, True Citra if none is given.
pub async fn nakshatras(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let bodies = q.body_list(Perspective::Geocentric, &[Body::Moon])?;
    if bodies.contains(&Body::Ascendant) {
        return Err(ApiError::bad("the ascendant has no listing: it passes through every nakshatra each day"));
    }
    let aya = q.ayanamshas()?.first().copied().unwrap_or(Ayanamsha::TrueCitra);
    let end = q.jd_end(jd)?;
    if end.is_some_and(|e| e.value() < jd.value()) {
        return Err(ApiError::bad("the end date is before the start"));
    }
    // After the one in force: `num - 1` more within the search span, or all up to the end date.
    let (more, max_days) = match end {
        Some(e) => (MAX_NAKSHATRAS - 1, e.value() - jd.value()),
        None => (q.num.unwrap_or(27).clamp(1, MAX_NAKSHATRAS as u32) as usize - 1, STAY_SEARCH_DAYS),
    };
    let span = crate::nakshatra::SPAN;
    // A slow planet's passage is a search over years: keep it off the async threads.
    let worker = app.clone();
    let list = tokio::task::spawn_blocking(move || -> Result<Vec<Value>, ApiError> {
        let mut list = Vec::with_capacity(bodies.len());
        for body in bodies {
            let stay = worker.eph.longitude_stay(body, jd, span, Some(aya), STAY_SEARCH_DAYS)?;
            let item = |index: u32, start: Option<f64>, direct: Option<bool>| {
                let mut v = Nakshatra::from_index(index).summary_json();
                v["start_jd"] = json!(start);
                v["direct"] = json!(direct);
                v
            };
            let mut items = vec![item(stay.division, stay.entered.map(|c| c.jd.value()), stay.entered.map(|c| c.direct))];
            let crossings = worker.eph.next_longitude_crossings(body, jd, more, span, Some(aya), max_days)?;
            items.extend(crossings.iter().map(|c| item(c.into, Some(c.jd.value()), Some(c.direct))));
            list.push(json!({ "key": body.to_string(), "items": items }));
        }
        Ok(list)
    })
    .await
    .map_err(|e| ApiError { status: axum::http::StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })??;
    let response = json!({
        "ps": "geocentric",
        "ayanamsha": { "key": aya.key(), "name": aya.name(), "value": app.eph.ayanamsha(jd, aya)? },
        "span": span, "bodies": list,
    });
    Ok(Json(with_dates(response, jd, jd.value(), end.map(JdUt::value))))
}

/// `/nakshatra`: the lunar mansion of the Moon (the birth star, at a birth), or of each body
/// in `bodies`: one of 27 divisions of 13°20′ of the sidereal zodiac, with its pada and lord.
/// Geocentric. The ayanamsha is the first one in `aya`, True Citra if none is given. The
/// ascendant (`as`) may be listed when a place is given. Each body but the ascendant also
/// has the moments it came into the nakshatra and leaves it, `start_jd` and `end_jd`.
pub async fn nakshatra(State(app): App, Q(q): Q) -> Reply {
    let jd = q.jd()?;
    let place = q.geo()?;
    let bodies = q.body_list(Perspective::Geocentric, &[Body::Moon])?;
    let aya = q.ayanamshas()?.first().copied().unwrap_or(Ayanamsha::TrueCitra);
    let list = app.eph.calc_longitudes(&bodies, jd, place.unwrap_or_default(), Perspective::Geocentric, Some(aya))?;
    // A slow planet's stay is a search over years: keep it off the async threads.
    let worker = app.clone();
    let items = tokio::task::spawn_blocking(move || -> Result<Vec<Value>, ApiError> {
        let mut items = Vec::with_capacity(list.len());
        for l in &list {
            let mut v = json!({ "key": l.body.to_string(), "lng": l.lng });
            for (field, value) in Nakshatra::from_sidereal(l.lng).json().as_object().into_iter().flatten() {
                v[field] = value.clone();
            }
            // The ascendant goes round the whole zodiac in a day: it has no stay worth timing.
            if l.body != Body::Ascendant {
                let stay = worker.eph.longitude_stay(l.body, jd, crate::nakshatra::SPAN, Some(aya), STAY_SEARCH_DAYS)?;
                v["start_jd"] = json!(stay.entered.map(|c| c.jd.value()));
                v["end_jd"] = json!(stay.left.map(|c| c.jd.value()));
            }
            items.push(v);
        }
        Ok(items)
    })
    .await
    .map_err(|e| ApiError { status: axum::http::StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })??;
    Ok(Json(json!({
        "date": moment(jd.value()), "geo": place.map(geo_json), "ps": "geocentric",
        "ayanamsha": { "key": aya.key(), "name": aya.name(), "value": app.eph.ayanamsha(jd, aya)? },
        "span": crate::nakshatra::SPAN, "bodies": items,
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
