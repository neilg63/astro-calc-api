//! Bulk endpoints: charts for many records at once, and aspect searches between records.
//!
//! These are `POST` requests. The records travel in the JSON body; everything else (bodies,
//! aspects, perspective, ayanamsha, tag filter) is given in the query string with the same
//! parameters as the other endpoints. Each record's `ref` and `tags` are echoed back, so a
//! client can match results to its own data.

use std::sync::Arc;

use astro_calc_core::{AspectCriteria, AspectHit, AspectMatch, Body, CalcError, ChartInput, ChartKind, ChartOptions, CoordSystem, GeoPos, JdUt, KeyedError, Perspective};
use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::error::ApiError;
use crate::handlers::{body_json, geo_json, moment, ps_key};
use crate::params::{Params, Q};

type Reply = Result<Json<Value>, ApiError>;
type App = State<Arc<AppState>>;
type Body_ = Result<Json<Value>, JsonRejection>;

/// Most records one request may carry.
pub const MAX_RECORDS: usize = 100_000;
/// Most records for the many-to-many search, whose work grows with the square of the list.
pub const MAX_MANY_TO_MANY: usize = 5_000;
/// Largest request body accepted by the bulk endpoints, in bytes.
pub const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// One record: a moment and a place, with the client's own reference and tags.
#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    /// The client's identifier, echoed back.
    #[serde(rename = "ref")]
    pub reference: String,
    pub lat: f64,
    pub lng: f64,
    #[serde(default)]
    pub alt: f64,
    /// UTC date-time, `YYYY-MM-DDThh:mm:ss` with an optional `Z`.
    pub dt: Option<String>,
    /// Julian Day (UT); takes precedence over `dt`.
    pub jd: Option<f64>,
    /// Anything the client wants back; also used by the `tags` filter.
    #[serde(default)]
    pub tags: Value,
}

#[derive(Debug, Deserialize)]
pub struct Pair {
    pub a: Record,
    pub b: Record,
}

/// The items of a list sent either bare or as `{ "items": [...] }`.
fn list_items(body: Value) -> Result<Vec<Value>, ApiError> {
    match body {
        Value::Array(items) => Ok(items),
        Value::Object(mut map) => match map.remove("items") {
            Some(Value::Array(items)) => Ok(items),
            _ => Err(ApiError::bad("the body must be a list, or an object with a list under \"items\"")),
        },
        _ => Err(ApiError::bad("the body must be a list, or an object with a list under \"items\"")),
    }
}

/// Reads each item on its own, so that one malformed item is reported under `errors` (with
/// its position in the list, and its `ref` if it has one) without failing the others.
fn parse_items<T: serde::de::DeserializeOwned>(items: Vec<Value>, errors: &mut Vec<Value>) -> Vec<T> {
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        let reference = item.get("ref").cloned().unwrap_or(Value::Null);
        match serde_json::from_value(item) {
            Ok(parsed) => out.push(parsed),
            Err(e) => errors.push(json!({ "index": index, "ref": reference, "error": format!("invalid item: {e}") })),
        }
    }
    out
}

impl Record {
    fn input(&self, key: usize) -> Result<ChartInput<usize>, String> {
        let jd = match (self.jd, &self.dt) {
            (Some(jd), _) if jd.is_finite() => JdUt(jd),
            (Some(_), _) => return Err("invalid jd".into()),
            (None, Some(dt)) => JdUt::from_iso(dt.trim().trim_end_matches(['Z', 'z'])).map_err(|_| format!("invalid date-time: {dt}"))?,
            (None, None) => return Err("a date is required: dt or jd".into()),
        };
        let geo = GeoPos::new(self.lat, self.lng, self.alt).map_err(|e| format!("invalid location: {e}"))?;
        Ok(ChartInput::new(key, jd, geo))
    }

    fn label(&self) -> Value {
        if self.tags.is_null() { json!({ "ref": self.reference }) } else { json!({ "ref": self.reference, "tags": self.tags }) }
    }

    /// Whether every `key:value` of the filter is among the record's tags.
    fn has_tags(&self, filter: &[(String, String)]) -> bool {
        filter.iter().all(|(key, wanted)| match self.tags.get(key) {
            Some(Value::String(s)) => s == wanted,
            Some(Value::Null) | None => false,
            // Numbers and booleans are compared by their JSON text.
            Some(other) => &other.to_string() == wanted,
        })
    }
}

fn error_json(reference: &str, message: impl ToString) -> Value {
    json!({ "ref": reference, "error": message.to_string() })
}

fn body(body: Body_) -> Result<Value, ApiError> {
    body.map(|Json(v)| v).map_err(|e| ApiError::bad(format!("invalid JSON body: {}", e.body_text())))
}

fn check_size(n: usize, max: usize) -> Result<(), ApiError> {
    if n > max { Err(ApiError { status: StatusCode::PAYLOAD_TOO_LARGE, message: format!("too many records: {n} (limit {max})") }) } else { Ok(()) }
}

