use std::fmt::Display;

pub fn get_fp_file(home_dir: impl Display) -> String {
    format!("{}/.var/cros-fp-templates", home_dir)
}
