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
| `cs` | Coordinate system: `ecliptic`/`ecl`/`0` (default), `equatorial`/`equ`/`1`, `horizontal`/`hor`/`2`, or `all` for the three together. `eq` is an older name for the same parameter. |
| `ps` | Perspective: where the observer is; see the table below. For the longitude and position endpoints the default is `topo` when a place is given and `geo` when not; elsewhere it is `geo`. |
| `aya` | Ayanamsha keys, comma-separated (`lh`, `kr`, `tc`, ...), or `all`. |
| `sid` | `1` or `true` subtracts the first ayanamsha from ecliptic longitudes. |
| `hsys` | House system letters, comma-separated (`P,K,W`), or `all`. Default `W`. `G` gives the 36 Gauquelin sectors. |
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

**Rise and set options** (`/rise-set`, `/rise-sets` and `/indian-day`)

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

The older names are still accepted: `topo=1` for `ps=topo`, and `eq` for `cs` with the same
values (`eq=0`, `eq=1` and `eq=2` for ecliptic, equatorial and horizontal). `cs` wins if both
are given.

### On/off parameters

`sid`, `fixed`, `dip`, `geoc`, `nolat`, `hindu` and `topo` are switches. `1`, `true` and `on`
turn one on; `0`, `false` and `off` turn it off, in any letter case (`sid=1`, `sid=true` and `sid=TRUE`
are the same). Any other value is ignored, as if the parameter had been left out, so its
default applies.

In responses the two angles of a position are named after the coordinate system:

| `cs` | Angles | Speeds |
|---|---|---|
| `ecl` | `lng`, `lat` | `lng_speed`, `lat_speed` |
| `equ` | `ras` (right ascension, in degrees), `dcl` (declination) | `ras_speed`, `dcl_speed` |
| `hor` | `azi` (azimuth), `alt` (altitude) | `azi_speed`, `alt_speed` |
| `all` | all six | all six |

In topocentric mode (`ps=topo` or `ps=local`, and by default whenever a place is given),
`/position`, `/positions` and the extended endpoints add `points`: the ascendant, descendant,
MC, IC, ARMC, vertex and the other chart angles of the moment and place, as in `/angles`.
These are tropical longitudes; `sid` does not change them.

