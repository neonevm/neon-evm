use std::{collections::HashMap, sync::Arc};

use semver::{Version, VersionReq};

pub trait ApiMethodConverter {
    fn rename(&self, method: &str) -> String {
        method.into()
    }

    fn convert_params(&self, params: serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(params)
    }

    fn convert_result(
        &self,
        result: Result<serde_json::Value, String>,
    ) -> Result<serde_json::Value, String> {
        result
    }
}

struct DefaultConverter;

impl ApiMethodConverter for DefaultConverter {}

pub struct MethodConverters(
    HashMap<String, Vec<(VersionReq, Arc<dyn ApiMethodConverter + Send + Sync>)>>,
);

impl MethodConverters {
    pub fn new() -> Self {
        MethodConverters(HashMap::new())
    }

    pub fn register_method_converter(
        &mut self,
        method: &str,
        versions: VersionReq,
        converter: Arc<dyn ApiMethodConverter + Send + Sync>,
    ) {
        self.0
            .entry(method.into())
            .and_modify(|v| v.push((versions.clone(), converter.clone())))
            .or_insert(vec![(versions, converter)]);
    }

    pub fn choose_converter(
        &self,
        method: &str,
        version: Version,
    ) -> Arc<dyn ApiMethodConverter + Send + Sync> {
        match self.0.get(method) {
            None => Arc::new(DefaultConverter),
            Some(entries) => {
                for entry in entries {
                    if entry.0.matches(&version) {
                        return entry.1.clone();
                    }
                }

                Arc::new(DefaultConverter)
            }
        }
    }
}
