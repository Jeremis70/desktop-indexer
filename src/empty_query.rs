use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum EmptyQueryMode {
    #[value(name = "recency")]
    Recency,
    #[value(name = "frequency")]
    Frequency,
}
