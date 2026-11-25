//! Endpoint tests against the router, without a network socket.
//! They need a planetary ephemeris file in `../astro-calc-core/data` (any astro-calc-build
//! tier) and are skipped otherwise.

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
            let planetary = scan_dir(&dir).is_ok_and(|files| files.iter().any(|f| f.is_planetary()));
            planetary.then(|| Arc::new(AppState::load(dir).unwrap()))
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

/// Equal but for the last bit or two: serde_json may parse a float one unit off.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 4.0 * f64::EPSILON * a.abs().max(b.abs()).max(1.0)
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
                // The two angles are named after the coordinate system.
                let (first, second) = match cs {
                    CoordSystem::Ecliptic => ("lng", "lat"),
                    CoordSystem::Equatorial => ("ras", "dcl"),
                    CoordSystem::Horizontal => ("azi", "alt"),
                };
                let speed = format!("{first}_speed");
                assert_eq!(got.as_object().unwrap().len(), 7, "{got}");
                for (key, value) in [(first, want.lng), (second, want.lat), (speed.as_str(), want.lng_speed), ("dist", want.dist)] {
                    assert!((f(&got[key]) - value).abs() <= 1e-12 * value.abs().max(1.0), "{} {key}", want.body);
                }
            }
        }
    }
    // Defaults: the default set, ecliptic. With a place the view is topocentric and the set
    // starts with the ascendant; without one it is geocentric and has no ascendant.
    let keys = |v: &Value, field: &str| v[field].as_array().unwrap().iter().map(|b| b["key"].as_str().unwrap().to_string()).collect::<Vec<_>>().join(",");
    let v = ok(&s, &format!("/positions?jd={JD}")).await;
    assert_eq!((v["cs"].as_str(), v["ps"].as_str()), (Some("ecliptic"), Some("geocentric")));
    assert_eq!(keys(&v, "bodies"), "su,mo,me,ve,ma,ju,sa,ur,ne,pl,ke,ra");
    let v = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6")).await;
    assert_eq!(v["ps"], "topocentric");
    assert_eq!(keys(&v, "bodies"), "as,su,mo,me,ve,ma,ju,sa,ur,ne,pl,ke,ra");
    assert!((f(&v["bodies"][0]["lng"]) - ascendant(JD, geo)).abs() < 1e-6);
    assert!(((f(&v["bodies"][11]["lng"]) - f(&v["bodies"][12]["lng"])).abs() - 180.0).abs() < 1e-9, "Ketu opposite Rahu");
    assert_eq!(ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&ps=geo")).await["ps"], "geocentric");
    assert_eq!(ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&topo=0")).await["ps"], "geocentric");
    // The set names, alone or with other keys, adapted to the perspective.
    assert_eq!(keys(&ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=basic")).await, "bodies"), "as,su,mo,me,ve,ma,ju,sa,ke,ra");
    assert_eq!(keys(&ok(&s, &format!("/positions?jd={JD}&bodies=basic,kr")).await, "bodies"), "su,mo,me,ve,ma,ju,sa,ke,ra,kr");
    assert_eq!(keys(&ok(&s, &format!("/positions?jd={JD}&ps=helio&bodies=default")).await, "bodies"), "mo,me,ve,ea,ma,ju,sa,ur,ne,pl");
    // `core`, the former name of the default set, still works.
    assert_eq!(keys(&ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=core")).await, "bodies"), "as,su,mo,me,ve,ma,ju,sa,ur,ne,pl,ke,ra");
    assert_eq!(keys(&ok(&s, &format!("/longitudes?jd={JD}&loc=56.1,-3.6&bodies=default,ch")).await, "longitudes"), "as,su,mo,me,ve,ma,ju,sa,ur,ne,pl,ke,ra,ch");
    assert_eq!(keys(&ok(&s, &format!("/positions?jd={JD}&ps=areo&bodies=basic")).await, "bodies"), "su,mo,me,ve,ea,ju,sa");

    // Longitudes only: the same values as full positions, sidereal when an ayanamsha is named.
    let v = ok(&s, &format!("/longitudes?jd={JD}&loc=56.1,-3.6")).await;
    assert_eq!((v["ps"].as_str(), v["sidereal"].as_bool(), v["ayanamsha"].is_null()), (Some("topocentric"), Some(false), true));
    assert_eq!(keys(&v, "longitudes"), "as,su,mo,me,ve,ma,ju,sa,ur,ne,pl,ke,ra");
    let want = s.eph.calc_longitudes(&Body::DEFAULT_BODIES, JD, geo, Perspective::Topocentric, None).unwrap();
    for (got, want) in v["longitudes"].as_array().unwrap().iter().zip(&want) {
        assert_eq!(got.as_object().unwrap().len(), 2);
        assert!((f(&got["lng"]) - want.lng).abs() < 1e-9, "{}", want.body);
    }
    let sid = ok(&s, &format!("/longitudes?jd={JD}&loc=56.1,-3.6&bodies=basic&aya=lh")).await;
    let aya = s.eph.ayanamsha(JD, Ayanamsha::Lahiri).unwrap();
    assert_eq!((sid["sidereal"].as_bool(), sid["ayanamsha"]["key"].as_str(), f(&sid["ayanamsha"]["value"])), (Some(true), Some("lh"), aya));
    assert_eq!(keys(&sid, "longitudes"), "as,su,mo,me,ve,ma,ju,sa,ke,ra");
    for (i, got) in sid["longitudes"].as_array().unwrap().iter().enumerate() {
        let tropical = f(&v["longitudes"][if i < 8 { i } else { i + 3 }]["lng"]);
        assert!(math::diff_deg(tropical - aya, f(&got["lng"])).abs() < 1e-9, "{}", got["key"]);
    }
    let no_place = ok(&s, &format!("/longitudes?jd={JD}")).await;
    assert_eq!((no_place["ps"].as_str(), keys(&no_place, "longitudes").as_str()), (Some("geocentric"), "su,mo,me,ve,ma,ju,sa,ur,ne,pl,ke,ra"));
    assert_eq!(keys(&ok(&s, &format!("/longitudes?jd={JD}&ps=jovi")).await, "longitudes"), "su,mo,me,ve,ea,ma,sa,ur,ne,pl");
    let one = ok(&s, &format!("/longitude?jd={JD}&loc=56.1,-3.6&body=mo&aya=tc")).await;
    let want = s.eph.calc_longitude(Body::Moon, JD, geo, Perspective::Topocentric, Some(Ayanamsha::TrueCitra)).unwrap();
    assert_eq!((one["key"].as_str(), one["ps"].as_str(), one["ayanamsha"]["key"].as_str()), (Some("mo"), Some("topocentric"), Some("tc")));
    assert!((f(&one["lng"]) - *want).abs() < 1e-9);
    assert!((f(&ok(&s, &format!("/longitude?jd={JD}&loc=56.1,-3.6&body=as")).await["lng"]) - ascendant(JD, geo)).abs() < 1e-6);

    // Legacy aliases still work.
    let v = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&topo=1&eq=1&bodies=mo")).await;
    assert_eq!((v["cs"].as_str(), v["ps"].as_str()), (Some("equatorial"), Some("topocentric")));
    // Every perspective by key and by name; other centres swap the Earth in by default.
    for ps in Perspective::ALL {
        for key in [ps.key(), ps.name()] {
            let v = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&ps={key}")).await;
            assert_eq!(v["ps"], ps.name(), "{key}");
            let keys: Vec<&str> = v["bodies"].as_array().unwrap().iter().map(|b| b["key"].as_str().unwrap()).collect();
            assert_eq!(keys.contains(&"ea"), !ps.is_earth_based(), "{key}: {keys:?}");
            assert!(keys.len() >= 8, "{key}: {keys:?}");
            let own = ps.center().filter(|_| !ps.is_earth_based()).map(|b| b.to_string());
            assert!(own.is_none_or(|k| !keys.contains(&k.as_str())), "{key}: {keys:?}");
        }
    }
    let helio = ok(&s, &format!("/positions?jd={JD}&ps=helio&bodies=ea,ma")).await;
    let expected = s.eph.calc_position(Body::Earth, JD, GeoPos::default(), CoordSystem::Ecliptic, Perspective::Heliocentric).unwrap();
    assert!((f(&helio["bodies"][0]["lng"]) - expected.lng).abs() < 1e-9);
    let local = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&ps=local&bodies=su")).await;
    let expected = s.eph.calc_position(Body::Sun, JD, geo, CoordSystem::Ecliptic, Perspective::LocalSpaceHorizon).unwrap();
    assert_eq!(local["cs"], "horizontal");
    assert!((f(&local["bodies"][0]["azi"]) - expected.lng).abs() < 1e-9 && (f(&local["bodies"][0]["alt"]) - expected.lat).abs() < 1e-9);

    // All three systems in one response, from one calculation.
    let all = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&ps=topo&cs=all&bodies=su,mo")).await;
    assert_eq!(all["cs"], "all");
    let want = s.eph.calc_positions_ext(&[Body::Sun, Body::Moon], JD, geo, Perspective::Topocentric).unwrap();
    for (got, want) in all["bodies"].as_array().unwrap().iter().zip(&want) {
        assert_eq!(got.as_object().unwrap().len(), 15, "{got}");
        for (key, value) in [
            ("lng", want.lng), ("lat", want.lat), ("ras", want.ras), ("dcl", want.dcl), ("azi", want.azi), ("alt", want.alt),
            ("lng_speed", want.lng_speed), ("lat_speed", want.lat_speed), ("ras_speed", want.ras_speed), ("dcl_speed", want.dcl_speed),
            ("azi_speed", want.azi_speed), ("alt_speed", want.alt_speed), ("dist", want.dist), ("dist_speed", want.dist_speed),
        ] {
            assert!((f(&got[key]) - value).abs() <= 1e-12 * value.abs().max(1.0), "{} {key}", want.body);
        }
    }
    // Topocentric results carry the chart angles of the moment and place; others do not.
    for path in ["/positions", "/extended-positions"] {
        let topo = ok(&s, &format!("{path}?jd={JD}&loc=56.1,-3.6&ps=topo&bodies=su,mo")).await;
        let want = angles(JD, geo);
        for (key, value) in [("ascendant", want.ascendant), ("descendant", want.descendant()), ("mc", want.mc), ("ic", want.ic()), ("vertex", want.vertex)] {
            assert!(close(f(&topo["points"][key]), value), "{path} {key}");
        }
        assert!(ok(&s, &format!("{path}?jd={JD}&loc=56.1,-3.6&ps=geo&bodies=su,mo")).await.get("points").is_none());
        // A place alone makes these endpoints topocentric.
        assert_eq!(ok(&s, &format!("{path}?jd={JD}&loc=56.1,-3.6&bodies=su,mo")).await["points"], topo["points"]);
    }
    for path in ["/position", "/extended-position"] {
        let one = ok(&s, &format!("{path}?jd={JD}&loc=56.1,-3.6&ps=topo&body=mo")).await;
        assert!(close(f(&one["points"]["ascendant"]), ascendant(JD, geo)) && one["points"]["mc"].is_number());
    }
    assert!(ok(&s, &format!("/positions?jd={JD}&bodies=su,mo")).await.get("points").is_none());
    assert!(ok(&s, &format!("/extended-positions?jd={JD}&ps=helio&bodies=ea,ma")).await.get("points").is_none());

    // The dedicated endpoints give the same combined result, whatever `cs` says.
    let q = format!("jd={JD}&loc=56.1,-3.6&ps=topo&bodies=su,mo");
    assert_eq!(ok(&s, &format!("/extended-positions?{q}")).await["bodies"], all["bodies"]);
    assert_eq!(ok(&s, &format!("/extended-positions?{q}&cs=equ")).await["bodies"], all["bodies"]);
    let single = ok(&s, &format!("/extended-position?jd={JD}&loc=56.1,-3.6&ps=topo&body=mo")).await;
    assert_eq!((&single["body"], &single["cs"]), (&all["bodies"][1], &all["cs"]));
    assert_eq!(single["body"].as_object().unwrap().len(), 15);
    let defaults = ok(&s, &format!("/extended-positions?jd={JD}&loc=56.1,-3.6")).await;
    assert_eq!((defaults["ps"].as_str(), defaults["bodies"].as_array().unwrap().len()), (Some("topocentric"), Body::DEFAULT_BODIES.len()));

    // Away from the Earth there is no horizon: ecliptic and equatorial only, no place needed.
    let helio = ok(&s, &format!("/extended-positions?jd={JD}&ps=helio&bodies=ea,ma")).await;
    let ecl = ok(&s, &format!("/positions?jd={JD}&ps=helio&bodies=ea,ma")).await;
    let equ = ok(&s, &format!("/positions?jd={JD}&ps=helio&bodies=ea,ma&cs=equ")).await;
    assert_eq!((helio["cs"].as_str(), helio["ps"].as_str()), (Some("all"), Some("heliocentric")));
    for (i, body) in helio["bodies"].as_array().unwrap().iter().enumerate() {
        assert_eq!(body.as_object().unwrap().len(), 15);
        for key in ["lng", "lat", "lng_speed", "lat_speed", "dist", "dist_speed"] {
            assert_eq!(body[key], ecl["bodies"][i][key], "{key}");
        }
        for key in ["ras", "dcl", "ras_speed", "dcl_speed"] {
            assert_eq!(body[key], equ["bodies"][i][key], "{key}");
        }
        for key in ["azi", "alt", "azi_speed", "alt_speed"] {
            assert!(body[key].is_null(), "{key}");
        }
    }
    let jovi = ok(&s, &format!("/extended-position?jd={JD}&ps=jovi&body=sa")).await;
    assert!(jovi["body"]["ras"].is_number() && jovi["body"]["azi"].is_null());

    let one = ok(&s, &format!("/position?jd={JD}&loc=56.1,-3.6&cs=all&body=ve&aya=lh&sid=1")).await;
    let venus = s.eph.calc_position_ext(Body::Venus, JD, geo, Perspective::Topocentric).unwrap();
    assert!((f(&one["body"]["ras"]) - venus.ras).abs() < 1e-9);
    assert!((venus.lng - f(&one["ayanamshas"][0]["value"]) - f(&one["body"]["lng"])).rem_euclid(360.0) < 1e-9);
    assert!(!ok(&s, &format!("/aspects?jd={JD}&ps=helio")).await["aspects"].as_array().unwrap().is_empty());

    // One body, and sidereal longitudes.
    let one = ok(&s, &format!("/position?jd={JD}&body=ve")).await;
    let sid = ok(&s, &format!("/position?jd={JD}&body=ve&aya=lh&sid=1")).await;
    let aya = f(&sid["ayanamshas"][0]["value"]);
    assert!((f(&one["body"]["lng"]) - aya - f(&sid["body"]["lng"])).rem_euclid(360.0) < 1e-9);
    assert_eq!(sid["sidereal"], true);

    // On/off parameters: 1, true and on are on; 0, false and off are off; anything else is ignored.
    for (value, on) in [("1", true), ("true", true), ("TRUE", true), ("0", false), ("false", false), ("FALSE", false), ("on", true), ("Off", false), ("xyz", false), ("2", false), ("", false)] {
        let v = ok(&s, &format!("/position?jd={JD}&body=ve&aya=lh&sid={value}")).await;
        assert_eq!(v["sidereal"], on, "sid={value}");
    }
    // An unreadable value leaves the default in place: topocentric when a place is given.
    for (value, ps) in [("0", "geocentric"), ("false", "geocentric"), ("true", "topocentric"), ("xyz", "topocentric")] {
        assert_eq!(ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&topo={value}")).await["ps"], ps, "topo={value}");
    }
    assert_eq!(ok(&s, &format!("/rise-set?jd={JD}&loc=10,10&dip=2")).await["options"]["dip"], false);

    // Coordinate systems by name or number, under `cs` or its older name `eq`.
    for (name, keys) in [("ecliptic", ["0", "ecl", "ecliptic"]), ("equatorial", ["1", "equ", "Equatorial"]), ("horizontal", ["2", "hor", "horizontal"])] {
        let want = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=mo&cs={name}")).await;
        for key in keys {
            for param in ["cs", "eq"] {
                let got = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=mo&{param}={key}")).await;
                assert_eq!(got["bodies"], want["bodies"], "{param}={key}");
            }
        }
    }
    let both = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=mo&cs=hor&eq=1")).await;
    assert!(both["bodies"][0]["azi"].is_number(), "cs takes precedence over eq");
}

#[tokio::test]
async fn combined_perspectives_nest_one_block_for_each() {
    let Some(s) = state() else { return };
    let keys = |v: &Value, field: &str| v[field].as_array().unwrap().iter().map(|b| b["key"].as_str().unwrap().to_string()).collect::<Vec<_>>().join(",");
    let base = format!("jd={JD}&loc=56.1,-3.6&bodies=su,mo,ma&aya=lh&sid=1");
    // `tg` and its older name `both`: every body from the place and from the Earth's centre.
    for (combined, order) in [("both", ["key", "geo", "topo"]), ("tg", ["key", "topo", "geo"])] {
        let pair = ok(&s, &format!("/extended-positions?{base}&ps={combined}")).await;
        assert_eq!((pair["ps"].as_str(), pair["cs"].as_str()), (Some(combined), Some("all")));
        assert!(pair["points"]["ascendant"].is_number());
        for ps in ["geo", "topo"] {
            let single = ok(&s, &format!("/extended-positions?{base}&ps={ps}")).await;
            for (nested, one) in pair["bodies"].as_array().unwrap().iter().zip(single["bodies"].as_array().unwrap()) {
                assert_eq!(nested.as_object().unwrap().keys().collect::<Vec<_>>(), order);
                assert_eq!(nested["key"], one["key"]);
                let mut want = one.clone();
                want.as_object_mut().unwrap().shift_remove("key");
                assert_eq!(nested[ps], want, "{combined} {ps}");
            }
        }
    }
    // One coordinate system, and one body.
    let ecl = ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=mo&ps=tg")).await;
    let moon = &ecl["bodies"][0];
    assert!((f(&moon["geo"]["lng"]) - f(&moon["topo"]["lng"])).abs() > 0.05);
    assert!(moon["geo"]["ras"].is_null());
    let one = ok(&s, &format!("/extended-position?jd={JD}&loc=56.1,-3.6&body=mo&ps=both")).await;
    assert!(one["body"]["topo"]["alt"].is_number());

    // `htg` adds the view from the Sun. A body that cannot be seen from a perspective has
    // no block for it: the Sun from the Sun, the Earth and the lunar nodes from the Earth or Sun.
    let all = ok(&s, &format!("/extended-positions?jd={JD}&loc=56.1,-3.6&bodies=su,ea,ma,ra&ps=htg")).await;
    assert_eq!((all["ps"].as_str(), keys(&all, "bodies").as_str()), (Some("htg"), "su,ea,ma,ra"));
    let helio = ok(&s, &format!("/extended-positions?jd={JD}&bodies=ea,ma&ps=helio")).await;
    let (sun, earth, mars, rahu) = (&all["bodies"][0], &all["bodies"][1], &all["bodies"][2], &all["bodies"][3]);
    assert_eq!(mars.as_object().unwrap().keys().collect::<Vec<_>>(), ["key", "helio", "topo", "geo"]);
    assert!(sun.get("helio").is_none() && sun["geo"]["lng"].is_number() && sun["topo"]["azi"].is_number());
    assert_eq!(earth.as_object().unwrap().keys().collect::<Vec<_>>(), ["key", "helio"]);
    assert!(rahu.get("helio").is_none() && rahu["geo"]["lng"].is_number());
    assert_eq!((f(&earth["helio"]["lng"]), f(&mars["helio"]["ras"])), (f(&helio["bodies"][0]["lng"]), f(&helio["bodies"][1]["ras"])));
    assert!(mars["helio"]["azi"].is_null() && mars["topo"]["azi"].is_number());
    // The Sun from the Earth is opposite the Earth from the Sun.
    assert!(((f(&sun["geo"]["lng"]) - f(&earth["helio"]["lng"])).rem_euclid(360.0) - 180.0).abs() < 0.01);
    // `hg` needs no place; a set gains the Earth for the view from the Sun.
    let hg = ok(&s, &format!("/positions?jd={JD}&ps=hg&bodies=basic")).await;
    assert_eq!((hg["ps"].as_str(), keys(&hg, "bodies").as_str()), (Some("hg"), "su,mo,me,ve,ma,ju,sa,ke,ra,ea"));
    assert!(hg.get("points").is_none() && hg["bodies"][9]["helio"]["lng"].is_number() && hg["bodies"][9].get("geo").is_none());

    // `/progress` nests the same blocks in every item, beside its Julian Day.
    let series = ok(&s, &format!("/progress?jd={JD}&loc=56.1,-3.6&bodies=su,ea,ma&days=3&ps=htg")).await;
    assert_eq!((series["ps"].as_str(), keys(&series, "bodies").as_str()), (Some("htg"), "su,ea,ma"));
    let single = ok(&s, &format!("/progress?jd={JD}&bodies=ma&days=3&ps=helio")).await;
    let (sun, mars) = (series["bodies"][0]["items"].as_array().unwrap(), series["bodies"][2]["items"].as_array().unwrap());
    assert_eq!((sun.len(), mars.len()), (3, 3));
    assert_eq!(mars[1].as_object().unwrap().keys().collect::<Vec<_>>(), ["jd", "helio", "topo", "geo"]);
    assert!((f(&mars[1]["jd"]) - JD - 1.0).abs() < 1e-9 && sun[1].get("helio").is_none() && sun[1]["geo"]["lng"].is_number());
    let mut want = single["bodies"][0]["items"][1].clone();
    want.as_object_mut().unwrap().shift_remove("jd");
    assert_eq!(mars[1]["helio"], want);
    assert!(mars[1]["helio"].get("jd").is_none() && (f(&mars[1]["geo"]["lng"]) - f(&mars[1]["topo"]["lng"])).abs() < 0.01);

    // The short keys, and the older ones, name the same perspectives.
    for (key, name) in [("local", "local-space-horizon"), ("geoc", "geocentric"), ("geo", "geocentric"), ("bary", "barycentric")] {
        assert_eq!(ok(&s, &format!("/positions?jd={JD}&loc=56.1,-3.6&bodies=ma&ps={key}")).await["ps"], name, "{key}");
    }
    // A topocentric block needs a place, a horizon needs the Earth, and only the position
    // endpoints and /progress take several perspectives.
    for path in [
        format!("/extended-positions?jd={JD}&ps=both"),
        format!("/progress?jd={JD}&days=2&ps=htg"),
        format!("/positions?jd={JD}&loc=56.1,-3.6&cs=hor&ps=hg"),
        "/moon-phases?ps=tg".into(),
        "/longitudes?ps=hg&loc=56.1,-3.6".into(),
    ] {
        assert_eq!(get(&s, &path).await.0, StatusCode::BAD_REQUEST, "{path}");
    }
}

#[tokio::test]
async fn chart_endpoints() {
    let Some(s) = state() else { return };
    let geo = GeoPos::new(56.1, -3.6, 0.0).unwrap();
    let q = format!("jd={JD}&loc=56.1,-3.6");
    let a = angles(JD, geo);
    let v = ok(&s, &format!("/ascendant?{q}")).await;
    assert!(close(f(&v["ascendant"]), a.ascendant) && close(f(&v["points"]["ascendant"]), a.ascendant));
    assert!(close(f(&v["points"]["mc"]), a.mc) && close(f(&v["points"]["descendant"]), a.descendant()) && close(f(&v["points"]["vertex"]), a.vertex));
    let v = ok(&s, &format!("/angles?{q}")).await;
    assert_eq!((f(&v["angles"]["mc"]), f(&v["angles"]["vertex"])), (a.mc, a.vertex));
    // Hourly by default, from one day before the moment to eight days after it.
    let v = ok(&s, &format!("/ascendant-progress?{q}")).await;
    let list = v["ascendants"].as_array().unwrap();
    assert_eq!(list.len(), 9 * 24 + 1);
    assert!(close(f(&list[0]["jd"]), JD - 1.0) && close(f(&list[216]["jd"]), JD + 8.0));
    assert!(close(f(&list[24]["jd"]), JD) && close(f(&list[24]["lng"]), a.ascendant) && close(f(&v["points"]["mc"]), a.mc));
    let v = ok(&s, &format!("/ascendant-progress?{q}&days=2&pdays=0&pd=4")).await;
    let list = v["ascendants"].as_array().unwrap();
    assert_eq!(list.len(), 9);
    assert!(close(f(&list[3]["lng"]), ascendant(JD + 0.75, geo)));
    for bad in ["pd=0", "days=-1", "days=1000000&pd=1000"] {
        assert_eq!(get(&s, &format!("/ascendant-progress?{q}&{bad}")).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    let v = ok(&s, &format!("/houses?{q}&hsys=P,U,w")).await;
    let sets = v["sets"].as_array().unwrap();
    assert_eq!(sets.iter().map(|x| x["system"].as_str().unwrap()).collect::<Vec<_>>(), ["P", "U", "W"]);
    assert!(close(f(&sets[0]["cusps"][1]), houses(JD, geo, HouseSystem::Placidus).cusps[1]));
    // `all` is every twelve-house system and then the Gauquelin sectors.
    let all = ok(&s, &format!("/houses?{q}&hsys=all")).await;
    let all = all["sets"].as_array().unwrap();
    assert_eq!(all.len(), HouseSystem::ALL.len() + 1);
    assert_eq!(all.last().unwrap()["system"], "G");
    // The 36 sectors run clockwise from the ascendant; every third cusp is a Placidus cusp.
    let v = ok(&s, &format!("/houses?{q}&hsys=g,P")).await;
    let (sectors, placidus) = (v["sets"][0]["cusps"].as_array().unwrap(), v["sets"][1]["cusps"].as_array().unwrap());
    assert_eq!((v["sets"][0]["system"].as_str(), v["sets"][0]["name"].as_str()), (Some("G"), Some("Gauquelin Sectors")));
    assert_eq!((sectors.len(), placidus.len()), (36, 12));
    let core = gauquelin_sectors(JD, geo);
    for i in 0..36 {
        assert!(close(f(&sectors[i]), core.cusps[i]), "sector cusp {i}");
    }
    assert!(close(f(&sectors[0]), f(&v["angles"]["ascendant"])) && close(f(&sectors[9]), f(&v["angles"]["mc"])));
    assert!(close(f(&sectors[3]), f(&placidus[11])) && close(f(&sectors[33]), f(&placidus[1])));
    assert_eq!(ok(&s, &format!("/houses?{q}")).await["sets"][0]["system"], "W");
    // An ayanamsha moves the cusps back, and whole-sign houses to the sidereal sign of the ascendant.
    let tropical = ok(&s, &format!("/houses?{q}&hsys=P,W")).await;
    assert_eq!(tropical["sidereal"], false);
    assert!(tropical["ayanamsha"].is_null());
    let sidereal = ok(&s, &format!("/houses?{q}&hsys=P,W&aya=lh")).await;
    assert_eq!(sidereal["sidereal"], true);
    assert_eq!(sidereal["ayanamsha"]["key"], "lh");
    let offset = f(&sidereal["ayanamsha"]["value"]);
    assert!(close(f(&sidereal["angles"]["ascendant"]), f(&tropical["angles"]["ascendant"])));
    for i in 0..12 {
        let moved = (f(&tropical["sets"][0]["cusps"][i]) - offset).rem_euclid(360.0);
        assert!(close(f(&sidereal["sets"][0]["cusps"][i]), moved), "cusp {i}");
    }
    let first = ((f(&tropical["angles"]["ascendant"]) - offset).rem_euclid(360.0) / 30.0).floor() * 30.0;
    assert!(close(f(&sidereal["sets"][1]["cusps"][0]), first));
    assert!(close(f(&sidereal["sets"][1]["cusps"][3]), (first + 90.0).rem_euclid(360.0)));
    // The sectors move back with the rest.
    let (g0, g1) = (ok(&s, &format!("/houses?{q}&hsys=G")).await, ok(&s, &format!("/houses?{q}&hsys=G&aya=lh")).await);
    for i in [0, 9, 20, 35] {
        assert!(close(f(&g1["sets"][0]["cusps"][i]), (f(&g0["sets"][0]["cusps"][i]) - offset).rem_euclid(360.0)), "sector cusp {i}");
    }
    assert_eq!(get(&s, &format!("/houses?{q}&aya=nope")).await.0, StatusCode::BAD_REQUEST);

    let v = ok(&s, &format!("/ayanamsha?jd={JD}&aya=lh,tc")).await;
    assert!(close(f(&v["ayanamshas"][0]["value"]), s.eph.ayanamsha(JD, Ayanamsha::Lahiri).unwrap()));
    assert_eq!(v["ayanamshas"][1]["key"], "tc");
    assert!(ok(&s, &format!("/ayanamsha?jd={JD}")).await["ayanamshas"].as_array().unwrap().len() >= 17);

    let v = ok(&s, &format!("/rise-set?{q}&bodies=su,mo,hip:65474")).await;
    let sun = s.eph.day_transits(Body::Sun, JD, geo, RiseSetOptions::default()).unwrap();
    assert!(close(f(&v["bodies"][0]["rise"]["jd"]), sun.rise.unwrap().value()));
    assert_eq!(v["bodies"].as_array().unwrap().len(), 3);
    let upper = ok(&s, &format!("/rise-set?{q}&bodies=su&mode=0")).await;
    assert!(f(&upper["bodies"][0]["rise"]["jd"]) != sun.rise.unwrap().value());

    // Explicit rise/set options, on their own or on top of a mode.
    let geo_alt = GeoPos::new(56.1, -3.6, 100.0).unwrap();
    for (extra, opts, place) in [
        ("disc=upper", RiseSetOptions::upper_limb(), geo),
        ("disc=upper&refraction=0", RiseSetOptions::upper_limb().with_refraction(Refraction::Off), geo),
        ("mode=0&disc=lower&fixed=1", RiseSetOptions::default().with_disc(DiscAlignment::LowerLimb).with_fixed_disc(true), geo),
        ("twilight=civil", RiseSetOptions::twilight(Horizon::CivilTwilight), geo),
        ("twilight=nautical", RiseSetOptions::twilight(Horizon::NauticalTwilight), geo),
        ("twilight=astro", RiseSetOptions::twilight(Horizon::AstronomicalTwilight), geo),
        ("horizon=1.5", RiseSetOptions::default().with_horizon(Horizon::Altitude(1.5)), geo),
        ("pressure=900&temp=25", RiseSetOptions::default().with_refraction(Refraction::Atmosphere { pressure: 900.0, temperature: 25.0 }), geo),
        ("alt=100&dip=1", RiseSetOptions::default().with_dip(true), geo_alt),
        ("geoc=1&nolat=1", RiseSetOptions::default().with_geocentric(true).with_ignore_latitude(true), geo),
        ("hindu=1", RiseSetOptions::hindu_rising(), geo),
    ] {
        let expected = s.eph.day_transits(Body::Sun, JD, place, opts).unwrap().rise.unwrap().value();
        let v = ok(&s, &format!("/rise-set?{q}&bodies=su&{extra}")).await;
        assert!(close(f(&v["bodies"][0]["rise"]["jd"]), expected), "{extra}");
    }
    let v = ok(&s, &format!("/rise-set?{q}&bodies=su&twilight=civil&disc=upper")).await;
    assert_eq!((v["options"]["twilight"].as_str(), f(&v["options"]["horizon"]), v["options"]["disc"].as_str()), (Some("civil"), -6.0, Some("upper")));
    assert_eq!(ok(&s, &format!("/indian-day?{q}&hindu=1")).await["options"]["ignore_latitude"], true);

    let v = ok(&s, &format!("/indian-day?{q}")).await;
    let day = s.eph.indian_day(JD, geo, RiseSetOptions::default()).unwrap();
    assert_eq!((f(&v["rise"]["jd"]), v["ghati"].as_u64(), v["display"].as_str()), (day.rise_jd, Some(day.ghati() as u64), Some(day.to_ghati_string().as_str())));

    // Day by day: by default the day before the date and the 8 after it start.
    let v = ok(&s, &format!("/rise-sets?{q}")).await;
    assert_eq!((f(&v["days"]), f(&v["pdays"])), (8.0, 1.0));
    let keys: Vec<_> = v["bodies"].as_array().unwrap().iter().map(|b| b["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["su", "mo", "me", "ve", "ma", "ju", "sa"]);
    for (i, day) in v["bodies"][1]["days"].as_array().unwrap().iter().enumerate() {
        let d = s.eph.day_transits(Body::Moon, JD + (i as f64 - 1.0), geo, RiseSetOptions::default()).unwrap();
        assert!(close(f(&day["day_start"]), d.day_start.value()));
        assert_eq!(day["rise"].as_f64(), d.rise.map(JdUt::value));
        assert_eq!(day["ic"].as_f64(), d.ic.map(JdUt::value));
        // serde_json may parse a float one unit off in its last bit.
        assert!((f(&day["alt_min"]) - d.min_alt.unwrap()).abs() < 1e-12 && (f(&day["alt_max"]) - d.max_alt.unwrap()).abs() < 1e-12);
    }
    assert_eq!(v["bodies"][1]["days"].as_array().unwrap().len(), 9);
    // Explicit span and options, applied to every day.
    let v = ok(&s, &format!("/rise-sets?{q}&bodies=su&days=3&pdays=0&disc=upper")).await;
    let days = v["bodies"][0]["days"].as_array().unwrap();
    assert_eq!(days.len(), 3);
    let last = s.eph.day_transits(Body::Sun, JD + 2.0, geo, RiseSetOptions::upper_limb()).unwrap();
    assert!(close(f(&days[2]["set"]), last.set.unwrap().value()));
    assert_eq!(f(&days[1]["day_start"]) - f(&days[0]["day_start"]), 1.0);
    // Near the pole the Sun neither rises nor sets in midwinter: the times are null.
    let v = ok(&s, &format!("/rise-sets?jd={JD}&loc=78.2,15.6&bodies=su&days=1&pdays=0")).await;
    let day = &v["bodies"][0]["days"][0];
    assert!(day["rise"].is_null() && day["set"].is_null() && day["mc"].is_number(), "{day}");

    // Transposed: a body's place at a historic moment, carried onto the sky of the date.
    let source = JD - 12_345.25;
    let h_geo = GeoPos::new(13.1, 80.3, 0.0).unwrap();
    let v = ok(&s, &format!("/transposed-transits?{q}&h_jd={source}&bodies=su,mo&days=2&refraction=off")).await;
    assert_eq!((f(&v["days"]), f(&v["pdays"]), f(&v["horizon_alt"]), v["ps"].as_str()), (2.0, 0.0, 0.0, Some("geocentric")));
    assert_eq!((f(&v["date"]["jd"]), f(&v["historic_date"]["jd"]), f(&v["geo"]["lat"])), (JD, source, geo.lat));
    // Geocentric: no historic place, and no chart angles.
    assert!(v["historic_geo"].is_null() && v.get("points").is_none() && v.get("historic_points").is_none());
    let natal = s.eph.calc_positions(&[Body::Sun, Body::Moon], source, GeoPos::default(), CoordSystem::Ecliptic, Perspective::Geocentric).unwrap();
    let start = local_day_start(JD, geo.lng);
    for (item, pos) in v["bodies"].as_array().unwrap().iter().zip(&natal) {
        assert_eq!((item["key"].as_str(), item["source"].as_str()), (Some(format!("tp:{}", pos.body).as_str()), Some(pos.body.to_string().as_str())));
        assert!(close(f(&item["lng"]), pos.lng) && close(f(&item["lat"]), pos.lat));
        let want = transposed_point_transit_days(pos, start, geo, 0.0, 2);
        for (day, d) in item["days"].as_array().unwrap().iter().zip(&want) {
            assert!(close(f(&day["day_start"]), d.start.value()) && close(f(&day["mc"]), d.mc.value()) && close(f(&day["ic"]), d.ic.value()));
            assert!(close(f(&day["rise"]), d.rise.unwrap().value()) && close(f(&day["set"]), d.set.unwrap().value()));
        }
    }
    // On the ecliptic and without refraction, the point rises as the ascendant.
    let v = ok(&s, &format!("/transposed-transits?{q}&h_jd={source}&bodies=mo&refraction=off&nolat=1")).await;
    let rise = f(&v["bodies"][0]["days"][0]["rise"]);
    assert!((ascendant(rise, geo) - natal[1].lng).abs() < 1e-6 && f(&v["bodies"][0]["lat"]) == 0.0);
    // A historic place makes it topocentric and adds the chart angles of both moments.
    let v = ok(&s, &format!("/transposed-transits?{q}&h_jd={source}&h_loc=13.1,80.3&bodies=mo")).await;
    assert_eq!((v["ps"].as_str(), f(&v["historic_geo"]["lng"])), (Some("topocentric"), 80.3));
    assert!(f(&v["horizon_alt"]) < -0.5 && (f(&v["bodies"][0]["lng"]) - natal[1].lng).abs() > 0.01);
    assert!(close(f(&v["points"]["ascendant"]), ascendant(JD, geo)) && close(f(&v["historic_points"]["ascendant"]), ascendant(source, h_geo)));
    assert!(v["historic_points"]["mc"].is_number() && v["points"]["vertex"].is_number());
    // The parts of the historic place can be given one by one, and ps=geo sets it aside.
    let parts = ok(&s, &format!("/transposed-transits?{q}&h_jd={source}&h_lat=13.1&h_lng=80.3&bodies=mo")).await;
    assert_eq!(parts["bodies"], v["bodies"]);
    let set_aside = ok(&s, &format!("/transposed-transits?{q}&h_jd={source}&h_loc=13.1,80.3&bodies=mo&ps=geo")).await;
    assert!(close(f(&set_aside["bodies"][0]["lng"]), natal[1].lng) && set_aside.get("points").is_none());

    // Positions: every coordinate system at once, as the core gives them.
    let v = ok(&s, &format!("/transposed-positions?{q}&h_jd={source}&h_loc=13.1,80.3&bodies=su,mo,ma")).await;
    let bodies = [Body::Sun, Body::Moon, Body::Mars];
    let want = s.eph.calc_transposed_positions(&bodies, JD, source, geo, h_geo, Perspective::Topocentric).unwrap();
    assert_eq!((v["ps"].as_str(), f(&v["historic_date"]["jd"]), f(&v["historic_geo"]["lat"])), (Some("topocentric"), source, 13.1));
    assert!(close(f(&v["points"]["ascendant"]), ascendant(JD, geo)) && close(f(&v["historic_points"]["ascendant"]), ascendant(source, h_geo)));
    for ((item, p), body) in v["bodies"].as_array().unwrap().iter().zip(&want).zip(bodies) {
        assert_eq!((item["key"].as_str(), item["source"].as_str()), (Some(format!("tp:{body}").as_str()), Some(body.to_string().as_str())));
        for (key, value) in [("lng", p.lng), ("lat", p.lat), ("ras", p.ras), ("dcl", p.dcl), ("azi", p.azi), ("alt", p.alt), ("azi_speed", p.azi_speed), ("alt_speed", p.alt_speed), ("dist", p.dist)] {
            assert!(close(f(&item[key]), value), "{body} {key}");
        }
        assert_eq!((f(&item["lng_speed"]), f(&item["ras_speed"]), f(&item["dist_speed"])), (0.0, 0.0, 0.0));
    }
    // The default bodies, seen from the centre of the Earth when no historic place is given.
    let v = ok(&s, &format!("/transposed-positions?{q}&h_dt=1962-03-28T05:25:00")).await;
    let keys: Vec<_> = v["bodies"].as_array().unwrap().iter().map(|b| b["source"].as_str().unwrap()).collect();
    assert_eq!(keys, ["su", "mo", "me", "ve", "ma", "ju", "sa"]);
    assert!(v["ps"].as_str() == Some("geocentric") && v["historic_geo"].is_null() && v.get("points").is_none());
    assert!(v["historic_date"]["utc"].as_str().unwrap().starts_with("1962-03-28T05:25:00"));

    let v = ok(&s, &format!("/pheno?jd={JD}&bodies=ve,ce")).await;
    assert!(close(f(&v["bodies"][0]["magnitude"]), s.eph.phenomena(Body::Venus, JD).unwrap().magnitude));
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
    assert!(close(f(&phases[0]["jd"]), s.eph.next_moon_phase(JD).unwrap().jd.value()));
    assert!(phases.windows(2).all(|w| f(&w[1]["jd"]) - f(&w[0]["jd"]) > 6.0));
    let month = ok(&s, &format!("/moon-phases?jd={JD}&days=30")).await;
    assert!((3..=5).contains(&month["phases"].as_array().unwrap().len()));
    let topo = ok(&s, &format!("/moon-phases?jd={JD}&num=1&ps=topo&loc=56.1,-3.6")).await;
    assert!((f(&topo["phases"][0]["jd"]) - f(&phases[0]["jd"])).abs() > 1e-4);

    // Tithis: the list opens with the one in force and runs for a lunar month by default.
    let v = ok(&s, &format!("/tithis?jd={JD}")).await;
    let tithis = v["tithis"].as_array().unwrap();
    assert_eq!((tithis.len(), v["ps"].as_str()), (30, Some("geocentric")));
    assert_eq!(tithis[0]["num"], v["tithi"]["num"]);
    assert!(f(&tithis[0]["jd"]) <= JD && f(&tithis[1]["jd"]) > JD);
    assert!(close(f(&tithis[1]["jd"]), s.eph.next_tithi(JD).unwrap().jd.value()));
    for pair in tithis.windows(2) {
        assert_eq!(pair[1]["num"].as_u64().unwrap(), pair[0]["num"].as_u64().unwrap() % 30 + 1);
    }
    // The 1st and the 16th begin at the new and the full moon.
    let phases = ok(&s, &format!("/moon-phases?jd={JD}&days=31")).await;
    for phase in phases["phases"].as_array().unwrap() {
        let same = tithis.iter().find(|t| t["phase"] == phase["phase"]);
        match phase["phase"].as_str() {
            Some("New" | "Full") => assert!((f(&same.unwrap()["jd"]) - f(&phase["jd"])).abs() < 1e-6, "{phase}"),
            _ => assert!(same.is_none()),
        }
    }
    let new = tithis.iter().find(|t| t["num"] == 1).unwrap();
    assert_eq!((new["paksha"].as_str(), new["name"].as_str(), new["day"].as_u64()), (Some("Shukla"), Some("Pratipada"), Some(1)));
    let week = ok(&s, &format!("/tithis?jd={JD}&days=7")).await;
    assert!((7..=9).contains(&week["tithis"].as_array().unwrap().len()));
    assert_eq!(ok(&s, &format!("/tithis?jd={JD}&num=3")).await["tithis"].as_array().unwrap().len(), 3);
    // Karanas: twice as many, the quarters among their boundaries.
    let k = ok(&s, &format!("/tithis?jd={JD}&karanas=1&num=60")).await;
    let karanas = k["karanas"].as_array().unwrap();
    assert_eq!((karanas.len(), k["tithis"].is_null()), (60, true));
    assert_eq!(karanas[0]["num"], k["karana"]["num"]);
    assert!(karanas.iter().any(|c| c["phase"] == "FirstQuarter") && karanas.iter().any(|c| c["phase"] == "LastQuarter"));
    assert!(karanas.iter().all(|c| c["tithi"].as_u64() == Some(c["num"].as_u64().unwrap().div_ceil(2))));

    // Nakshatras: the Moon by default, True Citra unless another ayanamsha is named.
    let v = ok(&s, &format!("/nakshatra?jd={JD}")).await;
    assert_eq!((v["ayanamsha"]["key"].as_str(), v["ps"].as_str(), v["bodies"].as_array().unwrap().len()), (Some("tc"), Some("geocentric"), 1));
    let moon = &v["bodies"][0];
    let want = s.eph.calc_longitude(Body::Moon, JD, GeoPos::default(), Perspective::Geocentric, Some(Ayanamsha::TrueCitra)).unwrap().value;
    assert!(close(f(&moon["lng"]), want));
    let span = 360.0 / 27.0;
    assert_eq!((moon["key"].as_str(), moon["num"].as_u64()), (Some("mo"), Some((want / span) as u64 + 1)));
    assert!((f(&moon["degrees"]) - want % span).abs() < 1e-9 && (f(&moon["fraction"]) - (want % span) / span).abs() < 1e-9);
    assert_eq!(moon["pada"].as_u64(), Some((want * 0.3) as u64 % 4 + 1));
    assert!(moon["name"].is_string() && moon["lord"].is_string());
    // The Moon's stay: about a day, round the date, in the same nakshatra from end to end.
    let (start, end) = (f(&moon["start_jd"]), f(&moon["end_jd"]));
    assert!(start <= JD && JD < end && (0.8..1.2).contains(&(end - start)), "{start} {end}");
    let inside = ok(&s, &format!("/nakshatra?jd={}", start + 0.001)).await;
    let after = ok(&s, &format!("/nakshatra?jd={}", end + 0.001)).await;
    assert_eq!(inside["bodies"][0]["num"], moon["num"]);
    assert_eq!(after["bodies"][0]["num"].as_u64(), Some(moon["num"].as_u64().unwrap() % 27 + 1));
    assert!(f(&inside["bodies"][0]["fraction"]) < 0.01 && (f(&after["bodies"][0]["start_jd"]) - end).abs() < 1e-6);
    let lahiri = ok(&s, &format!("/nakshatra?jd={JD}&aya=lh&bodies=su,mo,as&loc=56.1,-3.6")).await;
    assert_eq!((lahiri["ayanamsha"]["key"].as_str(), lahiri["bodies"].as_array().unwrap().len()), (Some("lh"), 3));
    assert!((f(&lahiri["bodies"][1]["lng"]) - want).abs() < 0.1 && !close(f(&lahiri["bodies"][1]["lng"]), want));
    // The listing opens with the nakshatra in force and its true start, then counts on.
    let listing = ok(&s, &format!("/nakshatras?jd={JD}")).await;
    assert_eq!((listing["ayanamsha"]["key"].as_str(), listing["bodies"][0]["key"].as_str()), (Some("tc"), Some("mo")));
    assert!(listing["date"]["utc"].is_string() && close(f(&listing["start_date"]["jd"]), JD) && listing["end_date"].is_null());
    let items = listing["bodies"][0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 27);
    assert_eq!((&items[0]["num"], &items[0]["name"], &items[0]["lord"]), (&moon["num"], &moon["name"], &moon["lord"]));
    assert!((f(&items[0]["start_jd"]) - start).abs() < 1e-6 && (f(&items[1]["start_jd"]) - end).abs() < 1e-6);
    for pair in items.windows(2) {
        assert_eq!(pair[1]["num"].as_u64(), Some(pair[0]["num"].as_u64().unwrap() % 27 + 1));
        assert!((0.8..1.2).contains(&(f(&pair[1]["start_jd"]) - f(&pair[0]["start_jd"]))) && pair[1]["direct"] == true);
    }
    assert_eq!(items.iter().map(|i| i["num"].as_u64().unwrap()).collect::<std::collections::BTreeSet<_>>().len(), 27);
    assert_eq!(items[0].as_object().unwrap().keys().collect::<Vec<_>>(), items[5].as_object().unwrap().keys().collect::<Vec<_>>());
    // By count, or up to an end date; several bodies each get a list.
    let few = ok(&s, &format!("/nakshatras?jd={JD}&num=3&bodies=mo,su")).await;
    assert_eq!((few["bodies"][0]["items"].as_array().unwrap().len(), few["bodies"][1]["items"].as_array().unwrap().len()), (3, 3));
    assert_eq!(few["bodies"][0]["items"][2], items[2]);
    let span_of = |v: &Value, i: usize| f(&v["bodies"][1]["items"][i + 1]["start_jd"]) - f(&v["bodies"][1]["items"][i]["start_jd"]);
    assert!((13.0..14.5).contains(&span_of(&few, 0)));
    let month = ok(&s, &format!("/nakshatras?jd={JD}&days=10")).await;
    let within = month["bodies"][0]["items"].as_array().unwrap();
    assert!((10..=12).contains(&within.len()) && f(&within[within.len() - 1]["start_jd"]) <= JD + 10.0);
    assert!(close(f(&month["end_date"]["jd"]), JD + 10.0) && within[..5] == items[..5]);
    // A retrograde planet steps back: Mercury re-entered Mula from Purva Ashadha in late December 2023.
    let mercury = ok(&s, "/nakshatras?dt=2023-12-01T00:00:00&days=60&bodies=me").await;
    let path: Vec<(u64, bool)> = mercury["bodies"][0]["items"].as_array().unwrap().iter().map(|i| (i["num"].as_u64().unwrap(), i["direct"] == true)).collect();
    assert!(path.windows(2).any(|w| w[1].0 + 1 == w[0].0 && !w[1].1), "{path:?}");
    for bad in ["bodies=as&loc=56.1,-3.6", "days=-1"] {
        assert_eq!(get(&s, &format!("/nakshatras?jd={JD}&{bad}")).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    // The Sun spends about a fortnight in one; the ascendant has no stay.
    let sun = &lahiri["bodies"][0];
    assert!((13.0..14.5).contains(&(f(&sun["end_jd"]) - f(&sun["start_jd"]))));
    assert!(lahiri["bodies"][2].get("start_jd").is_none() && lahiri["bodies"][2].get("end_jd").is_none());
    assert_eq!(get(&s, &format!("/nakshatra?jd={JD}&bodies=as")).await.0, StatusCode::BAD_REQUEST);

    // A plain series: two samples a day for three days.
    let v = ok(&s, &format!("/progress?jd={JD}&bodies=su,mo&days=3&pd=2")).await;
    assert_eq!((v["unit"].as_str(), v["bodies"].as_array().unwrap().len()), (Some("days"), 2));
    let moon = v["bodies"][1]["items"].as_array().unwrap();
    assert_eq!(moon.len(), 6);
    assert!((f(&moon[1]["jd"]) - JD - 0.5).abs() < 1e-9);
    // The dates of the span: the moment asked about, and its two ends, each in full.
    assert_eq!((f(&v["date"]["jd"]), f(&v["start_date"]["jd"]), f(&v["end_date"]["jd"])), (JD, JD, JD + 3.0));
    assert_eq!((v["start_date"]["utc"].as_str(), v["end_date"]["utc"].as_str()), (Some("1996-01-12T11:25:00Z"), Some("1996-01-15T11:25:00Z")));
    assert_eq!(v["end_date"]["unix"], 821_445_900 + 3 * 86_400);
    // `years` is the same series over a longer span: one sample a day, and the same fields.
    let v = ok(&s, &format!("/progress?jd={JD}&bodies=su&years=2")).await;
    let sun = v["bodies"][0]["items"].as_array().unwrap();
    assert_eq!((v["unit"].as_str(), sun.len()), (Some("days"), 731));
    assert_eq!((f(&v["span"]), f(&v["end_date"]["jd"])), (731.0, JD + 731.0));
    let expected = s.eph.calc_position(Body::Sun, JD + 30.0, GeoPos::default(), CoordSystem::Ecliptic, Perspective::Geocentric).unwrap();
    assert_eq!((f(&sun[30]["jd"]), f(&sun[30]["lng"])), (JD + 30.0, expected.lng));
    let (daily, yearly) = (moon[0].as_object().unwrap(), sun[0].as_object().unwrap());
    assert!(daily.keys().eq(yearly.keys()) && !yearly.contains_key("date") && !yearly.contains_key("age"));
    // Other spans carry the same three dates; an open-ended one has no end.
    let v = ok(&s, &format!("/ascendant-progress?jd={JD}&loc=51.5,-0.1&days=2&pdays=1")).await;
    assert_eq!((f(&v["date"]["jd"]), f(&v["start_date"]["jd"]), f(&v["end_date"]["jd"])), (JD, JD - 1.0, JD + 2.0));
    let v = ok(&s, &format!("/tithis?jd={JD}&num=3")).await;
    assert!(f(&v["start_date"]["jd"]) == JD && v["end_date"].is_null());
    let v = ok(&s, &format!("/tithis?jd={JD}&days=10")).await;
    assert_eq!(f(&v["end_date"]["jd"]), JD + 10.0);

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
        ("/positions?eq=3", StatusCode::BAD_REQUEST),
        ("/positions?ps=topo", StatusCode::BAD_REQUEST),
        ("/positions?bodies=as,su", StatusCode::BAD_REQUEST),
        ("/longitude", StatusCode::BAD_REQUEST),
        ("/longitude?body=as", StatusCode::BAD_REQUEST),
        ("/longitudes?bodies=su,zz", StatusCode::BAD_REQUEST),
        ("/longitudes?ps=helio&bodies=su", StatusCode::BAD_REQUEST),
        ("/longitudes?aya=zz", StatusCode::BAD_REQUEST),
        ("/positions?cs=all", StatusCode::BAD_REQUEST),
        ("/extended-positions", StatusCode::BAD_REQUEST),
        ("/extended-position?loc=10,10", StatusCode::BAD_REQUEST),
        ("/progress?cs=all&days=3&loc=10,10", StatusCode::BAD_REQUEST),
        ("/positions?ps=local", StatusCode::BAD_REQUEST),
        ("/positions?ps=martian", StatusCode::BAD_REQUEST),
        ("/positions?ps=helio&bodies=su", StatusCode::BAD_REQUEST),
        ("/positions?ps=areo&bodies=ra", StatusCode::BAD_REQUEST),
        ("/positions?ps=helio&cs=hor&loc=10,10", StatusCode::BAD_REQUEST),
        ("/positions?cs=hor", StatusCode::BAD_REQUEST),
        ("/positions?loc=95,0&ps=topo", StatusCode::BAD_REQUEST),
        ("/positions?aya=zz", StatusCode::BAD_REQUEST),
        ("/positions?sid=1", StatusCode::BAD_REQUEST),
        ("/position", StatusCode::BAD_REQUEST),
        ("/angles?jd=2450094.9", StatusCode::BAD_REQUEST),
        ("/houses?loc=10,10&hsys=Q", StatusCode::BAD_REQUEST),
        ("/rise-set?loc=10,10&mode=9", StatusCode::BAD_REQUEST),
        ("/rise-set?loc=10,10&disc=middle", StatusCode::BAD_REQUEST),
        ("/rise-set?loc=10,10&twilight=dusk", StatusCode::BAD_REQUEST),
        ("/rise-set?loc=10,10&horizon=95", StatusCode::BAD_REQUEST),
        ("/rise-set?loc=10,10&refraction=maybe", StatusCode::BAD_REQUEST),
        ("/progress?bodies=su", StatusCode::BAD_REQUEST),
        ("/progress?days=1000000&pd=24", StatusCode::BAD_REQUEST),
        ("/aspects?aspects=90:x", StatusCode::BAD_REQUEST),
        ("/rise-sets", StatusCode::BAD_REQUEST),
        ("/transposed-transits?loc=10,10", StatusCode::BAD_REQUEST),
        ("/transposed-transits?h_jd=2451545", StatusCode::BAD_REQUEST),
        ("/transposed-transits?loc=10,10&h_jd=2451545&h_loc=abc", StatusCode::BAD_REQUEST),
        ("/transposed-positions?loc=10,10", StatusCode::BAD_REQUEST),
        ("/transposed-positions?h_jd=2451545", StatusCode::BAD_REQUEST),
        ("/transposed-positions?loc=10,10&h_jd=2451545&ps=topo", StatusCode::BAD_REQUEST),
        ("/transposed-positions?loc=10,10&h_jd=2451545&ps=helio", StatusCode::BAD_REQUEST),
        ("/rise-sets?loc=10,10&days=1.5", StatusCode::BAD_REQUEST),
        ("/rise-sets?loc=10,10&pdays=-1", StatusCode::BAD_REQUEST),
        ("/rise-sets?loc=10,10&days=0&pdays=0", StatusCode::BAD_REQUEST),
        ("/rise-sets?loc=10,10&days=5000", StatusCode::BAD_REQUEST),
        ("/positions?jd=-4000000", StatusCode::UNPROCESSABLE_ENTITY), // 15,665 BCE: before every JPL file
        ("/nowhere", StatusCode::NOT_FOUND),
    ] {
        let (got, body) = get(&s, path).await;
        assert_eq!(got, status, "{path}: {body}");
        assert_eq!(body["valid"], false, "{path}");
        assert!(body["error"].as_str().is_some_and(|e| !e.is_empty()), "{path}");
    }
}

async fn post(state: &Arc<AppState>, path: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(path).header("content-type", "application/json").body(HttpBody::from(body.to_string())).unwrap();
    let response = router(state.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Records at scattered dates, alternately tagged `f` and `m`, deliberately unsorted.
fn bulk_records(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            let jd = 2_415_020.5 + ((i * 7919) % 73_000) as f64 + 0.37;
            serde_json::json!({
                "ref": format!("Person{i}"), "lat": 44.8993 - (i % 50) as f64, "lng": 12.39373 + (i % 90) as f64, "jd": jd,
                "tags": { "sex": if i % 2 == 0 { "f" } else { "m" }, "group": i % 3 },
            })
        })
        .collect()
}

#[tokio::test]
async fn bulk_charts_echo_refs_and_tags() {
    let Some(s) = state() else { return };
    let mut records = bulk_records(40);
    records[5] = serde_json::json!({ "ref": "UniqueNameA", "lat": 44.8993, "lng": 12.39373, "dt": "1976-12-08T17:39:45Z", "tags": { "sex": "f" } });
    records.push(serde_json::json!({ "ref": "BadDate", "lat": 0.0, "lng": 0.0, "dt": "not a date" }));
    records.push(serde_json::json!({ "ref": "TooEarly", "lat": 0.0, "lng": 0.0, "jd": 0.0 }));
    records.push(serde_json::json!({ "ref": "NoPlace", "dt": "1990-01-01T00:00:00Z" }));
    let (status, all) = post(&s, "/bulk/charts?bodies=as,su,mo,ma", Value::Array(records.clone())).await;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(all["count"], 40);
    let errors: Vec<&str> = all["errors"].as_array().unwrap().iter().map(|e| e["ref"].as_str().unwrap()).collect();
    // A malformed item comes first, with its position; then bad values, then failed calculations.
    assert_eq!(errors, ["NoPlace", "BadDate", "TooEarly"]);
    assert_eq!(all["errors"][0]["index"], 42);
    // Input order, with the metadata handed back.
    let chart = &all["charts"][5];
    assert_eq!((chart["ref"].as_str(), chart["tags"]["sex"].as_str(), chart["engine"].as_str()), (Some("UniqueNameA"), Some("f"), Some("jpl")));
    assert_eq!(chart["date"]["utc"], "1976-12-08T17:39:45Z");
    let keys: Vec<&str> = chart["bodies"].as_array().unwrap().iter().map(|b| b["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["as", "su", "mo", "ma"]);
    // The same numbers as the single-chart endpoint.
    let single = ok(&s, "/positions?dt=1976-12-08T17:39:45&loc=44.8993,12.39373&bodies=as,su,mo,ma&ps=geo").await;
    for (bulk, one) in chart["bodies"].as_array().unwrap().iter().zip(single["bodies"].as_array().unwrap()) {
        assert!((f(&bulk["lng"]) - f(&one["lng"])).abs() < 1e-9);
    }
    // Early years are read as dates. Whether this one can be calculated depends on the
    // planetary file installed: a long one covers it, the DE440 files do not.
    let (status, early) = get(&s, "/longitude?body=su&dt=0900-07-06T09:24:37").await;
    if s.eph.date_range().is_some_and(|range| range.contains(2_049_964.89)) {
        assert_eq!(status, StatusCode::OK, "{early}");
        assert!(early["date"]["utc"].as_str().unwrap().starts_with("0900-07-06"), "{early}");
    } else {
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{early}");
        assert!(early["error"].as_str().unwrap().starts_with("0900-07-06"), "{early}");
    }
    // The wrapped form and the tag filter.
    let (_, women) = post(&s, "/bulk/charts?bodies=su&tags=sex:f,group:0", serde_json::json!({ "items": records })).await;
    let list = women["charts"].as_array().unwrap();
    assert!(!list.is_empty() && list.len() < 20);
    assert!(list.iter().all(|c| c["tags"]["sex"] == "f" && c["tags"]["group"] == 0));
    // Malformed bodies are a 400, not a panic.
    let (status, bad) = post(&s, "/bulk/charts", serde_json::json!({ "nonsense": true })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    assert_eq!(bad["valid"], false);
}

#[tokio::test]
async fn bulk_aspect_searches_return_only_matches() {
    let Some(s) = state() else { return };
    let records = bulk_records(300);
    let query = "bodies=ma&bodies2=su,mo,me,ve,ma,ju,sa&aspects=150:2.0167";
    let request = serde_json::json!({ "source": records[0], "target": &records[1..] });
    let (status, found) = post(&s, &format!("/bulk/aspects/one-to-many?{query}"), request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(found["source"]["ref"], "Person0");
    let matches = found["matches"].as_array().unwrap();
    assert!(!matches.is_empty() && matches.len() < 150, "{}", matches.len());
    assert_eq!(found["count"], matches.len());
    for m in matches {
        assert!(m["ref"].as_str().unwrap().starts_with("Person") && m["tags"]["sex"].is_string());
        for a in m["aspects"].as_array().unwrap() {
            assert_eq!((a["a"].as_str(), f(&a["aspect"])), (Some("ma"), 150.0));
            assert!(f(&a["divergence"]) <= 2.0167);
        }
        assert!(f(&m["closest"]) <= 2.0167);
    }
    // Filtering by tag keeps a subset of the same matches.
    let (_, women) = post(&s, &format!("/bulk/aspects/one-to-many?{query}&tags=sex:f"), request.clone()).await;
    let women = women["matches"].as_array().unwrap();
    assert!(women.len() < matches.len());
    assert!(women.iter().all(|w| w["tags"]["sex"] == "f" && matches.iter().any(|m| m["ref"] == w["ref"])));
    // Progressing the source changes what matches.
    let (status, progressed) = post(&s, &format!("/bulk/aspects/one-to-many?{query}&prog=a&dt=2020-01-01"), request).await;
    assert_eq!(status, StatusCode::OK, "{progressed}");
    assert_eq!(progressed["progressed"]["a"], "progressed");
    assert_ne!(progressed["matches"], found["matches"]);

    // Pairs: only within each pair, both refs echoed.
    let pairs: Vec<Value> = records.chunks(2).map(|p| serde_json::json!({ "a": p[0], "b": p[1] })).collect();
    let (status, found) = post(&s, "/bulk/aspects/pairs?bodies=su,mo,ve,ma&aspects=150:2.0167", Value::Array(pairs)).await;
    assert_eq!(status, StatusCode::OK, "{found}");
    let matches = found["matches"].as_array().unwrap();
    assert!(!matches.is_empty() && matches.len() < 150);
    for m in matches {
        let (a, b): (usize, usize) = (m["a"]["ref"].as_str().unwrap()[6..].parse().unwrap(), m["b"]["ref"].as_str().unwrap()[6..].parse().unwrap());
        assert_eq!((a % 2, b), (0, a + 1));
    }

    // Many to many: every match names two different records of the list.
    let (status, found) = post(&s, "/bulk/aspects/many-to-many?bodies=ma&bodies2=ve&aspects=150:2.0167", Value::Array(records[..80].to_vec())).await;
    assert_eq!(status, StatusCode::OK, "{found}");
    let matches = found["matches"].as_array().unwrap();
    assert!(!matches.is_empty());
    assert!(matches.iter().all(|m| m["a"]["ref"] != m["b"]["ref"] && m["aspects"][0]["a"] == "ma" && m["aspects"][0]["b"] == "ve"));
    // Too long a list for the quadratic search is refused.
    let (status, _) = post(&s, "/bulk/aspects/many-to-many?bodies=ma", Value::Array(bulk_records(5_001))).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}
