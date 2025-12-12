use serde::{Deserialize, Serialize};

/// {"ome": { "version": "XX.YY" }}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipComment {
    pub ome: ZipOmeMetadata,
}

impl ZipComment {
    pub fn new<S: Into<String>>(version: S, json_first: bool) -> Self {
        Self {
            ome: ZipOmeMetadata {
                version: version.into(),
                json_first,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipOmeMetadata {
    pub version: String,
    pub json_first: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Array,
    Group,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZarrV3Metadata {
    pub node_type: NodeType,
    #[serde(default)]
    pub attributes: ZarrAttributes,
}

impl ZarrV3Metadata {
    pub fn is_array(&self) -> bool {
        matches!(self.node_type, NodeType::Array)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ZarrAttributes {
    pub ome: Option<OmeMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmeMetadata {
    pub version: String,
}
