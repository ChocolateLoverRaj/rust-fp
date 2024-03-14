use std::io;
use std::process::{Command, Output};

#[derive(Debug)]
pub enum FpLoadTemplateError {
    IoError(io::Error),
    StatusError(Output)
}

/// Load a template into the fingerprint sensor
pub fn fp_load_template(file: &str) -> Result<(), FpLoadTemplateError> {
    let output = Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fptemplate")
        .arg(file)
        .output().map_err(|e| FpLoadTemplateError::IoError(e))?;
    match output.status.success() {
        true => Ok(()),
        false => Err(FpLoadTemplateError::StatusError(output))
    }
}
