use std::process::Command;

use substring::Substring;

#[derive(Copy, Clone, Debug)]
pub enum FpModeInput {
    Capture,
    DeepSleep,
    FingerDown,
    FingerUp,
    Enroll,
    Match,
    Reset,
    ResetSensor,
    Maintenance,
}
impl FpModeInput {
    pub fn cli_str(&self) -> &'static str {
        match self {
            FpModeInput::Capture => "capture",
            FpModeInput::DeepSleep => "deepsleep",
            FpModeInput::FingerDown => "fingerdown",
            FpModeInput::FingerUp => "fingerup",
            FpModeInput::Enroll => "enroll",
            FpModeInput::Match => "match",
            FpModeInput::Reset => "reset",
            FpModeInput::ResetSensor => "reset_sensor",
            FpModeInput::Maintenance => "maintenance",
        }
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum FpModeOutput {
    Reset,
    EnrollPlusImage,
    Enroll,
    Match,
}

impl FpModeOutput {
    pub fn from_code(code: usize) -> Self {
        match code {
            0x0 => Self::Reset,
            0x10 => Self::Enroll,
            0x30 => Self::EnrollPlusImage,
            0x40 => Self::Match,
            _ => panic!("Unknown fpmode code: {:#?}", code),
        }
    }
}

pub fn fp_set_mode(fp_mode_input: FpModeInput) -> Result<(), std::io::Error> {
    Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fpmode")
        .arg(fp_mode_input.cli_str())
        .output()?;
    Ok(())
}

pub fn fp_get_mode() -> Result<FpModeOutput, std::io::Error> {
    let output = Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fpmode")
        .output()?;
    let output = String::from_utf8(output.stdout).unwrap();
    let output = output.split_whitespace().nth(2).unwrap();
    let output = output.substring(3, output.len() - 1);
    let output = usize::from_str_radix(output, 16).unwrap();
    Ok(FpModeOutput::from_code(output))
}
