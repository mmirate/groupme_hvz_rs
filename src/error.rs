use std;
pub type AnyError = Box<std::error::Error>;
pub type ResultB<T> = Result<T, AnyError>;
