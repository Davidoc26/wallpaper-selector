use std::fmt::{Debug, Display, Formatter};
use std::io;

use crate::core::desktop::gnome::Gnome;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    NotImplemented(String),
    Io(io::Error),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl Error {
    fn message(&self) -> String {
        match self {
            Error::NotImplemented(s) => format!("Not implemented for {}", s),
            Error::Io(err) => err.to_string(),
        }
    }
}

pub trait DesktopEnvironment {
    fn set_wallpaper(&self, path: &str) -> Result<()>;
}

impl Default for Box<dyn DesktopEnvironment + Send> {
    fn default() -> Self {
        Box::new(Gnome {})
    }
}

pub fn initialize(desktop: &str) -> Result<Box<dyn DesktopEnvironment + Send>> {
    match desktop.to_lowercase().as_str() {
        "gnome" => Ok(Box::new(Gnome {})),
        some => Err(Error::NotImplemented(some.to_string())),
    }
}

mod gnome {
    use std::process::Command;

    use super::{DesktopEnvironment, Result};

    pub struct Gnome {}

    impl DesktopEnvironment for Gnome {
        fn set_wallpaper(&self, path: &str) -> Result<()> {
            Command::new("flatpak-spawn")
                .args([
                    "--host",
                    "gsettings",
                    "set",
                    "org.gnome.desktop.background",
                    "picture-uri",
                    format!("file://{}", path).as_str(),
                ])
                .output()?;

            Command::new("flatpak-spawn")
                .args([
                    "--host",
                    "gsettings",
                    "set",
                    "org.gnome.desktop.background",
                    "picture-uri-dark",
                    format!("file://{}", path).as_str(),
                ])
                .output()?;

            Ok(())
        }
    }
}
