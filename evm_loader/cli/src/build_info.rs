use neon_lib::build_info_common::SlimBuildInfo;

build_info::build_info!(fn build_info);

pub fn get_build_info() -> SlimBuildInfo {
    build_info().into()
}
