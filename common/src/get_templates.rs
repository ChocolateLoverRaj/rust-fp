use std::fmt::{Display, Formatter};
use std::io;
use std::io::ErrorKind;

use crate::fp_file;
use async_std::fs::OpenOptions;
use async_std::io::ReadExt;
use rmp_serde::decode;

use crate::fp_file::get_fp_file;
use crate::template::Templates;

#[derive(Debug)]
pub enum Error {
    Open(io::Error),
    FpFile(fp_file::Error),
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
            Self::FpFile(e) => {
                write!(f, "Error getting fp file: {:#?}", e)
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

pub async fn get_templates() -> Result<Templates, Error> {
    match OpenOptions::new()
        .read(true)
        .open(get_fp_file().map_err( Error::FpFile)?)
        .await
    {
        Ok(mut file) => {
            let mut buf = Default::default();
            file.read_to_end(&mut buf)
                .await
                .map_err(Error::Read)?;
            let templates =
                rmp_serde::from_slice::<Templates>(&buf).map_err( Error::Decode)?;
            Ok(templates)
        }
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Ok(Default::default()),
            _ => Err(Error::Open(e)),
        },
    }
}
