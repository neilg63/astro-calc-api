# astro-calc-api

A REST API for astronomical and astrological calculations, built with
[Axum](https://github.com/tokio-rs/axum) on the
[`astro-calc-core`](https://github.com/neilg63/astro-calc-core) library.

Every endpoint is a `GET` that answers for one moment, optionally at one place, and returns
JSON.

## Status: pre-alpha

Endpoints, parameters and response fields may still change.

## Running

The server needs the JPL ephemeris files that `astro-calc-core` reads; see that project's
README for how to fetch them.

```bash
cp env.example .env
```

```bash
cargo run --release
```

Settings come from environment variables. A `.env` file in the working directory is read
at start-up; a variable set in the real environment overrides the same one in `.env`.

| Variable | Default | Meaning |
|---|---|---|
| `ASTRO_DATA_DIR` | `data` | Folder holding the `.bsp` files |
| `ASTRO_STATION_TABLE` | `stations.bin` in the data folder | Pre-computed station table |
| `HOST` | `127.0.0.1` | IP address to listen on |
| `PORT` | `8087` | Port to listen on |

If the station table (built by the core crate's `build_station_table` example) is present
and matches the ephemeris, station lookups use it; otherwise they are computed live.

## Parameters

All parameters are optional query-string values; each endpoint uses the ones that apply.

**Date and time**

| Parameter | Meaning |
|---|---|
| `dt` | Date-time `YYYY-MM-DDThh:mm:ss`, or any leading part (`1990-08-23T12`, `1990-08-23`). UTC unless an offset is given. Defaults to now. |
| `jd` | Julian Day (UT). Takes precedence over `dt` and ignores the offsets. |
| `tzos` | Offset of `dt` from UTC in seconds, east positive (`tzs` is an alias). |
| `tzom` | The same offset in minutes. |
| `tzoh` | The same offset in hours; fractions allowed (`5.5`). |
| `dt2`, `jd2` | End of a range, for the endpoints that take one. |
| `days`, `years` | Length of a range or series. |

**Place**

| Parameter | Meaning |
|---|---|
| `loc` | `lat,lng` or `lat,lng,alt` in degrees and metres. |
| `lat`, `lng`, `alt` | Each overrides its part of `loc`. |

A place is required for topocentric positions, horizontal coordinates, angles, houses,
rise and set times and the Indian day.

**What to calculate**

| Parameter | Meaning |
|---|---|
| `bodies` | Comma-separated body keys, e.g. `su,mo,ve,ma`, and the set names `default` and `basic`. See `/bodies`; stars are `hip:NUMBER`. |
| `body` | A single body key. |
| `cs` | Coordinate system: `ecliptic`/`ecl` (default), `equatorial`/`equ`, `horizontal`/`hor`, or `all` for the three together. |
| `ps` | Perspective: where the observer is; see the table below. For the longitude and position endpoints the default is `topo` when a place is given and `geo` when not; elsewhere it is `geo`. |
| `aya` | Ayanamsha keys, comma-separated (`lh`, `kr`, `tc`, ...), or `all`. |
| `sid` | `1` subtracts the first ayanamsha from ecliptic longitudes. |
| `hsys` | House system letters, comma-separated (`P,K,W`), or `all`. Default `W`. |
| `num`, `before` | How many items to return, and how many preceding the date. |
| `pd` | Samples per day (or per year with `years`) in `/progress`. Default 1. |
| `aspects` | Aspect definitions as `angle:orb` pairs: `0:8,60:6,90:8`. |
| `orb` | One orb for the default aspects (conjunction, sextile, square, trine, opposition). |

**Body sets**

| Name | Bodies |
|---|---|
| `default` | `as` ascendant, `su` Sun, `mo` Moon, `me` Mercury, `ve` Venus, `ma` Mars, `ju` Jupiter, `sa` Saturn, `ur` Uranus, `ne` Neptune, `pl` Pluto, `ke` Ketu, `ra` Rahu |
| `basic` | `default` without `ur`, `ne` and `pl`, as used in Indian astrology |

- `default` is used when no bodies are named in the longitude and position endpoints.
  `core` is its former name and is still accepted for now. A set name can be mixed
  with keys: `bodies=basic,ch`.
- The ascendant (`as`) needs a place. Without one it is left out of a set, and asking for
  it by name is an error.
- Seen from another centre (`ps=helio` and the like) a set gains the Earth (`ea`, after
  Venus) and loses the centre itself, the ascendant and the lunar nodes.
- Ketu is the point opposite Rahu (the true node); when both are asked for it is derived
  from Rahu, not calculated twice.

**Perspectives** (`ps`)

| Key | Name | Observer |
|---|---|---|
| `geo` | `geocentric` | Centre of the Earth |
| `topo` | `topocentric` | A place on the Earth's surface |
| `helio` | `heliocentric` | Centre of the Sun |
| `seleno` | `selenocentric` | Centre of the Moon |
| `hermeo` | `hermeocentric` | Centre of Mercury |
| `cythero` | `cytherocentric` | Centre of Venus |
| `areo` | `areocentric` | Centre of Mars |
| `jovi` | `jovicentric` | Centre of Jupiter |
| `crono` | `cronocentric` | Centre of Saturn |
| `bary` | `barycentric` | Centre of mass of the solar system |
| `local` | `local-space-horizon` | A place on the Earth, with positions on the local horizon |

- From a centre other than the Earth, the Earth (`ea`) is a body and is included by
  default. The centre itself and the Earth's lunar nodes and apsides cannot be requested.
- Coordinates from every centre stay on the Earth's ecliptic or equator of date.
- `local` always returns azimuth in `lng`, measured from north through east, and altitude
  in `lat`, whatever `cs` says. Note that `cs=hor` with `ps=topo` counts azimuth from the
  south westwards instead.
- `topo` and `local` need a place. Horizontal coordinates need an observer on the Earth.

**Rise and set options** (`/rise-set` and `/indian-day`)

| Parameter | Meaning |
|---|---|
| `disc` | Part of the disc that touches the horizon: `upper`, `center` (default) or `lower`. |
| `refraction` | `1` standard refraction (default), `0` none. |
| `pressure`, `temp` | Air pressure in hPa and temperature in °C, to scale the refraction. Standard is 1010 and 10. |
| `fixed` | `1` uses a fixed mean radius for the Sun and Moon instead of the actual one. |
| `horizon` | Apparent altitude of the horizon in degrees, for a ridge (`2`) or a view downwards (`-1`). |
| `twilight` | `civil`, `nautical` or `astronomical`: the Sun's centre 6°, 12° or 18° below the horizon. Disc, refraction and dip are then not used. |
| `dip` | `1` lowers the horizon by the dip for the observer's altitude (`alt`), taken as height above the surrounding ground or sea. |
| `geoc` | `1` uses geocentric positions, ignoring parallax. |
| `nolat` | `1` places the body on the ecliptic, ignoring its latitude. |
| `hindu` | `1` is the traditional Hindu rising: centre, no refraction, geocentric, no latitude. |
| `mode` | The older numeric presets 0–7; the parameters above override the preset. |

The conventional almanac sunrise and sunset are `disc=upper`. The response's `options`
object shows what the parameters resolved to.

| `mode` | Disc | Refraction | Fixed radius |
|---|---|---|---|
| 0 | upper | yes | no |
| 1, 3 | center | yes | no |
| 2 | center | no | no |
| 4 | lower | no | no |
| 5 | lower | yes | no |
| 6 | upper | no | yes |
| 7 | upper | yes | yes |

The older `topo=1`, `eq=1` and `eq=3` are accepted as aliases for `ps=topo`, `cs=equ` and
`cs=hor`.

In responses the two angles of a position are named after the coordinate system:

| `cs` | Angles | Speeds |
|---|---|---|
| `ecl` | `lng`, `lat` | `lng_speed`, `lat_speed` |
| `equ` | `ras` (right ascension, in degrees), `dcl` (declination) | `ras_speed`, `dcl_speed` |
| `hor` | `azi` (azimuth), `alt` (altitude) | `azi_speed`, `alt_speed` |
| `all` | all six | all six |

`/extended-position` and `/extended-positions` are the same as `/position` and `/positions`
with `cs=all`. `dist` and `dist_speed` (AU) are the same in every system. `cs=all` needs a place and an
observer on the Earth, because it includes the horizon; it is not available in `/progress`.

## Endpoints

| Endpoint | Returns | Main parameters |
|---|---|---|
| `/` | Service information and the ephemeris date range | |
| `/bodies` | Body keys and whether each is available | |
| `/stars` | The 410 built-in named stars | |
| `/longitude` | One body's ecliptic longitude only | `body`, `ps`, `aya`, place |
| `/longitudes` | Several bodies' longitudes, with the ascendant | `bodies`, `ps`, `aya`, place |
| `/position` | One body's position | `body`, `cs`, `ps`, `aya`, `sid` |
| `/positions` | Several bodies' positions | `bodies`, `cs`, `ps`, `aya`, `sid` |
| `/extended-position` | One body in all three coordinate systems | `body`, `ps`, `aya`, `sid`, place |
| `/extended-positions` | Several bodies in all three coordinate systems | `bodies`, `ps`, `aya`, `sid`, place |
| `/progress` | Positions over a series of times | `bodies`, `days` or `years`, `pd` |
| `/ascendant` | The ascendant | place |
| `/angles` | Ascendant, MC, vertex and the other angles | place |
| `/houses` | Angles and house cusps | place, `hsys` |
| `/ayanamsha` | Ayanamsha values | `aya` |
| `/rise-set` | Rise, set and meridian transits for the local day | place, `bodies`, rise/set options |
| `/stations` | Current speed and retrograde stations | `bodies`, `num`, `before`, or a range |
| `/moon-phases` | The next lunar phases | `num`, or a range; `ps` |
| `/indian-day` | The sunrise-to-sunrise day and its time units | place, rise/set options |
| `/pheno` | Phase, elongation, diameter and magnitude | `bodies` |
| `/aspects` | Aspects between bodies at one moment | `bodies`, `aspects` or `orb`, `ps` |

`/progress` has two forms. With `days` it samples real time, `pd` times a day. With `years`
it is a secondary progression: one day of motion stands for each year, and every sample
carries the `age` and the real `date` it represents.

`/longitude` and `/longitudes` return nothing but longitudes and cost about half as much
as full positions. Naming an ayanamsha in `aya` makes them sidereal directly; `sid` is not
needed there.

## Examples

```bash
curl 'http://127.0.0.1:8087/positions?dt=1996-01-12T11:25:00&loc=56.1,-3.6&ps=topo&bodies=su,mo,ve'
```

```bash
curl 'http://127.0.0.1:8087/houses?dt=1996-01-12T16:55&tzoh=5.5&lat=28.6&lng=77.2&hsys=P,W'
```

```bash
curl 'http://127.0.0.1:8087/stations?bodies=me&num=4'
```

## Errors

Errors are JSON with `valid: false` and an `error` message. A bad parameter gives status
400, a date outside the ephemeris files 422, and an unknown path 404.

## Tests

```bash
cargo test
```

The tests call the router directly and need the ephemeris files in
`../astro-calc-core/data`; they are skipped if the files are absent.

## Licence

Licensed under either of the MIT licence ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
