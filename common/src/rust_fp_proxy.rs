use zbus::{fdo, proxy};

#[proxy(
    default_service = "org.rust_fp.RustFp",
    default_path = "/org/rust_fp/RustFp",
    interface = "org.rust_fp.RustFp"
)]
trait RustFp {
    /// Call the org.freedesktop.Notifications.Notify D-Bus method
    fn get_max_templates(&self) -> fdo::Result<u64>;
    fn enroll_step(&self, id: u32) -> fdo::Result<Vec<u8>>;
    fn match_templates(&self, templates: Vec<Vec<u8>>) -> fdo::Result<Vec<u8>>;
}
