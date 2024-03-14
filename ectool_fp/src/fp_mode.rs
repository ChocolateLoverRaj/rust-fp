use std::process::Command;

use substring::Substring;

#[derive(Copy, Clone, Debug)]
pub enum FpModeInput {
    CAPTURE,
    DEEP_SLEEP,
    FINGER_DOWN,
    FINGER_UP,
    ENROLL,
    MATCH,
    RESET,
    RESET_SENSOR,
    MAINTAINENCE,
}
impl FpModeInput {
    pub fn cli_str(&self) -> &'static str {
        match self {
            FpModeInput::CAPTURE => "capture",
            FpModeInput::DEEP_SLEEP => "deepsleep",
            FpModeInput::FINGER_DOWN => "fingerdown",
            FpModeInput::FINGER_UP => "fingerup",
            FpModeInput::ENROLL => "enroll",
            FpModeInput::MATCH => "match",
            FpModeInput::RESET => "reset",
            FpModeInput::RESET_SENSOR => "reset_sensor",
            FpModeInput::MAINTAINENCE => "maintenance",
        }
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum FpModeOutput {
    RESET,
    ENROLL_PLUS_IMAGE,
    ENROLL,
    MATCH,
}

impl FpModeOutput {
    pub fn from_code(code: usize) -> Self {
        match code {
            0x0 => Self::RESET,
            0x10 => Self::ENROLL,
            0x30 => Self::ENROLL_PLUS_IMAGE,
            0x40 => Self::MATCH,
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
