use std::fmt::{Display, Formatter};
use std::io;

use async_std::fs::{create_dir_all, OpenOptions};
use async_std::io::WriteExt;
use rmp_serde::encode;

use crate::fp_file;
use crate::fp_file::{get_fp_dir, get_fp_file};
use crate::template::Templates;

#[derive(Debug)]
pub enum Error {
    Encode(encode::Error),
    FpDir(fp_file::Error),
    CreateDir(io::Error),
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
            },
            Self::FpDir(e) => {
                    write!(f, "Error getting fp file: {:#?}", e)
            }
            Self::CreateDir(e) => {
                write!(f, "Error creating dir: {:#?}", e)
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
    create_dir_all(get_fp_dir().map_err(|e| Error::FpDir(e))?).await.map_err(|e| Error::CreateDir(e))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(get_fp_file().map_err(|e| Error::FpFile(e))?)
        .await
        .map_err(|e| Error::Open(e))?;
    file.write(&vec).await.map_err(|e| Error::Write(e))?;
    Ok(())
}
