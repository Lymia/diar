use std::panic::Location;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("io error encountered at {1}: {0}")]
	IoError(Box<std::io::Error>, &'static Location<'static>),
	#[error("encountered while iterating directory at {1}: {0}")]
	JWalkError(Box<jwalk::Error>, &'static Location<'static>),
}
impl From<std::io::Error> for Error {
	#[track_caller]
	fn from(err: std::io::Error) -> Self {
		Error::IoError(Box::new(err), Location::caller())
	}
}
impl From<jwalk::Error> for Error {
	#[track_caller]
	fn from(err: jwalk::Error) -> Self {
		Error::JWalkError(Box::new(err), Location::caller())
	}
}

pub type Result<T> = std::result::Result<T, Error>;
