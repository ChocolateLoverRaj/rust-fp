use std::io;
use std::process::Command;

pub fn fp_hello() -> Result<(), io::Error> {
    Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("hello")
        .output()?;
    Ok(())
}
