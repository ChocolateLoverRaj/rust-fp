use serde::{Deserialize, Serialize};
pub use rust_fp::fingerprint_driver::EnrollStepOutput;

#[derive(Serialize, Deserialize)]
pub struct EnrollStepDbusOutput {
    pub id: u32,
    pub output: EnrollStepOutput,
}