use std::io::BufRead;
use std::process::Command;
use std::time::Duration;
use std::{io, thread};

#[derive(Debug)]
pub struct FpStats {
    pub last_matching_finger: Option<usize>,
}

/// Get information about the matched finger
pub fn fp_get_stats() -> Result<FpStats, io::Error> {
    // This is needed for some reason
    thread::sleep(Duration::from_millis(100));
    let output = Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fpstats")
        .output()?;

    Ok(FpStats {
        last_matching_finger: {
            // Sometimes instead of a matching time being -1 it will not have a matching time, and it will say "Invalid"
            let number = output
                .stdout
                .lines()
                .nth(2)
                .unwrap()
                .unwrap()
                .split_whitespace()
                .nth(6)
                .map(|number| number.to_owned());
            number
                .map(|mut number| {
                    // Change "-1)" to just "-1"
                    number.pop();
                    let match_index = number.parse::<isize>().unwrap();
                    match match_index {
                        -1 => None,
                        _ => Some(match_index as usize),
                    }
                })
                .flatten()
        },
    })
}
