use std::os::raw::{c_char, c_double, c_int};
use serde::{Serialize, Deserialize};
use libswe_sys::sweconst::Bodies;
use crate::calc::settings::ayanamshas::*;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct AltitudeSet {
  pub azimuth: f64,
  pub value: f64,
  pub apparent: f64,
}

enum BodyAltitudes {
  EquToHor =1,
  EclToHor = 0,
}

#[link(name = "swe")]
extern "C" {
  
  pub fn swe_rise_trans(
      tjd_ut: c_double,
      ipl: c_int,
      starname: *mut [c_char; 1],
      epheflag: c_int,
      rsmi: c_int,
      geopos: *mut [c_double; 3],
      atpress: c_double,
      attemp: c_double,
      tret: *mut [c_double; 3],
      serr: *mut c_char
  );

  /*
   double tjd_ut,
    int32  calc_flag,
    double *geopos,
    double atpress,
    double attemp,
    double *xin, 
    double *xaz) 
  */

  pub fn swe_azalt(
      tjd_ut: c_double,
      iflag: c_int,
      geopos: *mut [c_double; 3],
      atpress: c_double,
      attemp: c_double,
      xin: *mut [c_double; 2],
      xaz: *mut [c_double; 3]
  );


  pub fn swe_get_ayanamsa_ex_ut(
      jd: c_double,
      iflag: c_int,
      daya: *mut c_double,
      serr: *mut c_char
  ) -> c_double;

  // swe_set_topo(double geolon, double geolat, double geoalt);

  pub fn swe_set_topo(
    lng: c_double,
    lat: c_double,
    alt: c_double
  );

  // swe_set_sid_mode(sidModeNum, 0, 0);
  pub fn swe_set_sid_mode(sid_mode: i32, t9: f64, ayan_t0: f64);


  /* // convert ecliptic to equatorial
  pub fn swe_cotrans(xin: *mut [c_double; 3], xout: *mut [c_double; 3], eps: c_double);
 */
}

/**
 * May trigger freeing of unallocated memory
 * Call via sleep
 */
pub fn rise_trans_raw(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64, iflag: i32) -> [f64; 3] {
  let mut serr = [0; 255];
  let geopos = &mut [lng, lat, 0f64];
  let star_ref = &mut ['\0' as i8]; // set to \0 cast as i8 to ignore *starname
  // epheflag:  Ephemeris flag as integer (SE$FLG_JPLEPH=1, SE$FLG_SWIEPH=2 or SE$FLG_MOSEPH=4)
  let epheflag: i32 = 1;
  let result = unsafe {
    let p_xx: &mut [f64; 3] = &mut [0f64, 0f64, 0f64];
    std::ptr::drop_in_place(p_xx);
    let p_serr = serr.as_mut_ptr();
    swe_rise_trans(
        tjd_ut,
        ipl as i32,
        star_ref,
        epheflag,
        iflag,
        geopos,
        0f64,
        0f64,
        p_xx,
        p_serr,
    );
    *p_xx
  };
  result.to_owned()
}

pub fn rise_trans(tjd_ut: f64, ipl: Bodies, lat: f64, lng: f64, iflag: i32) -> f64 {
  rise_trans_raw(tjd_ut, ipl, lat, lng, iflag)[0]
}

/*
  Wrapper for swe_azalt.
  tjd_jd: Julian Day,
  is_equal: if true 
*/
pub fn azalt(tjd_ut: f64, is_equal: bool, geo_lat: f64, geo_lng: f64, lng: f64, lat: f64) -> AltitudeSet {
  let iflag = if is_equal { BodyAltitudes::EquToHor } else { BodyAltitudes::EclToHor } as i32;
  let geopos = &mut [geo_lng, geo_lat, 0f64];
  let result = unsafe {
      let p_xin = &mut [lng, lat];
      let p_xaz = &mut [0f64, 0f64, 0f64];
      swe_azalt(
          tjd_ut,
          iflag,
          geopos,
          0f64,
          0f64,
          p_xin,
          p_xaz,
      );
      *p_xaz
  };
  AltitudeSet{
      azimuth: result[0] % 360f64,
      value: result[1] % 360f64,
      apparent: result[2] % 360f64,
  }
}

pub fn get_ayanamsha(tjd_ut: f64, mode: Ayanamsha) -> f64 {
  let mut daya: [f64; 1] = [0.0; 1];
  let mut serr = [0; 255];
  set_sid_mode(mode.as_i32());
  let result = unsafe {
      let p_daya = daya.as_mut_ptr();
      let p_serr = serr.as_mut_ptr();
      let status = swe_get_ayanamsa_ex_ut(
          tjd_ut,
          65536i32, // SEFLG_SIDEREAL
          p_daya,
          p_serr
      );
      status
  };
  //set_sid_mode(0);
  result
}

pub fn set_topo(lat: f64, lng: f64, alt: f64) {
  unsafe {
    swe_set_topo(lng, lat, alt);
  }
}

pub fn set_sid_mode(iflag: i32) {
  unsafe {
    swe_set_sid_mode(iflag, 0f64, 0f64);
  }
}


