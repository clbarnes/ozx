use serde::{Deserialize, Serialize};

/// e.g.
///
/// ```json
/// {"ome": { "version": "XX.YY", "centralDirectory": { "jsonFirst": true } }}`
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipComment {
    pub ome: ZipOmeMetadata,
}

impl ZipComment {
    pub fn new<S: Into<String>>(version: S, json_first: bool) -> Self {
        Self {
            ome: ZipOmeMetadata {
                version: version.into(),
                zip_file: Some(OmeZipFile::new(json_first)),
            },
        }
    }

    pub fn json_first(&self) -> bool {
        self.ome
            .zip_file
            .as_ref()
            .and_then(|z| z.central_directory.as_ref())
            .and_then(|c| c.json_first)
            .unwrap_or(false)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmeZipFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    central_directory: Option<CentralDirectory>,
}

impl OmeZipFile {
    pub fn new(json_first: bool) -> Self {
        Self {
            central_directory: Some(CentralDirectory {
                json_first: Some(json_first),
            }),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CentralDirectory {
    #[serde(skip_serializing_if = "Option::is_none")]
    json_first: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipOmeMetadata {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_file: Option<OmeZipFile>,
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
