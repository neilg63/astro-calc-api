# astro-calc-api

This application extends [Stéphane Bressani's](https://github.com/stephaneworkspace/libswe-sys) Rust bridge for Swiss Ephemeris calculation engine to support:

- Rise and set times for the referenced day/night period
- Altitude calculations
- Easy conversion of chrono datetimes (NaiveDatetimes) or ISO Datetime strings to and from Julian Days
- Ayanamshas for sidereal longitudes used in Indian astrology
- Sun period and longitudes at sunrise and sunset

This is the open-source version of a custom API server, using ActixWeb, with a rich set of astronomical and astrological features, and uses the [Julian Day Converter](https://crates.io/crates/julian_day_converter) library crate for integration with the chrono crate to handle date-time interopability.

A simple Web interface showcasing many of the possible calculations, with a few derived custom features, is available at [astroui.findingyou.co/](https://astroui.findingyou.co/) available in a separate [repo](https://github.com/neilg63/astrocalc-front), which also uses the parallel [GeoTimeZone API](https://github.com/neilg63/geotime) project.

## Build instructions:

You may use `cargo build` to build an executable for your operating system (all versions of Linux, Mac or Windows supported by Rust 1.61+). However, you will have to configure the Swiss Ephemeris data library. This may already be available if you have installed other versions of Swiss Ephemeris. On Linux libswe is installed at `/usr/share/libswe/ephe`. The source files can be downloaded from [www.astro.com/ftp/swisseph/](https://www.astro.com/ftp/swisseph/) and please contact Alois Treindl for more information about [Swiss Ephemeris licensing](https://www.astro.com/swisseph/).

The API is publicly available at [astroapi.findingyou.co](https://astroapi.findingyou.co). This is a sample data-set with [equatorial and ecliptic coordinates as well as transitions of the sun, moon and core planets](https://astroapi.findingyou.co/chart-data?dt=2022-06-01T00:00:00&loc=48.15,6.667&ct=1&topo=1&eq=3&iso=1)

## Environment Variables

The application will detect the two key configuration options from a .env file in the launch directory. By default, this is in the project root where the production executable is at `target/release/astro_calc_api`.

You may set the following options as shown in *sample.env*:

- `sweph_path=/path-to-swiss-ephemeris-data-directory`
- `port=9999`

If the Swiss Ephemeris data path is not detected, Swiss Ephemeris will revert to the less accurate but simpler Moshier formula.

## Command line parameters

These will override the above.

- -e: ephemeris path
- -p: port number

## Endpoints

GET /jd/:datetef

Julian day, unix time stamp and UTC date-time string

Path parameters

- :dateref: either ISO date string with optional time or julian day

GET /date

Show date and time variants with current, previous and next sunrise and sunset data.

- dt: Date (ISO 8601 UTC)
- loc: lat,lng(,alt) coordinates as decimals, e.g. 45.1,13.2 is 45.1 N and 13.2º S, -21.75,-45.21 is 21º S and 45.21º W
- iso: 0 = julian days, 1 ISO UTC, Show sunrise and sunset time ISO 8601 UTC

### GET /positions

Longitudes of referenced celestial bodies and the ascendant. This may power simplified astrological charts. Use this endpoint, if all you need are longitudes, the ascendants plus sun and moon transitions for the day in question.

Query string parameters:

- **dt**: Date, must be UTC as 2024-02-28T12:00:00
- **jd**: Julian Day as a decimal.
- **loc**: lat,lng(,alt) coordinates
- **bodies**: comma-separated list of required bodies, all or core
- **topo**: 0 = geocentric, 1 topocentric
- **eq**: 0 = ecliptic, 1 equatorial
- **iso**: 0 = julian days (transition times), 1 ISO UTC
- **sid**: 0 = never apply specified ayanamsha (defaylt), 1 apply ayanamsha specified by *aya* in ecliptic mode
- **aya**: two letter or full machine name for the ayanamsha to be applied when sid=1 and eq=0

### GET /progress

Progress of celestial body positions. This may power charts and 3D animations of planetary orbits over time

Query string parameters:

- **dt**: start date
- **jd**: Julian Day as a decimal.
- **loc**: lat,lng(,alt) coordinates, required for topocentric
- **bodies**: comma-separated list of required bodies, all or core
- **days**: number of days worth of transitions, default 28, e.g. 366 will return a whole year
- **pd**: number of samples per day, default 2, i.e. every 12 hours
- **dspan**: number of days per sample, overrides pd above for longer spans, max 1000 samples
- **topo**: 0 = geocentric, 1 topocentric
- **eq**: 0 = ecliptic only, 1 equatorial only, 3 horizontal (2 = dual is not applicable)

### GET /chart-data

Rich configurable set of astrological data for a given time and geolocation. May power astrological charts with extra transitions and ayanamsha variants, progress synastry positions (P2) and house systems. Previous and next planet stations (retrograde motion switches) will be shown if retro is 1.

Query string parameters:

- **dt**: Data
- **jd**: Julian Day as a decimal.
- **loc**: lat,lng(,alt) coordinates
- **bodies**: comma-separated list of 2-letter abbreviations for required bodies, all or core
- **topo**: 0 = geocentric, 1 topocentric
- **eq**:
  - 0 = ecliptic only,
  - 1 equatorial only,
  - 2 both ecliptic and equatorial,
  - 3 both with altitude, azimuth and extra planetary phenomena such as magnitude and phase angle. The azimuth and altitude will only be shown in topocentric mode.
  - 4 With extra planetary phenomena such as magnitude and phase angle as an inline subset.
- **ph**: 1 = show planetary phenomena for the referenced time unless it is shown inline with celestial body data, 0 = no extra phenomena unless eq == 4
- **hsys**: Comma-separated list of house system letters or `all` for all systems, default W (whole house system)
- **ct**: 0 = default. 1 = show rise/set times (current transitions) for the selected bodies
- **mode**: Alignment and refraction options for rise / set times. See notes below for all options.
- **aya**: Comma-separated list of available ayanamshas (see below). These are added as separate data-set and should be applied in a post processing stage via simple subtraction from the lng or ascendant values, which are always tropical (they may be automatically applied in /positions)
- **retro**: 1: show retrograde and peak stations of the main planets, 0: do not show planet stations (default)

### GET /rise-set-times

- dt: current date-time
- loc: current lat,lng(,alt) coordinates
- bodies: comma-separated list of required bodies, all or core")
- iso: 0 = show julian days (default), 1 = show ISO datetime UTC
- mode: Alignment and refraction options for rise / set times. The default is *center disc* with refraction. Without refraction, the rise times may be 2 to 5 minutes later and set times 2 to 5 minutes earlier.
  - 0 => None / unadjusted
  - 1 => No Refraction only
  - 2 => Centre disc + no refraction
  - 3 => Centre disc only (default)
  - 4 => Bottom disc + no refraction
  - 5 => Bottom disc only
  - 6 => Fixed disc + no refraction
  - 7 => Fixed disc only

### GET /sun-rise-set-times

Query string parameters:

- dt: current date-time
- loc: current lat,lng(,alt) coordinates
- days: number of days worth of transitions, default 28, e.g. 366 will return a whole year")
- iso: 0 = show julian days (default), 1 = show ISO datetime UTC
- mode: Alignment and refraction options for rise / set times. See notes above for all options.
- full: 0,1 (default) show as dailt transition sets with max, min as well as next_rise/next_set and prev_set/prev_rise, 2: show as linear sequence of transition events

Calculate the alternating rise, set, IC and MC times as well and min. and max. altitudes of the sun over 1 or more days.

#### GET /ascendant

Progress of the ascendant only or, with the *bodies* option of celestial bodies too. By default, 24 longitude values are shown over a day starting 12 hours before the current time or the time referenced by the *dt* or *jd* parameters. The interval between samples (values) is displayed both as a simple string (e.g. 1h = 24 per day) and as a decimal fraction of day. Specify more days via the *days* parameter only affects the end time. It will always start 12 hour before tthe referenced tim. To show different intervals, use the the *pd* (per day) parameter, e.g. ```&pd=48``` would show one value every 30 minutes.
The "currentIndex" value serves to match the current value. This endpoint is handy to plot the progression of the ascendant with the Sun, Moon and core planets over a few days. If the Sun and Moon (su, mo) are specified via bodies the endpoint shows the sun/moon angle and moon phase.

Query string parameters:

- **dt**: start date
- **jd**: Julian Day as a decimal.
- **loc**: lat,lng(,alt) coordinates, required for topocentric
- **bodies**: comma-separated list of celestial bodies. If specified longitudes are shown as nested key/value sets.
- **days**: number of days worth of results, default = 1
- **pd**: number of samples per day, default 24, i.e. hourly intervals
- **topo**: 0 = geocentric, 1 topocentric, only for celestial bodies
- **eq**: 0 = ecliptic only, 1 equatorial only
- **ct**: 0 or 1 = show sun rise and set times if more than 0
- **iso**: 0 or 1 = show rise and set time as UTC date-time strings rather than julian days
- **mode**: 0 to 7: Alignment and refraction options as detailed above

### GET /transposed-rise-times

- dt: current date-time
- dt2: historical date-time
- loc: current lat,lng(,alt) coordinates
- loc2: historical lat,lng(,alt) coordinates
- bodies: comma-separated list of required bodies, all or core"
- iso: 0 = show julian days (default), 1 = show ISO datetime UTC
- mode: Alignment and refraction options for natural / current rise / set times. See notes above for all options.

Calculate the current transitions of the referenced bodies and their projected current transition times based on their historical positions. dt2 and loc2 refer the time and place of a historical event (e.g. birth). The rise/set times are recalculated based on their historic positions.

### GET /planet-stations

Show retrograde start, retrograde peak, retrograde end and forward peak speeds of the core planets over a specified period:

- dt: start date-time or year only, between 2000 and 2050
- dt2: end date-time or year only, between 2000 and 2050
- bodies: comma-separated list of required planets, all or core, but may only include me: Mercury, ve: Venus, ma: Mars, ju: Jupiter, sa: Saturn, ur: Uranus, ne: Neptune and pl: Pluto
- iso: 0 = show julian days (default), 1 = show ISO datetime UTC

### GET /test-rise-sets

Compare transition calculation methods. One uses swe_rise_calc and the other, better suited to polar latitudes uses swe_azalt to approximate transits by variations in altitude. Eventually, the latter method will be uses for all latitudes > 60º or < -60º.

Query string parameters:

- dt: referenced date-time
- loc: current lat,lng(,alt) coordinates
- bodies: comma-separated list of required bodies, all or core
- iso: 0 = show julian days (default), 1 = show ISO datetime UTC
- mode: Alignment and refraction options for rise / set times. See notes above for all options.

### GET /test-swe-rise

This lets you test the Swiss Ephemeris method swe_rise_trans with centre disc, bottom disc and fixed disc aligment with and without refraction. The default mode is *centre disc only*, which means *with refraction*. 

- dt: referenced date-time
- loc: current lat,lng(,alt) coordinates
- bodies: comma-separated list of required bodies, all or core
- iso: 0 = show julian days (default), 1 = show ISO datetime UTC
- mode: Alignment and refraction options for rise / set times. See notes above for all options.

### GET /pheno

This shows planetary phenomena for the referenced time and celestial bodies. This only applies to visible planets, moons and stars

Query string parameters:

- dt: referenced date-time
- bodies: comma-separated list of required bodies, all or core

## Option Legend

### Celestial Bodies / Planets, Sun, moons, asteroids etc. / Grahas

- all: All planets from Mercury to Pluto (except Earth) + Sun, Moon, Rahu (True Node) and Ketu
- core: All used in traditional astrology, Sun, Moon, Mars, Mercury, Jupiter, Saturn, Rahu and Ketu
- su: Sun
- mo: Moon
- me: Mercury
- ve: Venus
- ea: Earth
- ma: Mars
- ju: Jupiter
- sa: Saturn
- ne: Neptune
- ur: Uranus
- pl: Pluto
- ra: True Node / Rahu,
- ke: Opposite True Node / Ketu,
- mn: Mean Node
- sn: South Node
- kr: Kronos
- is: Isis
- jn: Juno
- ce: Ceres
- ch: Chiron

### House Systems

- A: equal
- B: Alcabitius
- C: Campanus
- D: equal (MC)
- E: equal
- F: Carter poli-equ.
- G: Gauquelin sectors
- H: horizon/azimut
- I: Sunshine
- i: Sunshine/alt.
- K: Koch
- L: Pullen SD
- M: Morinus
- N: equal/1=Aries
- O: Porphyry
- Q: Pullen SR
- R: Regiomontanus
- S: Sripati
- T: Polich/Page
- U: Krusinski-Pisa-Goelzer
- V: equal/Vehlow
- W: equal/ whole sign
- X: axial rotation system/Meridian houses
- Y: APC houses

### Ayanamshas (sidereal mode offsets)

- all: All variants listed below
- tc, true_citra: True Citra
- lh, lahiri: Lahiri
- kr, krishnamurti: Krishnamurti
- yu, yukteshwar: Yukteshwar
- ra, raman: Raman
- va, valensmoon: Valensmoon
- tm, true_mula: True Mula
- tr, true_revati: True Revati
- tp, true_pushya: True Pushya
- ts, true_sheoran: True Sheoran
- at, aldebaran_15_tau: Aldebaran 15 Tau
- gm, galcent_mula_wilhelm: Galcent Mula Wilhelm
- gc, galcent_cochrane: Galcent Cochrane
- hi, hipparchos: Hipparchos
- sa, sassanian: Sassanian
- us, ushashashi: Sassanian
- jb, jnbhasin: Jnbhasin

NB: Only the simplified /positions endpoint lets you apply ayanamshas via the sid=1 option as required by many astronomers. For /chart-data and /progress you may subtract the required ayanamsha from the longitude, ascendant, descendant and right ascension. This is much more efficient than letting the underlying Swiss Ephemeris engine do it for you. The data sets may include the current ayanamsha values. To recalculate in javascript:

```
const subtract360 = (lng, value) => (lng + 360 - value) % 360;
const adjustedLongitude = subtract360(tropicalLongitude);
```

Julian day to unix time:

```javascript

// Julian day to standard unix timestamp in seconds with 1970-01-01 00:00:00 UTC equal to 2440587.5 Julian days
const JULIAN_DAY_UNIX_EPOCH = 2440587.5; // 1970-01-01T00:00:00 UTC

const julianDayToUnixTime = (jd = 0) => {
  return (jd - JULIAN_DAY_UNIX_EPOCH) * 24 * 60 * 60;
};

// Julian day to millisecond timestamp as used by JavaScript and accepted as the first parameter of the Date() constructor.
const julianDayToMillisecondTimestamp = (jd = 0) => {
  return julianDayToUnixTime(jd) * 1000;
};

// reference date should equal 1978-06-28T10:33:49 UTC
const jd = 2443687.9401592254;

// construct JS Date object that will apply your local time.
const localJsDate = new Date( julianDayToMillisecondTimestamp(jd) )
```
