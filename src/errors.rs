#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("io error encountered: {0}")]
	IoError(Box<std::io::Error>),
	#[error("encountered while iterating directory: {0}")]
	JWalkError(Box<jwalk::Error>),
}
impl From<std::io::Error> for Error {
	fn from(err: std::io::Error) -> Self {
		Error::IoError(Box::new(err))
	}
}
impl From<jwalk::Error> for Error {
	fn from(err: jwalk::Error) -> Self {
		Error::JWalkError(Box::new(err))
	}
}

pub type Result<T> = std::result::Result<T, Error>;
