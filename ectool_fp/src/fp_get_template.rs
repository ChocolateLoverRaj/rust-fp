use std::io;
use std::io::ErrorKind;
use std::process::Command;

pub fn fp_get_template(index: usize) -> Result<Vec<u8>, io::Error> {
    let output = Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fptemplate")
        .arg(index.to_string())
        .output()?;
    match output.status.success() {
        true => Ok(output.stdout),
        false => Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("Process exited with code {}", output.status),
        )),
    }
}
