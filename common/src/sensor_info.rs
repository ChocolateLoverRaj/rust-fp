use ectool_fp::fp_get_info::FpInfo;
use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

#[derive(Clone, Copy, Type, Serialize, Deserialize, Debug)]
pub struct SensorInfo {
    /// Max number of templates it can store and match
    pub templates: u32,
}

impl From<FpInfo> for SensorInfo {
    fn from(value: FpInfo) -> Self {
        Self {
            templates: value.templates_slots_total as u32,
        }
    }
}
