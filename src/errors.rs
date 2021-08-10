use std::backtrace::Backtrace;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("io error encountered: {0}")]
	IoError(Box<std::io::Error>, Backtrace),
	#[error("encountered while iterating directory: {0}")]
	JWalkError(Box<jwalk::Error>, Backtrace),
}
impl From<std::io::Error> for Error {
	fn from(err: std::io::Error) -> Self {
		Error::IoError(Box::new(err), Backtrace::capture())
	}
}
impl From<jwalk::Error> for Error {
	fn from(err: jwalk::Error) -> Self {
		Error::JWalkError(Box::new(err), Backtrace::capture())
	}
}

pub type Result<T> = std::result::Result<T, Error>;