/// The records that pass the tag filter and have a valid date and place, keyed by their
/// index in `records`; the invalid ones are added to `errors`.
fn inputs(records: &[Record], filter: &[(String, String)], errors: &mut Vec<Value>) -> Vec<ChartInput<usize>> {
    let mut out = Vec::with_capacity(records.len());
    for (i, record) in records.iter().enumerate().filter(|(_, r)| r.has_tags(filter)) {
        match record.input(i) {
            Ok(input) => out.push(input),
            Err(message) => errors.push(error_json(&record.reference, message)),
        }
    }
    out
}

fn calc_errors(records: &[Record], failed: Vec<KeyedError<usize>>, errors: &mut Vec<Value>) {
    errors.extend(failed.into_iter().map(|e| error_json(&records[e.key].reference, e.error)));
}

/// Runs CPU-bound work off the async threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work).await.map_err(|e| ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, message: e.to_string() })
}

/// The earthly perspective of a bulk request: geocentric unless `ps` says otherwise.
fn perspective(q: &Params) -> Result<Perspective, ApiError> {
    let ps = q.perspective()?;
    if ps.is_earth_based() && ps != Perspective::LocalSpaceHorizon { Ok(ps) } else { Err(ApiError::bad("bulk requests need ps=geo or ps=topo")) }
}

/// Aspect criteria from the query string: `bodies` for chart A, `bodies2` for chart B
/// (default: the same), `aspects`/`orb`, `aya` with `sid=1`, and `prog` with a date.
fn criteria(q: &Params) -> Result<AspectCriteria, ApiError> {
    let bodies_a = q.bodies(&Body::LUMINARIES_AND_PLANETS)?;
    let bodies_b = q.bodies2()?.unwrap_or_else(|| bodies_a.clone());
    let mut criteria = AspectCriteria::new(&bodies_a, &bodies_b, &q.aspect_orbs()?);
    criteria.perspective = perspective(q)?;
    criteria.ayanamsha = sidereal(q)?;
    if let Some((kind_a, kind_b)) = q.progressed()? {
        criteria = criteria.progressed_to(q.jd()?, kind_a, kind_b);
    }
    Ok(criteria)
}

fn sidereal(q: &Params) -> Result<Option<astro_calc_core::Ayanamsha>, ApiError> {
    if q.sid != Some(1) {
        return Ok(None);
    }
    q.ayanamshas()?.first().copied().map(Some).ok_or_else(|| ApiError::bad("sid=1 needs an ayanamsha: aya=key"))
}

fn criteria_json(c: &AspectCriteria) -> Value {
    let keys = |bodies: &[Body]| bodies.iter().map(|b| b.to_string()).collect::<Vec<_>>();
    let kind = |k: ChartKind| if k == ChartKind::Progressed { "progressed" } else { "natal" };
    let mut v = json!({
        "ps": ps_key(c.perspective),
        "sidereal": c.ayanamsha.and_then(|a| a.key()),
        "bodies": keys(&c.bodies_a), "bodies2": keys(&c.bodies_b),
        "targets": c.aspects.iter().map(|t| json!({ "angle": t.target(), "orb": t.orb() })).collect::<Vec<_>>(),
    });
    if let Some(at) = c.at {
        v["progressed"] = json!({ "date": moment(at.value()), "a": kind(c.kind_a), "b": kind(c.kind_b) });
    }
    v
}

fn hit_json(h: &AspectHit) -> Value {
    json!({
        "a": h.body_a.to_string(), "b": h.body_b.to_string(), "lng_a": h.lng_a, "lng_b": h.lng_b,
        "aspect": h.angle, "orb": h.orb, "divergence": h.deviation.abs(),
    })
}

fn hits_json(m: &AspectMatch<usize>) -> Value {
    Value::Array(m.hits.iter().map(hit_json).collect())
}

fn done(mut reply: Value, count_key: &str, list: Vec<Value>, errors: Vec<Value>) -> Reply {
    reply["count"] = json!(list.len());
    reply[count_key] = Value::Array(list);
    reply["errors"] = Value::Array(errors);
    Ok(Json(reply))
}

/// `POST /bulk/charts`: positions of `bodies` for every record.
pub async fn charts(State(app): App, Q(q): Q, records: Body_) -> Reply {
    let mut errors = Vec::new();
    let records: Vec<Record> = parse_items(list_items(body(records)?)?, &mut errors);
    check_size(records.len(), MAX_RECORDS)?;
    let ps = perspective(&q)?;
    let opts = ChartOptions { bodies: q.bodies(&Body::DEFAULT_BODIES)?, perspective: ps, ayanamsha: sidereal(&q)? };
    let filter = q.tag_filter()?;
    let reply = json!({ "ps": ps_key(ps), "sidereal": opts.ayanamsha.and_then(|a| a.key()) });
    blocking(move || {
        let found = app.eph.bulk_charts(&inputs(&records, &filter, &mut errors), &opts);
        calc_errors(&records, found.errors, &mut errors);
        let list = found
            .charts
            .iter()
            .map(|chart| {
                let record = &records[chart.key];
                let mut v = record.label();
                v["date"] = moment(chart.jd.value());
                v["geo"] = geo_json(GeoPos { lat: record.lat, lng: record.lng, alt: record.alt });
                v["engine"] = json!(chart.engine.key());
                v["bodies"] = chart.positions.iter().map(|p| body_json(p, CoordSystem::Ecliptic, ps, None)).collect();
                v
            })
            .collect();
        done(reply, "charts", list, errors)
    })
    .await?
}

