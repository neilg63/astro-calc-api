use serde_json::json;
use crate::lib::settings::{ayanamshas::{all_ayanamsha_keys, AynamshaInfo}, house_systems::houses_as_key_map};
use actix_web::{get, Responder,web::Json};

#[get("/appendix")]
pub async fn appendix_info() -> impl Responder {
  let ayanamsha_details: Vec<AynamshaInfo> = all_ayanamsha_keys().into_iter().map(|ak| AynamshaInfo::new(ak)).collect();
  let houses = houses_as_key_map();
  Json(json!({ "ayanamshas": ayanamsha_details, "houseSystems": houses }))
}
