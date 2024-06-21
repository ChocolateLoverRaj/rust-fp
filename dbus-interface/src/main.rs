use log::{error, info};
use postcard::to_allocvec;
use rand::random;
use rust_fp::drivers::get_drivers;
use rust_fp::fingerprint_driver::{EnrollStepError, EnrollStepOutput, OpenedFingerprintDriver};
use simple_logger::SimpleLogger;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::{io, thread};
use zbus::export::futures_util::future::join_all;
use zbus::{connection::Builder, fdo, interface};
use rust_fp_common::enroll_step_dbus_output::EnrollStepDbusOutput;

struct RustFp {
    driver: Box<dyn OpenedFingerprintDriver>,
    enrolling_id: Option<u32>,
}

#[interface(name = "org.rust_fp.RustFp")]
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

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().init().unwrap();
    info!("Checking compatible fingerprint drivers");
    let drivers = get_drivers();
    let compatibilities = join_all(
        drivers
            .iter()
            .map(|driver| Box::pin((driver.is_compatible)())),
    )
    .await;
    let (oks, errors) = {
        let mut oks = vec![];
        let mut errors = vec![];
        for (index, result) in compatibilities.into_iter().enumerate() {
            match result {
                Ok(value) => {
                    oks.push((index, value));
                }
                Err(e) => {
                    errors.push((index, e));
                }
            }
        }
        (oks, errors)
    };
    if !errors.is_empty() {
        #[derive(Debug)]
        struct ErrorCheckingCompatibleDrivers {
            errors: HashMap<&'static str, io::Error>,
        }

        impl Display for ErrorCheckingCompatibleDrivers {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "Error checking compatible drivers: {:#?}", self.errors)
            }
        }
        impl Error for ErrorCheckingCompatibleDrivers {}

        return Err(ErrorCheckingCompatibleDrivers {
            errors: errors
                .into_iter()
                .map(|(index, error)| (drivers[index].name, error))
                .collect(),
        }
        .into());
    }

    enum CompatibleDrivers {
        None,
        One(usize),
        Multiple(Vec<usize>),
    }
    let compatible_drivers = {
        let compatible_drivers = oks
            .into_iter()
            .filter_map(|(index, compatible)| match compatible {
                true => Some(index),
                false => None,
            })
            .collect::<Vec<_>>();
        match compatible_drivers.len() {
            0 => CompatibleDrivers::None,
            1 => CompatibleDrivers::One(compatible_drivers[0]),
            2.. => CompatibleDrivers::Multiple(compatible_drivers),
        }
    };
    let driver = match compatible_drivers {
        CompatibleDrivers::None => Err("No compatible drivers"),
        CompatibleDrivers::One(driver) => Ok(&drivers[driver]),
        CompatibleDrivers::Multiple(_drivers) => Err("Too many compatible drivers"),
    }?;
    info!("Compatible driver found: {}", driver.name);
    info!("Opening driver.");
    let driver = (driver.open_and_init)().await?;
    info!("Opened driver.");
    info!("Starting dbus interface");
    let _connection = Builder::system()?
        .name("org.rust_fp.RustFp")?
        .serve_at(
            "/org/rust_fp/RustFp",
            RustFp {
                driver,
                enrolling_id: Default::default(),
            },
        )?
        .build()
        .await?;

    loop {
        thread::park();
    }
}
