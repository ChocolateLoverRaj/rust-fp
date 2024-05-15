use zbus::{fdo, proxy};
use zbus::zvariant::Optional;

use crate::enrolling_status_data::EnrollingStatusData;
use crate::sensor_info::SensorInfo;

#[proxy(
    default_service = "org.crosfp.CrosFp",
    default_path = "/org/crosfp/CrosFp",
    interface = "org.crosfp.CrosFp"
)]
trait CrosFp {
    /// Call the org.freedesktop.Notifications.Notify D-Bus method
    fn get_fp_info(&self) -> zbus::Result<SensorInfo>;
    fn start_enroll(&self) -> fdo::Result<u32>;
    fn get_enroll_progress(
        &self,
        operation_id: u32,
        wait_for_next: bool,
    ) -> fdo::Result<Optional<EnrollingStatusData>>;
    fn wait_for_operation_start(&self, operation_id: u32) -> fdo::Result<()>;
    fn clear_operation_result(&self, operation_id: u32) -> fdo::Result<()>;
    fn match_finger(&self, templates: Vec<Vec<u8>>) -> fdo::Result<u32>;
    fn get_match_result(
        &self,
        operation_id: u32,
        wait: bool,
    ) -> fdo::Result<Vec<u8>>;
}
