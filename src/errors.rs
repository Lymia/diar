use crate::objects::ObjectId;
use std::{
    fmt::{Display, Formatter},
    panic::Location,
};

#[derive(Debug)]
pub struct Error(ErrorContents);
#[derive(thiserror::Error, Debug)]
pub enum ErrorKind {
    #[error("io error encountered at {1}: {0}")]
    IoError(std::io::Error, &'static Location<'static>),
    #[error("encountered while iterating directory at {1}: {0}")]
    JWalkError(jwalk::Error, &'static Location<'static>),
    #[error("encountered while iterating directory at {1}: {0}")]
    FastCDC(fastcdc::v2020::Error, &'static Location<'static>),
}
#[derive(Debug)]
pub enum ErrorContents {
    Kind(Box<ErrorKind>),
    ObjectIdError(ObjectId),
    InternalError(&'static &'static str),
}

impl ErrorContents {
    pub fn emit<T>(mut self) -> Result<T> {
        Err(Error(self))
    }
}

impl From<std::io::Error> for Error {
    #[track_caller]
    fn from(err: std::io::Error) -> Self {
        Error(ErrorContents::Kind(Box::new(ErrorKind::IoError(err, Location::caller()))))
    }
}
impl From<jwalk::Error> for Error {
    #[track_caller]
    fn from(err: jwalk::Error) -> Self {
        Error(ErrorContents::Kind(Box::new(ErrorKind::JWalkError(err, Location::caller()))))
    }
}
impl From<fastcdc::v2020::Error> for Error {
    #[track_caller]
    fn from(err: fastcdc::v2020::Error) -> Self {
        Error(ErrorContents::Kind(Box::new(ErrorKind::FastCDC(err, Location::caller()))))
    }
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            ErrorContents::Kind(k) => Display::fmt(k, f),
            ErrorContents::InternalError(x) => write!(f, "internal error: {x}"),
            ErrorContents::ObjectIdError(id) => {
                write!(f, "invalid object id: {:?} (is it from a different reader/writer?)", id)
            }
        }
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub fn error<T>(str: &'static &'static str) -> Result<T> {
    Err(Error(ErrorContents::InternalError(str)))
}
pub fn ensure(cond: bool, str: &'static &'static str) -> Result<()> {
    if cond {
        Ok(())
    } else {
        error(str)
    }
}
