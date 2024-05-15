use std::fmt::{Display, Formatter};
use std::io;

use crate::fp_file;
use async_std::fs::OpenOptions;
use async_std::io::WriteExt;
use rmp_serde::encode;

use crate::fp_file::get_fp_file;
use crate::template::Templates;

#[derive(Debug)]
pub enum Error {
    Encode(encode::Error),
    FpFile(fp_file::Error),
    Open(io::Error),
    Write(io::Error),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(e) => {
                write!(f, "Error encoding file: {:#?}", e)
            }
            Self::FpFile(e) => {
                write!(f, "Error getting fp file: {:#?}", e)
            }
            Self::Open(e) => {
                write!(f, "Error opening file: {:#?}", e)
            }
            Self::Write(e) => {
                write!(f, "Error reading file: {:#?}", e)
            }
        }
    }
}

pub async fn set_templates(templates: &Templates) -> Result<(), Error> {
    let vec = encode::to_vec(templates).map_err(|e| Error::Encode(e))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(get_fp_file().map_err(|e| Error::FpFile(e))?)
        .await
        .map_err(|e| Error::Open(e))?;
    file.write(&vec).await.map_err(|e| Error::Write(e))?;
    Ok(())
}
