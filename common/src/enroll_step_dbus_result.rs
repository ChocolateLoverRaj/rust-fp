use rust_fp::fingerprint_driver::EnrollStepResult;
use serde::{Deserialize, Serialize};
pub use rust_fp::fingerprint_driver::EnrollStepOutput;

#[derive(Serialize, Deserialize, Debug)]
pub struct EnrollStepDbusOutput {
    pub id: u32,
    pub result: EnrollStepResult,
}
