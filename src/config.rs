//! Server settings, read from the environment. A `.env` file in the working directory is
//! loaded first; variables already set in the real environment take precedence over it.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Settings with their defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `ASTRO_DATA_DIR`: folder holding the `.bsp` ephemeris files. Default `data`.
    pub data_dir: PathBuf,
    /// `ASTRO_STATION_TABLE`: station table file. Default `stations.bin` in the data folder.
    pub station_table: PathBuf,
    /// `HOST`: address to listen on. Default `127.0.0.1`.
    pub host: IpAddr,
    /// `PORT`: port to listen on. Default `8087`.
    pub port: u16,
}

impl Config {
    /// Loads `.env` if there is one, then reads the settings from the environment.
    pub fn load() -> Result<Self, String> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(e) if e.not_found() => {}
            Err(e) => return Err(format!("cannot read .env: {e}")),
        }
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Builds the settings from any source of variables; blank values count as unset.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let get = |key: &str| get(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let data_dir = PathBuf::from(get("ASTRO_DATA_DIR").unwrap_or_else(|| "data".into()));
        let station_table = get("ASTRO_STATION_TABLE").map_or_else(|| data_dir.join("stations.bin"), PathBuf::from);
        let host = match get("HOST") {
            Some(h) => h.parse().map_err(|_| format!("HOST is not an IP address: {h}"))?,
            None => IpAddr::V4(Ipv4Addr::LOCALHOST),
        };
        let port = match get("PORT") {
            Some(p) => p.parse().map_err(|_| format!("PORT is not a port number: {p}"))?,
            None => 8087,
        };
        Ok(Self { data_dir, station_table, host, port })
    }

    pub fn address(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_overrides_and_bad_values() {
        let none = Config::from_lookup(|_| None).unwrap();
        assert_eq!((none.data_dir.to_str(), none.station_table.to_str(), none.port), (Some("data"), Some("data/stations.bin"), 8087));
        assert_eq!(none.address().to_string(), "127.0.0.1:8087");
        let set = Config::from_lookup(|k| match k {
            "ASTRO_DATA_DIR" => Some("/srv/eph".into()),
            "HOST" => Some("0.0.0.0".into()),
            "PORT" => Some(" 9000 ".into()),
            _ => Some("  ".into()),
        })
        .unwrap();
        assert_eq!((set.data_dir.to_str(), set.station_table.to_str()), (Some("/srv/eph"), Some("/srv/eph/stations.bin")));
        assert_eq!(set.address().to_string(), "0.0.0.0:9000");
        let table = Config::from_lookup(|k| (k == "ASTRO_STATION_TABLE").then(|| "/tmp/st.bin".to_string())).unwrap();
        assert_eq!(table.station_table.to_str(), Some("/tmp/st.bin"));
        assert!(Config::from_lookup(|k| (k == "PORT").then(|| "eighty".to_string())).is_err());
        assert!(Config::from_lookup(|k| (k == "HOST").then(|| "localhost".to_string())).is_err());
    }
}
