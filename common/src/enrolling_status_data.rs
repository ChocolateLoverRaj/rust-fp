use serde::{Deserialize, Serialize};
use zbus::zvariant::{Optional, Type};

#[derive(Default, Clone, Type, Serialize, Deserialize, PartialEq)]
pub struct EnrollingStatusData {
    pub template: Optional<Vec<u8>>,
    pub images: u32,
}
