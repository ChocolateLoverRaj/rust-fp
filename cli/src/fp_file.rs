use home::home_dir;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
    HomeDir,
    PathBufToStr,
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HomeDir => {
                write!(f, "Error getting home dir")
            }
            Self::PathBufToStr => {
                write!(f, "Couldn't convert path buf to str")
            }
        }
    }
}

pub fn get_fp_dir() -> Result<String, Error> {
    Ok(format!(
        "{}/.var",
        home_dir()
            .ok_or(Error::HomeDir)?
            .to_str()
            .ok_or(Error::PathBufToStr)?
    ))
}

pub fn get_fp_file() -> Result<String, Error> {
    Ok(format!(
        "{}/cros-fp-templates",
        get_fp_dir()?
    ))
}
