use derive_more::{Display, From};

#[derive(Debug, Display, From)]
pub enum Error {
    Interpreter(crate::interpreter::Error),
    Cognitive(crate::cognitive::Error),
    Io(std::io::Error),
    Serde(serde_json::Error),
    #[display("CLI error: {_0}")]
    Cli(String),
}
pub type Result<T> = std::result::Result<T, Error>;

impl std::error::Error for Error {}
