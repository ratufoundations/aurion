#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    /// Mode Developer: Log detail (DEBUG/TRACE), output terminal ANSI berwarna, file log dev
    #[default]
    Developer,
    /// Mode Production: Terminal bersih, performa I/O maksimal, hanya log error kritis
    Production,
}
