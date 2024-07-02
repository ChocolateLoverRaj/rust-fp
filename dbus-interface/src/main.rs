#![warn(unused_crate_dependencies)]

use log::info;
use rust_fp::drivers::get_drivers;
use rust_fp_common::rust_fp_dbus::RustFp;
use simple_logger::SimpleLogger;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::{io, thread};
use zbus::export::futures_util::future::join_all;
use zbus::connection::Builder;

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
