use rust_fp::fingerprint_driver::OpenedFingerprintDriver;
use zbus::{fdo, interface};
use log::info;
use postcard::to_allocvec;
use rand::random;
use rust_fp::fingerprint_driver::EnrollStepOutput;

use crate::enroll_step_dbus_result::EnrollStepDbusOutput;


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
        let result = self
            .driver
            .start_or_continue_enroll()
            .await;
        if let Ok(EnrollStepOutput::Complete(_)) = result {
            self.enrolling_id = None;
        }
        info!("Enroll id: {id}. Result: {result:?}.");
        Ok(to_allocvec(&EnrollStepDbusOutput {
            id,
            result
        }).unwrap())
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
