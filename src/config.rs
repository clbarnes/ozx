use std::collections::BTreeMap;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct Config {
    pub stores: BTreeMap<String, BTreeMap<String, String>>,
}
