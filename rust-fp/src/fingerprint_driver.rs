use std::error::Error;
use std::io;
use futures::future::BoxFuture;

pub struct FingerprintDriver {
    /// Returns `true` if this device has a  fingerprint sensor compatible with this driver
    pub is_compatible: Box<dyn Fn() -> BoxFuture<'static, io::Result<bool>>>,
    pub name: &'static str,
    pub open_and_init: Box<dyn Fn() -> BoxFuture<'static, io::Result<Box<dyn OpenedFingerprintDriver>>>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub enum EnrollStepError {
    GenericError,
    LowQuality,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub enum EnrollStepOutput {
    InProgress(u8),
    Complete(Vec<u8>),
}

pub type EnrollStepResult = Result<EnrollStepOutput, EnrollStepError>;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct MatchedOutput {
    pub index: usize,
    /// If the template was updated successfully, the updated template should be outputted
    pub updated_template: Option<Vec<u8>>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NoMatchError {
    Other,
    LowQuality,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MatchOutput {
    Match(MatchedOutput),
    NoMatch(Option<NoMatchError>),
}

pub trait OpenedFingerprintDriver: Sync + Send {
    fn start_or_continue_enroll(&mut self) -> BoxFuture<EnrollStepResult>;
    fn get_max_templates(&mut self) -> Result<usize, ()>;
    fn match_templates<'a>(&'a mut self, templates: &'a Vec<Vec<u8>>) -> BoxFuture<Result<MatchOutput, Box<dyn Error>>>;
}