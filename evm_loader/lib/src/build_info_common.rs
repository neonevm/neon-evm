use build_info::chrono::{DateTime, Utc};
use build_info::semver::Version;
use build_info::VersionControl::Git;
use build_info::{BuildInfo, OptimizationLevel};
use serde::Serialize;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Serialize)]
pub struct SlimBuildInfo {
    timestamp: DateTime<Utc>,
    profile: String,
    optimization_level: OptimizationLevel,
    crate_info: CrateInfo,
    compiler: CompilerInfo,
    version_control: GitInfo,
}

#[derive(Debug, Clone, Serialize)]
struct CrateInfo {
    name: String,
    version: Version,
}

#[derive(Debug, Clone, Serialize)]
struct CompilerInfo {
    version: Version,
}

#[derive(Debug, Clone, Serialize)]
struct GitInfo {
    commit_id: String,
    dirty: bool,
    branch: Option<String>,
    tags: Vec<String>,
}

impl From<&BuildInfo> for SlimBuildInfo {
    fn from(build_info: &BuildInfo) -> Self {
        let build_info = build_info.clone();

        let crate_info = build_info.crate_info;

        let Git(git_info) = build_info
            .version_control
            .expect("Project should be built inside version control");

        SlimBuildInfo {
            timestamp: build_info.timestamp,
            profile: build_info.profile,
            optimization_level: build_info.optimization_level,
            crate_info: CrateInfo {
                name: crate_info.name,
                version: crate_info.version,
            },
            compiler: CompilerInfo {
                version: build_info.compiler.version,
            },
            version_control: GitInfo {
                commit_id: git_info.commit_id,
                dirty: git_info.dirty,
                branch: git_info.branch,
                tags: git_info.tags,
            },
        }
    }
}

impl Display for SlimBuildInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BuildInfo={}",
            serde_json::to_string(&self).expect("Serialization should not fail")
        )
    }
}
