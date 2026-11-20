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
    pub central_directory: Option<CentralDirectory>,
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
    pub json_first: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipOmeMetadata {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_file: Option<OmeZipFile>,
}