`/extended-position` and `/extended-positions` are the same as `/position` and `/positions`
with `cs=all`. `dist` and `dist_speed` (AU) are the same in every system. For an observer on the Earth (`geo`, `topo` or `local`) `cs=all` needs a place, because it includes the
horizon. From any other centre (`helio`, `seleno` and so on) there is no horizon: no place is needed and
`azi`, `alt` and their speeds are `null`. `cs=all` is not available in `/progress`.

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
| `/ascendant` | The ascendant and the related angles, as `points`. The top-level `ascendant` field is deprecated: read `points.ascendant` | place |
| `/ascendant-progress` | The ascendant at even steps, as `ascendants` (`jd`, `lng`), from `pdays` days before the moment (default 1) to `days` days after it (default 8), with the `points` of the moment itself | place, `days`, `pdays`, `pd` (samples per day, default 24) |
| `/angles` | Ascendant, MC, vertex and the other angles | place |
| `/houses` | Angles and house cusps: 12 per system, or 36 for the Gauquelin sectors (`G`), which are numbered clockwise from the ascendant, so their longitudes decrease. With `aya` the cusps are sidereal by the first ayanamsha named (given back as `ayanamsha`, with `sidereal: true`): they move back by its value, and whole-sign houses start at the sidereal sign of the ascendant. The `angles` stay tropical | place, `hsys`, `aya` |
| `/ayanamsha` | Ayanamsha values | `aya` |
| `/rise-set` | Rise, set and meridian transits for the local day | place, `bodies`, rise/set options |
| `/rise-sets` | The same, with lowest and highest altitude, day by day | place, `bodies`, `days`, `pdays`, rise/set options |
| `/transposed-positions` | Each body's place at a historic moment, carried into the sky of the date, in all three coordinate systems | place, `h_dt` or `h_jd`, historic place, `bodies`, `ps` |
| `/transposed-transits` | Rise, set and meridian transits of those transposed places, day by day | place, `h_dt` or `h_jd`, historic place, `bodies`, `ps`, `days`, `pdays`, rise/set options |
| `/stations` | Current speed and retrograde stations | `bodies`, `num`, `before`, or a range |
| `/moon-phases` | The next lunar phases | `num`, or a range; `ps` |
| `/indian-day` | The sunrise-to-sunrise day and its time units | place, rise/set options |
| `/pheno` | Phase, elongation, diameter and magnitude | `bodies` |
| `/aspects` | Aspects between bodies at one moment | `bodies`, `aspects` or `orb`, `ps` |
| `POST /bulk/...` | Charts and aspect searches over many records | see [Bulk endpoints](#bulk-endpoints) |

`/progress` has two forms. With `days` it samples real time, `pd` times a day. With `years`
it is a secondary progression: one day of motion stands for each year, and every sample
carries the `age` and the real `date` it represents.

`/rise-sets` covers the local days from `pdays` before the date (default 1) up to `days`
after it (default 8): nine days by default. The default bodies are the Sun, Moon and the
five classical planets. For each body and day it returns `day_start`, `rise`, `mc`, `set`
and `ic` as Julian Days (UT), and `alt_min` and `alt_max`, the geometric altitudes in
degrees at the lower and upper transit. A time is `null` when the event does not happen
that day, e.g. the Sun in polar winter. At most 20,000 body-days per request.

### Transposed positions and transits

`/transposed-positions` and `/transposed-transits` take where each body stood at a historic
moment, typically a birth, and treat that ecliptic longitude and latitude as a fixed point
in the sky of the date and place asked about.

| Parameter | Meaning |
|---|---|
| `dt` or `jd`, and `loc` (or `lat`, `lng`, `alt`) | The target moment and place. The place is required. |
| `h_dt` or `h_jd` | The historic moment. Required. `h_dt` takes the same offset as `dt`. |
| `h_loc` (or `h_lat`, `h_lng`, `h_alt`) | The historic place. |
| `ps` | `topo`: the historic positions are seen from the historic place, the default when one is given. `geo`: from the centre of the Earth, the default without one. |
| `bodies` | Default: the Sun, Moon and the five classical planets. |

Both return `date` and `geo` as `/positions` does, `historic_date` and `historic_geo` in the
same form (`historic_geo` is `null` in geocentric mode), and `ps`. In topocentric mode they
add `points` and `historic_points`: the ascendant, MC and the other chart angles at the
target and at the historic moment and place, as in `/angles`.

Each body is keyed `tp:` and its source (`tp:su`), with the plain key in `source`.

`/transposed-positions` gives each point in all three coordinate systems, with the fields of
`/extended-positions`. `lng` and `lat` are the historic values; `ras` and `dcl` use the
obliquity of the date; `azi` and `alt` are for the target place and moment. Only `azi_speed`
and `alt_speed` are not zero, since the point turns with the sky and has no motion of its
own. `dist` is the body's distance at the historic moment.

`/transposed-transits` gives the same days and fields as `/rise-sets`, with each point's
`lng` and `lat`, but `days` defaults to 1 and `pdays` to 0. A fixed point has no disc and
no parallax, so of the rise/set options only `horizon`, `refraction` (with `pressure` and
`temp`), `dip` and `nolat` have an effect; `horizon_alt` in the response is the resulting
altitude at rising and setting.

```json
{ "key": "tp:su", "source": "su", "lng": 93.5725, "lat": 0.0001, "days": [ { "day_start": 2461366.5099, "rise": 2461366.8383, "mc": 2461367.0003,
  "set": 2461367.1620, "ic": 2461367.5004, "alt_min": -54.2375, "alt_max": 13.7482 }, … ] }
```

`/longitude` and `/longitudes` return nothing but longitudes and cost about half as much
as full positions. Naming an ayanamsha in `aya` makes them sidereal directly; `sid` is not
needed there.

## Bulk endpoints

For large sets of records the bulk endpoints take a `POST` with the records as JSON
(`content-type: application/json`). The options go in the query string, with the same
parameters as the `GET` endpoints. They are generic: the service calculates and filters by
position, and hands back each record's own `ref` and `tags` untouched, so anything specific
to an application stays in the client.

| Endpoint | Body | Returns |
|---|---|---|
| `POST /bulk/charts` | a list of records | the positions of `bodies` for each record |
| `POST /bulk/aspects/pairs` | a list of pairs | the pairs in aspect |
| `POST /bulk/aspects/one-to-many` | a source record and a list of targets | the targets in aspect to the source |
| `POST /bulk/aspects/many-to-many` | a list of records | every two records in aspect |

### The record

```json
{
  "ref": "UniqueNameA",
  "lat": 44.8993,
  "lng": 12.39373,
  "alt": 0,
  "dt": "1976-12-08T17:39:45Z",
  "tags": { "sex": "f" }
}
```

| Field | Type | Required | Meaning |
|---|---|---|---|
| `ref` | string | yes | The client's identifier, returned with every result. Not checked for uniqueness |
| `lat`, `lng` | number | yes | Latitude (−90 to 90, north positive) and longitude (−180 to 180, east positive) in degrees |
| `alt` | number | no | Altitude in metres. Default 0 |
| `dt` | string | one of `dt`, `jd` | UTC date-time, `YYYY-MM-DDThh:mm:ss`, with or without a final `Z`. Years before 1000 are accepted in the proleptic Gregorian calendar with astronomical years, negative ones included: `0900-07-06T09:24:37`, `-0043-03-15` (44 BC) |
| `jd` | number | one of `dt`, `jd` | Julian Day (UT). Takes precedence over `dt` |
| `tags` | any JSON | no | Returned with every result. If it is an object, its members can be used by the `tags` filter |

Records need not be sorted by date. Other fields are ignored.

### Request bodies

A list of records, for `/bulk/charts` and `/bulk/aspects/many-to-many`, sent either bare or
under `items`:

```json
[
  { "ref": "UniqueNameA", "lat": 44.8993, "lng": 12.39373, "dt": "1976-12-08T17:39:45Z", "tags": { "sex": "f" } },
  { "ref": "UniqueNameB", "lat": 48.1462, "lng": 11.38975, "dt": "1969-07-06T09:24:37Z" }
]
```

```json
{ "items": [ { "ref": "UniqueNameA", "lat": 44.8993, "lng": 12.39373, "jd": 2443121.2359375 } ] }
```

Pairs, for `/bulk/aspects/pairs` (also bare or under `items`). Aspects are sought only
between the `a` and the `b` of the same pair:

```json
[
  {
    "a": { "ref": "UniqueNameA", "lat": 62.1854, "lng": 16.6714, "dt": "1973-04-17T11:23:19Z" },
    "b": { "ref": "UniqueNameB", "lat": 44.8993, "lng": 12.39373, "dt": "1976-12-08T17:39:45Z" }
  }
]
```

One to many, for `/bulk/aspects/one-to-many` (`targets` is accepted for `target`):

```json
{
  "source": { "ref": "UniqueNameA", "lat": 62.1854, "lng": 16.6714, "dt": "1973-04-17T11:23:19Z" },
  "target": [
    { "ref": "UniqueNameB", "lat": 44.8993, "lng": 12.39373, "dt": "1976-12-08T17:39:45Z", "tags": { "sex": "f" } },
    { "ref": "UniqueNameC", "lat": 48.1462, "lng": 11.38975, "dt": "1969-07-06T09:24:37Z", "tags": { "sex": "f" } }
  ]
}
```

### Query parameters

| Parameter | Used by | Meaning |
|---|---|---|
| `bodies` | all | Bodies of the chart, or of chart A in an aspect search. Default: the default set (with the ascendant) for charts; Sun, Moon and planets for aspects |
| `bodies2` | aspects | Bodies of chart B. Default: the same as `bodies` |
| `aspects`, `orb` | aspects | As for `/aspects`: `angle:orb` pairs such as `aspects=150:2.0167`, or one `orb` for the major aspects. An aspect counts on either side of chart A |
| `tags` | charts, one-to-many, many-to-many | Keep only records whose `tags` have all these members, as `key:value` pairs: `tags=sex:f,group:2`. Applied before anything is calculated. Not applied to `source` or to pairs |
| `prog` | aspects | Compare secondary-progressed charts (a day for each year of age) as they stand on `dt`/`jd` (default now): `a`, `b` or `ab` |
| `ps` | all | `geo` (default) or `topo` |
| `aya` with `sid=1` | all | Sidereal longitudes, each chart with the ayanamsha of its own date |

In an aspect search chart A is the `a` of a pair, the `source`, or either record of a
many-to-many list; chart B is the `b`, a target, or the other record.

### Response: `/bulk/charts`

`POST /bulk/charts?bodies=as,su,mo`

```json
{
  "ps": "geocentric",
  "sidereal": null,
  "count": 1,
  "charts": [
    {
      "ref": "UniqueNameB",
      "tags": { "sex": "f" },
      "date": { "jd": 2443121.2359375, "utc": "1976-12-08T17:39:45Z", "unix": 218914785 },
      "geo": { "lat": 44.8993, "lng": 12.39373, "alt": 0.0 },
      "engine": "jpl",
      "bodies": [
        { "key": "as", "dist": 0.0, "dist_speed": 0.0, "lng": 107.56774342163084, "lat": 0.0, "lng_speed": 291.814407679972, "lat_speed": 0.0 },
        { "key": "su", "dist": 0.9848618676039517, "dist_speed": -0.000122096962651419, "lng": 256.7761586614102, "lat": -0.00011947845843578805, "lng_speed": 1.015673628529612, "lat_speed": -1.4649013064418188e-05 },
        { "key": "mo", "dist": 0.0026606808365049854, "dist_speed": -1.8793591885415267e-05, "lng": 98.8716828827942, "lat": -4.704603314531093, "lng_speed": 12.3571274141554, "lat_speed": -0.446406895446394 }
      ]
    }
  ],
  "errors": []
}
```

| Field | Meaning |
|---|---|
| `ps` | The perspective used |
| `sidereal` | Key of the ayanamsha applied, or `null` for tropical longitudes |
| `count` | Number of charts returned |
| `charts` | One per record that could be calculated, in the order of the request |
| `charts[].ref`, `.tags` | As sent; `tags` is left out if none were sent |
| `charts[].date`, `.geo` | The moment and place, as in the `GET` endpoints |
| `charts[].engine` | How the positions were calculated: `jpl`, or `analytic` for the built-in series |
| `charts[].bodies` | Ecliptic positions in the order of `bodies`, as in `/positions`: degrees, degrees per day, AU. A negative `lng_speed` means retrograde |
| `errors` | Records left out; see [Errors in bulk requests](#errors-in-bulk-requests) |

### Response: aspect searches

`POST /bulk/aspects/one-to-many?bodies=su,mo,ve,ma&aspects=90:6,150:3&tags=sex:f`

```json
{
  "ps": "geocentric",
  "sidereal": null,
  "bodies": ["su", "mo", "ve", "ma"],
  "bodies2": ["su", "mo", "ve", "ma"],
  "targets": [ { "angle": 90.0, "orb": 6.0 }, { "angle": 150.0, "orb": 3.0 } ],
  "source": { "ref": "UniqueNameA", "tags": { "sex": "m" } },
  "count": 2,
  "matches": [
    {
      "ref": "UniqueNameB",
      "tags": { "sex": "f" },
      "closest": 0.23993932459211464,
      "aspects": [
        { "a": "su", "b": "ve", "lng_a": 27.309892975443553, "lng_b": 299.05427383564705, "aspect": 90.0, "orb": 6.0, "divergence": 1.7443808602034778 },
        { "a": "ve", "b": "ve", "lng_a": 29.29421316023914, "lng_b": 299.05427383564705, "aspect": 90.0, "orb": 6.0, "divergence": 0.23993932459211464 }
      ]
    },
    {
      "ref": "UniqueNameC",
      "tags": { "sex": "f" },
      "closest": 1.2149320634413243,
      "aspects": [
        { "a": "ma", "b": "su", "lng_a": 315.33643587500603, "lng_b": 104.12150381156471, "aspect": 150.0, "orb": 3.0, "divergence": 1.2149320634413243 }
      ]
    }
  ],
  "errors": []
}
```

`/bulk/aspects/pairs` and `/bulk/aspects/many-to-many` have no `source`, and each match
names both records:

```json
{
  "a": { "ref": "UniqueNameB", "tags": { "sex": "f" } },
  "b": { "ref": "UniqueNameC", "tags": { "sex": "f" } },
  "closest": 0.3898435798829496,
  "aspects": [
    { "a": "ve", "b": "ve", "lng_a": 299.05427383564705, "lng_b": 59.444117415530016, "aspect": 120.0, "orb": 6.0, "divergence": 0.3898435798829496 }
  ]
}
```

| Field | Meaning |
|---|---|
| `ps`, `sidereal` | As for charts |
| `bodies`, `bodies2` | The bodies taken from chart A and from chart B |
| `targets` | The aspects sought, each with its orb |
| `progressed` | Only with `prog`: the `date` progressed to, and whether `a` and `b` are `progressed` or `natal` |
| `source` | One-to-many only: the source's `ref` and `tags` |
| `count` | Number of matches |
| `matches` | Only the records, or pairs, with at least one aspect, in the order of the request |
| `matches[].ref`, `.tags` | One-to-many: the matching target |
| `matches[].a`, `.b` | Pairs and many-to-many: `ref` and `tags` of the records taken as chart A and chart B |
| `matches[].closest` | The smallest `divergence` among the aspects, for ranking |
| `matches[].aspects[]` | `a`, `b`: body keys of chart A and B; `lng_a`, `lng_b`: their longitudes; `aspect`, `orb`: the target matched; `divergence`: degrees from exact |
| `errors` | Records left out |

In a many-to-many search with the same `bodies` and `bodies2` each two records appear once,
the earlier of the list as `a`. With different lists they are tried both ways round, and
`a` is the record whose `bodies` were used.

### Errors in bulk requests

A problem with one record never fails the request. The record is left out and listed:

```json
"errors": [
  { "index": 0, "ref": "X", "error": "invalid item: invalid type: string \"north\", expected f64" },
  { "index": 1, "ref": null, "error": "invalid item: missing field `ref`" },
  { "ref": "Y", "error": "invalid location: latitude 95 out of range -90..=90" },
  { "ref": "Z", "error": "1585-07-06 (JD 2300155.501488508) is outside the JPL ephemeris data for NAIF id 399, which covers 1849-12-26 to 2150-01-22" }
]
```

An item that cannot be read at all (a missing or mistyped field) has its position in the
list as `index`, and its `ref` if one could be found. A pair is left out if either of its
records fails. The request as a whole fails, with `valid: false` and an `error` as for the
other endpoints, only when:

| Status | Cause |
|---|---|
| 400 | The body is not JSON, is not a list (or has none under `items`, or under `target`), the `source` is missing or invalid, or a query parameter is wrong |
| 413 | More than 100,000 records, more than 5,000 after filtering for a many-to-many search, or a body over 64 MB |
| 422 | The `source` chart cannot be calculated |

### Example

```bash
curl -X POST 'http://127.0.0.1:8087/bulk/aspects/one-to-many?bodies=ma&bodies2=su,mo,ve&aspects=150:2.0167&tags=sex:f' -H 'content-type: application/json' -d @records.json
```

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
