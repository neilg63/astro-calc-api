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
| `bodies` | Comma-separated body keys, e.g. `su,mo,ve,ma`. See `/bodies`; stars are `hip:NUMBER`. |
| `body` | A single body key. |
| `cs` | Coordinate system: `ecliptic`/`ecl` (default), `equatorial`/`equ`, `horizontal`/`hor`. |
| `ps` | Perspective: `geocentric`/`geo` (default) or `topocentric`/`topo`. |
| `aya` | Ayanamsha keys, comma-separated (`lh`, `kr`, `tc`, ...), or `all`. |
| `sid` | `1` subtracts the first ayanamsha from ecliptic longitudes. |
| `hsys` | House system letters, comma-separated (`P,K,W`), or `all`. Default `W`. |
| `mode` | Rise/set mode 0–7: disc alignment and refraction. Default 3. |
| `num`, `before` | How many items to return, and how many preceding the date. |
| `pd` | Samples per day (or per year with `years`) in `/progress`. Default 1. |
| `aspects` | Aspect definitions as `angle:orb` pairs: `0:8,60:6,90:8`. |
| `orb` | One orb for the default aspects (conjunction, sextile, square, trine, opposition). |

The older `topo=1`, `eq=1` and `eq=3` are accepted as aliases for `ps=topo`, `cs=equ` and
`cs=hor`.

In responses, `lng` and `lat` are the two angles of the coordinate system asked for:
longitude and latitude, right ascension and declination, or azimuth and altitude.

## Endpoints

| Endpoint | Returns | Main parameters |
|---|---|---|
| `/` | Service information and the ephemeris date range | |
| `/bodies` | Body keys and whether each is available | |
| `/stars` | The 410 built-in named stars | |
| `/position` | One body's position | `body`, `cs`, `ps`, `aya`, `sid` |
| `/positions` | Several bodies' positions | `bodies`, `cs`, `ps`, `aya`, `sid` |
| `/progress` | Positions over a series of times | `bodies`, `days` or `years`, `pd` |
| `/ascendant` | The ascendant | place |
| `/angles` | Ascendant, MC, vertex and the other angles | place |
| `/houses` | Angles and house cusps | place, `hsys` |
| `/ayanamsha` | Ayanamsha values | `aya` |
| `/rise-set` | Rise, set and meridian transits for the local day | place, `bodies`, `mode` |
| `/stations` | Current speed and retrograde stations | `bodies`, `num`, `before`, or a range |
| `/moon-phases` | The next lunar phases | `num`, or a range; `ps` |
| `/indian-day` | The sunrise-to-sunrise day and its time units | place, `mode` |
| `/pheno` | Phase, elongation, diameter and magnitude | `bodies` |
| `/aspects` | Aspects between bodies at one moment | `bodies`, `aspects` or `orb`, `ps` |

`/progress` has two forms. With `days` it samples real time, `pd` times a day. With `years`
it is a secondary progression: one day of motion stands for each year, and every sample
carries the `age` and the real `date` it represents.

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