/// `POST /bulk/aspects/pairs`: aspects within each given pair of records.
pub async fn aspects_pairs(State(app): App, Q(q): Q, pairs: Body_) -> Reply {
    let mut errors = Vec::new();
    let pairs: Vec<Pair> = parse_items(list_items(body(pairs)?)?, &mut errors);
    check_size(pairs.len() * 2, MAX_RECORDS)?;
    let criteria = criteria(&q)?;
    blocking(move || {
        // Keys are the pair's index; a pair with an invalid record is reported and left out.
        let mut inputs = Vec::with_capacity(pairs.len());
        for (i, pair) in pairs.iter().enumerate() {
            match (pair.a.input(i), pair.b.input(i)) {
                (Ok(a), Ok(b)) => inputs.push((a, b)),
                (a, b) => {
                    errors.extend(a.err().map(|m| error_json(&pair.a.reference, m)));
                    errors.extend(b.err().map(|m| error_json(&pair.b.reference, m)));
                }
            }
        }
        let found = app.eph.aspects_in_pairs(&inputs, &criteria);
        // Both records of a pair share its index, so a failure is traced by re-testing each.
        for failed in found.errors {
            let pair = &pairs[failed.key];
            let reference = if is_calculable(&app, &pair.a, &criteria) { &pair.b.reference } else { &pair.a.reference };
            errors.push(error_json(reference, failed.error));
        }
        let list = found
            .matches
            .iter()
            .map(|m| json!({ "a": pairs[m.a].a.label(), "b": pairs[m.b].b.label(), "closest": m.closest(), "aspects": hits_json(m) }))
            .collect();
        done(criteria_json(&criteria), "matches", list, errors)
    })
    .await?
}

fn is_calculable(app: &AppState, record: &Record, criteria: &AspectCriteria) -> bool {
    let check = |input: ChartInput<usize>| -> Result<(), CalcError> {
        app.eph.calc_longitudes(&criteria.bodies_a, input.jd, input.geo, criteria.perspective, None).map(|_| ())
    };
    record.input(0).is_ok_and(|input| check(input).is_ok())
}

/// `POST /bulk/aspects/one-to-many`: aspects between `source` and each record of `target`.
pub async fn aspects_one_to_many(State(app): App, Q(q): Q, request: Body_) -> Reply {
    let mut request = body(request)?;
    let source = match request["source"].take() {
        Value::Null => return Err(ApiError::bad("a record is required under \"source\"")),
        source => serde_json::from_value::<Record>(source).map_err(|e| ApiError::bad(format!("invalid source: {e}")))?,
    };
    let targets = match (request["target"].take(), request["targets"].take()) {
        (Value::Array(items), _) | (_, Value::Array(items)) => items,
        _ => return Err(ApiError::bad("a list of records is required under \"target\"")),
    };
    let mut errors = Vec::new();
    let target: Vec<Record> = parse_items(targets, &mut errors);
    check_size(target.len(), MAX_RECORDS)?;
    let criteria = criteria(&q)?;
    let filter = q.tag_filter()?;
    let source_input = source.input(usize::MAX).map_err(|m| ApiError::bad(format!("source: {m}")))?;
    blocking(move || {
        let found = app.eph.aspects_one_to_many(&source_input, &inputs(&target, &filter, &mut errors), &criteria)?;
        calc_errors(&target, found.errors, &mut errors);
        let list = found
            .matches
            .iter()
            .map(|m| {
                let mut v = target[m.b].label();
                v["closest"] = json!(m.closest());
                v["aspects"] = hits_json(m);
                v
            })
            .collect();
        let mut reply = criteria_json(&criteria);
        reply["source"] = source.label();
        done(reply, "matches", list, errors)
    })
    .await?
}

/// `POST /bulk/aspects/many-to-many`: aspects between every two records of one list.
pub async fn aspects_many_to_many(State(app): App, Q(q): Q, records: Body_) -> Reply {
    let mut errors = Vec::new();
    let records: Vec<Record> = parse_items(list_items(body(records)?)?, &mut errors);
    let criteria = criteria(&q)?;
    let filter = q.tag_filter()?;
    blocking(move || {
        let inputs = inputs(&records, &filter, &mut errors);
        check_size(inputs.len(), MAX_MANY_TO_MANY)?;
        let found = app.eph.aspects_many_to_many(&inputs, &criteria);
        calc_errors(&records, found.errors, &mut errors);
        let list = found
            .matches
            .iter()
            .map(|m| json!({ "a": records[m.a].label(), "b": records[m.b].label(), "closest": m.closest(), "aspects": hits_json(m) }))
            .collect();
        done(criteria_json(&criteria), "matches", list, errors)
    })
    .await?
}
