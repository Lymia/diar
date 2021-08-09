#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("io error encountered")]
	IoError(Box<std::io::Error>),
}
impl From<std::io::Error> for Error {
	fn from(err: std::io::Error) -> Self {
		Error::IoError(Box::new(err))
	}
}

pub type Result<T> = std::result::Result<T, Error>;
