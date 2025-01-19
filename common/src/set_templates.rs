use std::fmt::{Display, Formatter};
use std::io;

use async_std::fs::{create_dir_all, OpenOptions};
use async_std::io::WriteExt;
use async_std::path::Path;
use rmp_serde::encode;

use crate::template::Templates;

#[derive(Debug)]
pub enum Error {
    Encode(encode::Error),
    CreateDir(io::Error),
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
            Self::CreateDir(e) => {
                write!(f, "Error creating dir: {:#?}", e)
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

pub async fn set_templates(fp_file: impl AsRef<Path>, templates: &Templates) -> Result<(), Error> {
    let vec = encode::to_vec(templates).map_err(Error::Encode)?;
    create_dir_all(fp_file.as_ref().parent().unwrap())
        .await
        .map_err(Error::CreateDir)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(fp_file)
        .await
        .map_err(Error::Open)?;
    file.write(&vec).await.map_err(Error::Write)?;
    Ok(())
}
