#![warn(clippy::all)]

pub mod filters;
mod process;
#[cfg(feature = "json")]
pub mod query;
pub mod readers;
#[cfg(feature = "web")]
pub mod web;
#[cfg(test)]
mod tests;

pub use process::{FilteredLogIterator, process};

#[cfg_attr(feature = "json", derive(serde_derive::Serialize))]
#[derive(Clone)]
pub enum Color {
    #[cfg_attr(feature = "json", serde(rename = "default"))]
    Default,
    #[cfg_attr(feature = "json", serde(rename = "fixed"))]
    Fixed {
        color: String,
    },
    #[cfg_attr(feature = "json", serde(rename = "fromValue"))]
    FromValue {
        value: String,
    },
}

#[cfg_attr(feature = "json", derive(serde_derive::Serialize))]
#[derive(Clone)]
pub struct Record {
    pub text: String,
    pub variables: Vec<(String, String)>,
    pub color: Color,
}

impl Record {
    pub fn new(text: String) -> Record {
        Record {
            text,
            variables: Vec::new(),
            color: Color::Default,
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}
