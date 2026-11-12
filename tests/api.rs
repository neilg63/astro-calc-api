//! Endpoint tests against the router, without a network socket.
//! They need the ephemeris files in `../astro-calc-core/data` and are skipped otherwise.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use astro_calc_api::{AppState, router};
use astro_calc_core::*;
use axum::body::Body as HttpBody;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn state() -> Option<Arc<AppState>> {
    static STATE: OnceLock<Option<Arc<AppState>>> = OnceLock::new();
    STATE
        .get_or_init(|| {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../astro-calc-core/data");
            dir.join("de440s.bsp").exists().then(|| Arc::new(AppState::load(dir).unwrap()))
        })
        .clone()
}

async fn get(state: &Arc<AppState>, path: &str) -> (StatusCode, Value) {
    let response = router(state.clone()).oneshot(Request::get(path).body(HttpBody::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn ok(state: &Arc<AppState>, path: &str) -> Value {
    let (status, body) = get(state, path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

const JD: f64 = 2_450_094.975_694_444_5; // 1996-01-12T11:25:00 UTC

#[tokio::test]
async fn dates_offsets_and_locations() {
    let Some(s) = state() else { return };
    let utc = ok(&s, "/positions?dt=1996-01-12T11:25:00&bodies=su").await;
    assert!((f(&utc["date"]["jd"]) - JD).abs() < 1e-8);
    assert_eq!(utc["date"]["utc"], "1996-01-12T11:25:00Z");
    assert_eq!(utc["date"]["unix"], 821_445_900);
    // The same moment given as local time one hour east of Greenwich, three ways, and as a JD.
    for query in ["dt=1996-01-12T12:25:00&tzoh=1", "dt=1996-01-12T12:25:00&tzom=60", "dt=1996-01-12T12:25:00&tzos=3600", "jd=2450094.9756944445&tzoh=5"] {
        let v = ok(&s, &format!("/positions?{query}&bodies=su")).await;
        assert!((f(&v["date"]["jd"]) - JD).abs() < 1e-8, "{query}");
        assert_eq!(v["bodies"][0]["lng"], utc["bodies"][0]["lng"], "{query}");
    }
    let half = ok(&s, "/positions?dt=1996-01-12T17:25&tzoh=5.5&bodies=su").await;
    assert!((f(&half["date"]["jd"]) - (JD + 0.5 / 24.0)).abs() < 1e-8);
    // Partial date-times fill in from the left.
    assert_eq!(ok(&s, "/ayanamsha?dt=1990-08-23T12&aya=lh").await["date"]["utc"], "1990-08-23T12:00:00Z");
    assert_eq!(ok(&s, "/ayanamsha?dt=1990-08-23&aya=lh").await["date"]["utc"], "1990-08-23T00:00:00Z");
    // No date at all means now.
    let now = ok(&s, "/ayanamsha?aya=lh").await;
    assert!(f(&now["date"]["jd"]) > 2_460_000.0);

    // lat, lng and alt each override their part of loc.
    let v = ok(&s, "/angles?dt=1996-01-12T11:25:00&loc=56.1,-3.6,120&lat=48.5&alt=30").await;
    assert_eq!((f(&v["geo"]["lat"]), f(&v["geo"]["lng"]), f(&v["geo"]["alt"])), (48.5, -3.6, 30.0));
    let v = ok(&s, "/angles?dt=1996-01-12T11:25:00&lat=10&lng=20").await;
    assert_eq!((f(&v["geo"]["lat"]), f(&v["geo"]["lng"]), f(&v["geo"]["alt"])), (10.0, 20.0, 0.0));
}

#[tokio::test]
async fn positions_match_the_core_library() {
    let Some(s) = state() else { return };
    let geo = GeoPos::new(56.1, -3.6, 0.0).unwrap();
    for (cs_key, cs) in [("ecl", CoordSystem::Ecliptic), ("equatorial", CoordSystem::Equatorial), ("hor", CoordSystem::Horizontal)] {
        for (ps_key, ps) in [("geo", Perspective::Geocentric), ("topocentric", Perspective::Topocentric)] {
            let v = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=su,mo,ma,ra,ml,kr,hip:65474&cs={cs_key}&ps={ps_key}")).await;
            let bodies = parse_body_list("su,mo,ma,ra,ml,kr,hip:65474").unwrap();
            let expected = s.eph.calc_positions(&bodies, JD, geo, cs, ps).unwrap();
            assert_eq!(v["bodies"].as_array().unwrap().len(), expected.len());
            for (got, want) in v["bodies"].as_array().unwrap().iter().zip(&expected) {
                assert_eq!(got["key"], want.body.to_string());
                // JSON text round-trips a float to within its last digit.
                for (key, value) in [("lng", want.lng), ("lat", want.lat), ("lng_speed", want.lng_speed), ("dist", want.dist)] {
                    assert!((f(&got[key]) - value).abs() <= 1e-12 * value.abs().max(1.0), "{} {key}", want.body);
                }
            }
        }
    }
    // Defaults: the core bodies, ecliptic, geocentric. Legacy aliases still work.
    let v = ok(&s, &format!("/positions?jd={JD}")).await;
    assert_eq!((v["cs"].as_str(), v["ps"].as_str(), v["bodies"].as_array().unwrap().len()), (Some("ecliptic"), Some("geocentric"), Body::CORE.len()));
    let v = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&topo=1&eq=1&bodies=mo")).await;
    assert_eq!((v["cs"].as_str(), v["ps"].as_str()), (Some("equatorial"), Some("topocentric")));
    // One body, and sidereal longitudes.
    let one = ok(&s, &format!("/position?jd={JD}&body=ve")).await;
    let sid = ok(&s, &format!("/position?jd={JD}&body=ve&aya=lh&sid=1")).await;
    let aya = f(&sid["ayanamshas"][0]["value"]);
    assert!((f(&one["body"]["lng"]) - aya - f(&sid["body"]["lng"])).rem_euclid(360.0) < 1e-9);
    assert_eq!(sid["sidereal"], true);
}

#[tokio::test]
async fn chart_endpoints() {
    let Some(s) = state() else { return };
    let geo = GeoPos::new(56.1, -3.6, 0.0).unwrap();
    let q = format!("jd={JD}&loc=56.1,-3.6");
    let a = angles(JD, geo);
    assert_eq!(f(&ok(&s, &format!("/ascendant?{q}")).await["ascendant"]), a.ascendant);
    let v = ok(&s, &format!("/angles?{q}")).await;
    assert_eq!((f(&v["angles"]["mc"]), f(&v["angles"]["vertex"])), (a.mc, a.vertex));
    let v = ok(&s, &format!("/houses?{q}&hsys=P,U,w")).await;
    let sets = v["sets"].as_array().unwrap();
    assert_eq!(sets.iter().map(|x| x["system"].as_str().unwrap()).collect::<Vec<_>>(), ["P", "U", "W"]);
    assert_eq!(f(&sets[0]["cusps"][1]), houses(JD, geo, HouseSystem::Placidus).cusps[1]);
    assert_eq!(ok(&s, &format!("/houses?{q}&hsys=all")).await["sets"].as_array().unwrap().len(), HouseSystem::ALL.len());
    assert_eq!(ok(&s, &format!("/houses?{q}")).await["sets"][0]["system"], "W");

    let v = ok(&s, &format!("/ayanamsha?jd={JD}&aya=lh,tc")).await;
    assert_eq!(f(&v["ayanamshas"][0]["value"]), s.eph.ayanamsha(JD, Ayanamsha::Lahiri).unwrap());
    assert_eq!(v["ayanamshas"][1]["key"], "tc");
    assert!(ok(&s, &format!("/ayanamsha?jd={JD}")).await["ayanamshas"].as_array().unwrap().len() >= 17);

    let v = ok(&s, &format!("/rise-set?{q}&bodies=su,mo,hip:65474")).await;
    let sun = s.eph.day_transits(Body::Sun, JD, geo, RiseSetOptions::default()).unwrap();
    assert_eq!(f(&v["bodies"][0]["rise"]["jd"]), sun.rise.unwrap().value());
    assert_eq!(v["bodies"].as_array().unwrap().len(), 3);
    let upper = ok(&s, &format!("/rise-set?{q}&bodies=su&mode=0")).await;
    assert!(f(&upper["bodies"][0]["rise"]["jd"]) != sun.rise.unwrap().value());

    let v = ok(&s, &format!("/indian-day?{q}")).await;
    let day = s.eph.indian_day(JD, geo, RiseSetOptions::default()).unwrap();
    assert_eq!((f(&v["rise"]["jd"]), v["ghati"].as_u64(), v["display"].as_str()), (day.rise_jd, Some(day.ghati() as u64), Some(day.to_ghati_string().as_str())));

    let v = ok(&s, &format!("/pheno?jd={JD}&bodies=ve,ce")).await;
    assert_eq!(f(&v["bodies"][0]["magnitude"]), s.eph.phenomena(Body::Venus, JD).unwrap().magnitude);
}

#[tokio::test]
async fn events_and_series() {
    let Some(s) = state() else { return };
    let v = ok(&s, &format!("/stations?jd={JD}&bodies=me,ma&num=5&before=2")).await;
    let mercury = &v["bodies"][0];
    assert_eq!((mercury["key"].as_str(), mercury["retrograde"].as_bool()), (Some("me"), Some(true)));
    assert_eq!(mercury["stations"].as_array().unwrap().len(), 7);
    let live = s.eph.next_stations(Body::Mercury, JD, 5, 2).unwrap();
    for (got, want) in mercury["stations"].as_array().unwrap().iter().zip(&live.stations) {
        assert_eq!(got["type"], want.kind().key());
        assert!((f(&got["jd"]) - want.jd()).abs() < 1e-4);
    }
    let ranged = ok(&s, &format!("/stations?jd={JD}&bodies=me&days=365")).await;
    assert!((10..=14).contains(&ranged["bodies"][0]["stations"].as_array().unwrap().len()));
    assert_eq!(ok(&s, &format!("/stations?jd={JD}")).await["bodies"].as_array().unwrap().len(), 8);

    let v = ok(&s, &format!("/moon-phases?jd={JD}&num=6")).await;
    let phases = v["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 6);
    assert_eq!(f(&phases[0]["jd"]), s.eph.next_moon_phase(JD).unwrap().jd.value());
    assert!(phases.windows(2).all(|w| f(&w[1]["jd"]) - f(&w[0]["jd"]) > 6.0));
    let month = ok(&s, &format!("/moon-phases?jd={JD}&days=30")).await;
    assert!((3..=5).contains(&month["phases"].as_array().unwrap().len()));
    let topo = ok(&s, &format!("/moon-phases?jd={JD}&num=1&ps=topo&loc=56.1,-3.6")).await;
    assert!((f(&topo["phases"][0]["jd"]) - f(&phases[0]["jd"])).abs() > 1e-4);

    // A plain series: two samples a day for three days.
    let v = ok(&s, &format!("/progress?jd={JD}&bodies=su,mo&days=3&pd=2")).await;
    assert_eq!((v["progressed"].as_bool(), v["bodies"].as_array().unwrap().len()), (Some(false), 2));
    let moon = v["bodies"][1]["items"].as_array().unwrap();
    assert_eq!(moon.len(), 6);
    assert!((f(&moon[1]["jd"]) - JD - 0.5).abs() < 1e-9);
    // A secondary progression: age 30 is the sky thirty days after birth.
    let v = ok(&s, &format!("/progress?jd={JD}&bodies=su&years=40")).await;
    let sun = v["bodies"][0]["items"].as_array().unwrap();
    assert_eq!((v["progressed"].as_bool(), sun.len()), (Some(true), 40));
    let expected = s.eph.calc_position(Body::Sun, JD + 30.0, GeoPos::default(), CoordSystem::Ecliptic, Perspective::Geocentric).unwrap();
    assert_eq!((f(&sun[30]["age"]), f(&sun[30]["lng"])), (30.0, expected.lng));
    assert_eq!(&sun[30]["date"]["utc"].as_str().unwrap()[..4], "2026");

    // Aspects agree with the core matcher.
    let v = ok(&s, &format!("/aspects?jd={JD}&bodies=su,mo,me,ve,ma,ju,sa&aspects=0:8,60:6,90:8,120:8,180:8")).await;
    let bodies = parse_body_list("su,mo,me,ve,ma,ju,sa").unwrap();
    let pos = s.eph.calc_positions(&bodies, JD, GeoPos::default(), CoordSystem::Ecliptic, Perspective::Geocentric).unwrap();
    let targets = aspect_orbs(&[(0.0, 8.0), (60.0, 6.0), (90.0, 8.0), (120.0, 8.0), (180.0, 8.0)]);
    let mut expected = Vec::new();
    for (i, a) in pos.iter().enumerate() {
        for b in &pos[i + 1..] {
            expected.extend(match_aspects(a.lng, b.lng, &targets).into_iter().map(|m| (a.body.to_string(), b.body.to_string(), m.target(), m.divergence())));
        }
    }
    let got = v["aspects"].as_array().unwrap();
    assert!(!expected.is_empty());
    assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
    for (g, e) in got.iter().zip(&expected) {
        assert_eq!((g["a"].as_str().unwrap(), g["b"].as_str().unwrap(), f(&g["aspect"])), (e.0.as_str(), e.1.as_str(), e.2));
        assert!((f(&g["divergence"]) - e.3).abs() < 1e-9);
    }
    assert_eq!(ok(&s, &format!("/aspects?jd={JD}&orb=3")).await["targets"][0]["orb"], 3.0);
}

#[tokio::test]
async fn information_and_errors() {
    let Some(s) = state() else { return };
    let v = ok(&s, "/").await;
    assert_eq!(v["name"], "astro-calc-api");
    assert!(v["endpoints"].as_array().unwrap().len() >= 15);
    let v = ok(&s, "/bodies").await;
    assert!(v["bodies"].as_array().unwrap().iter().any(|b| b["key"] == "su" && b["available"] == true));
    assert_eq!(ok(&s, "/stars").await["stars"].as_array().unwrap().len(), 410);

    for (path, status) in [
        ("/positions?bodies=su,xx", StatusCode::BAD_REQUEST),
        ("/positions?dt=not-a-date", StatusCode::BAD_REQUEST),
        ("/positions?jd=abc", StatusCode::BAD_REQUEST),
        ("/positions?cs=galactic", StatusCode::BAD_REQUEST),
        ("/positions?ps=topo", StatusCode::BAD_REQUEST),
        ("/positions?cs=hor", StatusCode::BAD_REQUEST),
        ("/positions?loc=95,0&ps=topo", StatusCode::BAD_REQUEST),
        ("/positions?aya=zz", StatusCode::BAD_REQUEST),
        ("/positions?sid=1", StatusCode::BAD_REQUEST),
        ("/position", StatusCode::BAD_REQUEST),
        ("/angles?jd=2450094.9", StatusCode::BAD_REQUEST),
        ("/houses?loc=10,10&hsys=Q", StatusCode::BAD_REQUEST),
        ("/rise-set?loc=10,10&mode=9", StatusCode::BAD_REQUEST),
        ("/progress?bodies=su", StatusCode::BAD_REQUEST),
        ("/progress?days=1000000&pd=24", StatusCode::BAD_REQUEST),
        ("/aspects?aspects=90:x", StatusCode::BAD_REQUEST),
        ("/positions?dt=1700-01-01", StatusCode::UNPROCESSABLE_ENTITY),
        ("/nowhere", StatusCode::NOT_FOUND),
    ] {
        let (got, body) = get(&s, path).await;
        assert_eq!(got, status, "{path}: {body}");
        assert_eq!(body["valid"], false, "{path}");
        assert!(body["error"].as_str().is_some_and(|e| !e.is_empty()), "{path}");
    }
}
