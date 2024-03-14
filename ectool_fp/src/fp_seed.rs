use std::io;
use std::process::Command;

pub fn fp_seed(seed: &str) -> Result<(), io::Error> {
    Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fpseed")
        .arg(seed)
        .output()?;
    Ok(())
}
