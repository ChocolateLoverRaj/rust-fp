use std::io;
use std::process::{Command, Output};

pub fn fp_seed(seed: &str) -> io::Result<Output> {
    Command::new("ectool")
        .arg("--name=cros_fp")
        .arg("fpseed")
        .arg(seed)
        .spawn()
        .unwrap()
        .wait_with_output()
}
