use std::thread;

use serde::{Deserialize, Serialize};
use zbus::{connection::Builder, fdo, interface, message::Header};
use zbus::zvariant::{Optional, Type};

use common::seed::SEED;
use ectool_fp::fp_enroll::fp_enroll;
use ectool_fp::fp_get_info::fp_get_info;
use ectool_fp::fp_get_template::fp_get_template;
use ectool_fp::fp_hello::fp_hello;
use ectool_fp::fp_mode::{fp_get_mode, FpModeOutput};
use ectool_fp::fp_seed::fp_seed;

use crate::get_user_id::get_user_id;
use crate::Mode::Enrolling;

mod get_user_id;

#[derive(Default, Clone, Type, Serialize, Deserialize)]
struct EnrollingData {
    user_id: u32,
    template: Optional<Vec<u8>>,
    images: u32
}

enum Mode {
    Enrolling(EnrollingData),
    Matching
}

#[derive(Default)]
struct CrosFp {
    mode: Option<Mode>,
}

#[interface(name = "org.crosfp.CrosFp")]
impl CrosFp {
    async fn start_enroll(&mut self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        match self.mode {
            Some(_) => Err(fdo::Error::AccessDenied("Sensor is busy".into())),
            None => {
                let user_id = get_user_id(header).await?;
                // seed
                let _ = fp_seed(SEED);
                fp_enroll()
                    .map_err(|e| fdo::Error::Failed(format!("Error enrolling: {:#?}", e)))?;
                self.mode = Some(Enrolling(EnrollingData {
                    user_id,
                    template: Default::default(),
                    images: Default::default()
                }));
                Ok(())
            }
        }
    }

    /// Proceeds to next step or completes
    async fn continue_enrolling(
        &mut self,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<EnrollingData> {
        match &mut self.mode {
            Some(Enrolling(enrolling_data)) => {
                let user_id = get_user_id(header).await?;
                if user_id == enrolling_data.user_id{
                    let fp_mode = fp_get_mode().map_err(|e| {
                        fdo::Error::Failed(format!("Error getting fp mode: {:#?}", e))
                    })?;
                    match fp_mode {
                        FpModeOutput::Enroll => {
                            enrolling_data.images += 1;
                            fp_enroll()
                                .map_err(|e| fdo::Error::Failed(format!("Error enrolling: {:#?}", e)))?;
                            Ok(enrolling_data.to_owned())
                        },
                        FpModeOutput::EnrollPlusImage => Ok(enrolling_data.to_owned()),
                        FpModeOutput::Reset => {
                            // TODO: Handle sensor being reseted from other things such as suspend
                            // This means enrolling is done
                            enrolling_data.images += 1;
                            let info = fp_get_info().map_err(|e| {
                                fdo::Error::Failed(format!("Error getting fp info: {:#?}", e))
                            })?;
                            let template =
                                fp_get_template(info.templates_slots_used - 1).map_err(|e| {
                                    fdo::Error::Failed(format!("Error getting template: {:#?}", e))
                                })?;
                            enrolling_data.template = Some(template).into();
                            let enrolling_data = enrolling_data.to_owned();
                            self.mode = None;
                            Ok(enrolling_data)
                        }
                        _ => Err(fdo::Error::Failed(format!(
                            "Unexpected fp mode: {:#?}",
                            fp_mode
                        ))),
                    }
                } else {
                    Err(fdo::Error::AccessDenied(
                        "A different user is using the fp sensor".into(),
                    ))
                }
            }
            _ => Err(fdo::Error::AccessDenied("Not enrolling".into())),
        }
    }

    // async fn enroll_next_step(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
    //     match &self.enrolling {
    //         Some(enrolling) => {
    //             let user_id = get_user_id(header).await?;
    //             if user_id == enrolling.user_id {
    //                 fp_enroll()
    //                     .map_err(|e| fdo::Error::Failed(format!("Error enrolling: {:#?}", e)))?;
    //                 Ok(())
    //             } else {
    //                 Err(fdo::Error::AccessDenied(
    //                     "A different user is using the sensor".into(),
    //                 ))
    //             }
    //         }
    //         None => Err(fdo::Error::AccessDenied("Not enrolling".into())),
    //     }
    // }
}

// Although we use `async-std` here, you can use any async runtime of choice.
#[async_std::main]
async fn main() -> zbus::Result<()> {
    println!("Main!");
    match fp_hello() {
        Ok(_) => {
            let _connection = Builder::system()?
                .name("org.crosfp.CrosFp")?
                .serve_at("/org/crosfpCrosFp", CrosFp::default())?
                .build()
                .await?;

            loop {
                thread::park();
            }
        }
        Err(e) => {
            println!("Error: {:#?}", e);
            Err(zbus::Error::Failure(format!(
                "Couldn't communicate with fp sensor: {:#?}",
                e
            )))
        }
    }
}
