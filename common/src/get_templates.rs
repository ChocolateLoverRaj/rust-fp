use std::fmt::{Display, Formatter};
use std::io;
use std::io::ErrorKind;

use async_std::fs::OpenOptions;
use async_std::io::ReadExt;
use async_std::path::Path;
use rmp_serde::decode;

use crate::template::Templates;

#[derive(Debug)]
pub enum Error {
    Open(io::Error),
    Read(io::Error),
    Decode(decode::Error),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open(e) => {
                write!(f, "Error opening file: {:#?}", e)
            }
            Self::Read(e) => {
                write!(f, "Error reading file: {:#?}", e)
            }
            Self::Decode(e) => {
                write!(f, "Error decoding file: {:#?}", e)
            }
        }
    }
}

pub async fn get_templates(fp_file: impl AsRef<Path>) -> Result<Templates, Error> {
    match OpenOptions::new().read(true).open(fp_file).await {
        Ok(mut file) => {
            let mut buf = Default::default();
            file.read_to_end(&mut buf).await.map_err(Error::Read)?;
            let templates = rmp_serde::from_slice::<Templates>(&buf).map_err(Error::Decode)?;
            Ok(templates)
        }
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Ok(Default::default()),
            _ => Err(Error::Open(e)),
        },
    }
}
