/*
* Default values used for server configuration in tbe absence of a detected .env file
* or command line parameters
*/

pub const SWEPH_PATH_DEFAULT: &str = "/usr/share/libswe/ephe";
pub const DEFAULT_PORT: u32 = 8087;


pub fn empty_string() -> String {
  "" . to_string()
}
