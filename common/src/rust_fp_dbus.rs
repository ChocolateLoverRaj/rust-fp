use rust_fp::fingerprint_driver::OpenedFingerprintDriver;
use zbus::{fdo, interface};
use log::{error, info};
use postcard::to_allocvec;
use rand::random;
use rust_fp::fingerprint_driver::{EnrollStepError, EnrollStepOutput};

use crate::enroll_step_dbus_output::EnrollStepDbusOutput;

pub struct RustFp {
    pub driver: Box<dyn OpenedFingerprintDriver>,
    pub enrolling_id: Option<u32>,
}

#[interface(
    name = "org.rust_fp.RustFp",
    proxy(
        default_path = "/org/rust_fp/RustFp",
        default_service = "org.rust_fp.RustFp"
    )
)]
impl RustFp {
    async fn get_max_templates(&mut self) -> fdo::Result<u64> {
        let max_templates = self
            .driver
            .get_max_templates()
            .map_err(|_e| fdo::Error::Failed("Error getting max templates".into()))?;
        Ok(max_templates as u64)
    }

    async fn enroll_step(&mut self, id: u32) -> fdo::Result<Vec<u8>> {
        let id = match self.enrolling_id {
            None => {
                let id = random();
                self.enrolling_id = Some(id);
                Ok(id)
            }
            Some(enrolling_id) => {
                if enrolling_id != id {
                    // TODO: Have a timeout or something so a faulty app doesn't disable the FP sensor until this program is restarted
                    Err(fdo::Error::Failed(
                        "Something else is in the middle of enrolling. Wait until it's done."
                            .into(),
                    ))
                } else {
                    Ok(id)
                }
            }
        }?;
        let output = self
            .driver
            .start_or_continue_enroll()
            .await
            .map_err(|error| {
                fdo::Error::Failed(
                    match error {
                        EnrollStepError::GenericError => {
                            error!("Generic error: {error:?}");
                            "Error"
                        },
                        EnrollStepError::LowQuality => "Low Quality",
                    }
                    .into(),
                )
            })?;
        if let EnrollStepOutput::Complete(_) = output {
            self.enrolling_id = None;
        }
        info!("Enroll id: {id}");
        Ok(to_allocvec(&EnrollStepDbusOutput { id, output }).unwrap())
    }

    async fn match_templates(&mut self, templates: Vec<Vec<u8>>) -> fdo::Result<Vec<u8>> {
        let output = self
            .driver
            .match_templates(&templates)
            .await
            .map_err(|e| fdo::Error::Failed(format!("{e:?}")))?;
        Ok(to_allocvec(&output).unwrap())
    }
}
