use crate::drivers::cros_fp::CrosFp;
use crate::fingerprint_driver::FingerprintDriver;

mod cros_fp;

trait GetFingerprintDriver {
    fn get_driver() -> FingerprintDriver;
}

pub fn get_drivers() -> Vec<FingerprintDriver> {
    vec![CrosFp::get_driver()]
}
