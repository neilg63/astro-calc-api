use std::collections::HashMap;

fn info_map(pairs: Vec<(&str, &str)>) -> HashMap<String, String> {
  let mut info: HashMap<String, String> = HashMap::new();
  for pair in pairs {
    info.insert(pair.0.to_owned(), pair.1.to_owned());
  }
  info
}

pub fn endpoint_help() -> HashMap<String, HashMap<String,String>> {
  let mut help: HashMap<String, HashMap<String, String>> = HashMap::new();

  help.insert("GET /jd/:datetef".to_string(), info_map(
    vec![( 
      "description", "Julian day, unix time stamp and UTC date-time string"),
      (":dateref", "either ISO date string with optional time or julian day"),
    ]
  ));

  help.insert("GET /appendix".to_string(), info_map(
    vec![( 
      "description", "Names, keys, numbers and attribiutes of celestial objects (grahas), houses amd ayanamshas"),
    ]
  ));
  
  help.insert("GET /positions".to_string(), info_map(
    vec![
      ("description", "Longitudes of referenced celestial bodies and the ascendant"),
      ( "dt", "Date"),
      ("loc", "lat,lng(,alt) coordinates"),
      ("bodies", "comma-separated list of required bodies, all or core"),
      ("topo", "0 = geocentric, 1 topocentric"),
      ("eq", "0 = ecliptic, 1 equatorial"),
      ("iso", "0 julian days (rise/set times), 1 ISO UTC datetime strings"),
    ]
  ));
  help.insert("GET /chart-data".to_string(), info_map(
    vec![
      ("dt", "Date"),
      ("loc", "lat,lng(,alt) coordinates, e.g. &loc=45.336,13.278,50 or just &loc=45.336,13.278"),
      ("bodies", "comma-separated list of required bodies, all or core"),
      ("topo", "0: geocentric, 1: topocentric"),
      ("eq", "0: ecliptic only, 1 equatorial only, 2: show equatorial and ecliptic, 3: show azimuth and altitide. 4: Also show other planetary phenomena"),
      ("it", "1: show Indian time units with progression from sunrise to sunrise (sun periods) with longitudes, 0: do not show Indian time"),
      ("retro", "1: show retrograde and peak stations of the main planets, 0: do not show planet stations."),
      ("ct", "include rise/set times for the referenced bodies"),
      ("mode", "Alignment and refraction options for rise / set times. See 'transition options' for all options."),
      ("trbs", "Comma-seprated list of body keys for rise-set times. If not specified, the keys in bodies will be used"),
      ("hsys", "Comma-separated list of house system letters or `all` for all systems, default W (whole house system)"),
      ("aya", "comma-separated list of available ayanamshas (see below). These are added as separate data-set and should be applied in a post processing stage via simple subtraction from the lng, ascendant or rectAscension values, which are always tropical (they may automatically applied in /positions)"),
      ("iso", "0: julian days (transition times), 1: ISO UTC datetime strings"),
    ]
  ));
  help.insert("GET /progress".to_string(), info_map(
    vec![
      ("description", "Progress of celestial body positions"),
      ( "dt", "start date"),
      ("loc", "lat,lng(,alt) coordinates, required for topocentric, e.g. &loc=45.336,13.278,50 or just &loc=45.336,13.278"),
      ("bodies", "comma-separated list of required bodies, all or core"),
      ("days", "number of days, default 28, e.g. 366 will return a whole year"),
      ("pd", "number of samples per day, default 2, i.e. every 12 hours"),
      ("dspan", "number of days per sample, overrides pd above for longer spans, max 1000 samples"),
      ("topo", "0 = geocentric, 1 topocentric"),
      ("eq", "0 = ecliptic only, 1 equatorial only"),
    ]
  ));
  help.insert("GET /rise-set-times".to_string(), info_map(
    vec![
      ("dt", "reference start date, default: current date"),
      ("loc", "current lat,lng(,alt) coordinates"),
      ("bodies", "comma-separated list of required bodies, all or core"),
      ("iso", "0 show all times as julian days, 1 show rise/set times as ISO UTC datetime strings"),
      ("days", "Number of days from the start date"),
      ("mode", "Alignment and refraction options for rise / set times. See 'transition options' for all options.")
    ]
  ));
  help.insert("GET /sun-rise-set-times".to_string(), info_map(
    vec![
      ("dateref", "current date-time"),
      ("loc", "current lat,lng(,alt) coordinates"),
      ("days", "Day span of rise/set times, default 28, e.g. 366 will return a whole year"),
      ("iso", "0 = show julian days (default), 1 = show ISO datetime UTC"),
      ("full", "0 or 1 = show as daily rise sets based on solar time, 2 = show a sequence of rise/MC/set/IC times with min/max altitudes (as for /rise-set-times above)"),
      ("mode", "Alignment and refraction options for rise / set times. See 'transition options' for all options.")
    ]
  ));

  help.insert("GET /pheno".to_string(), info_map(
    vec![
      ("dt", "current date-time"),
      ("loc", "current lat,lng(,alt) coordinates"),
      ("bodies", "comma-separated list of required bodies, all or core"),
    ]
  ));
   help.insert("GET /houses".to_string(), info_map(
    vec![
      ("dt", "current date-time"),
      ("loc", "current lat,lng(,alt) coordinates"),
      ("hsys", "Comma-separated list of house system letters or `all` for all systems, default W (whole house system)"),
      ("aya", "Selected aynanamsha, only one allowed. 0 means tropical)"),
    ]
  ));
  help
}

pub fn rise_set_option_help() -> HashMap<String, HashMap<String,String>> {
  let mut help: HashMap<String, HashMap<String, String>> = HashMap::new();
  help.insert("Rise / set times".to_string(), info_map(
    vec![
      ("0", "None / unadjusted"),
      ("1", "No Refraction only"),
      ("2", "Centre disc + no refraction"),
      ("3", "Centre disc only (default)"),
      ("4", "Bottom disc + no refraction"),
      ("5", "Bottom disc only"),
      ("6", "Fixed disc + no refraction"),
      ("7", "Fixed disc only")
    ]
  ));
  help
}