use serde::{Deserialize, Serialize};
use super::Buffer;

/// FunctionMetadata is an EOF function signature.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionMetadata {
    pub input: u8,
    pub output: u8,
    pub max_stack_height: u16,
}

/// Container is an EOF container object.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Container {
    pub types: Vec<FunctionMetadata>,
    pub code: Vec<Buffer>,
    pub data: Buffer,
}
