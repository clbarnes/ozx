use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZarrMetadata {
    attributes: Option<Attributes>,
}

impl ZarrMetadata {
    pub fn version(&self) -> Option<&str> {
        self.attributes
            .as_ref()
            .and_then(|attr| attr.ome.as_ref())
            .map(|ome| ome.version.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Attributes {
    ome: Option<Ome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ome {
    version: String,
}
