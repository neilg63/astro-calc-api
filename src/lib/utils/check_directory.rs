use std::{fs, path::Path};

pub fn validate_directory(ephemeris_path: &String) -> (bool, String) {
  let ephe_directory = Path::new(ephemeris_path);
  let has_path = ephe_directory.exists() && ephe_directory.is_dir();
  let validated_path: &str = if has_path { ephemeris_path.as_str() } else { "" };
  let is_valid = if has_path { validate_files(validated_path) } else { false };
  (is_valid, validated_path.to_owned())
}

fn validate_files(validated_path: &str) -> bool {
  let mut num_matched: u32 = 0;
  for file_ref in fs::read_dir(validated_path).unwrap() {
    if let Ok(file) = file_ref {
        let fp = file.path();
        if fp.is_file() {
          let fpp = fp.as_path();
          if let Some(ext) = fpp.extension() {
            if ext == "se1" {
              if let Some(filename) = fpp.file_name() {
                if let Some(filestr) = filename.to_str() {
                  if filestr.starts_with("se") {
                    num_matched += 1;
                  }
                }
              }
            }
          }
        }
        
    }
  }
  num_matched > 12
}
