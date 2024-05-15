use std::io;
use std::process::Command;

pub fn fp_enroll() -> Result<(), io::Error> {
    Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fpmode")
        .arg("enroll")
        .output()?;
    Ok(())
}
