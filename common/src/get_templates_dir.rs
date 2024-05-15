use ectool_fp::fp_get_info::fp_get_info;

pub fn get_templates_dir(username: &str) -> String {
    let fp_info = fp_get_info().unwrap();
    let fp_id = fp_info.get_unique_string_for_templates();
    let templates_dir = format!("/var/lib/cros-fp/{}/{}", fp_id, username);
    templates_dir
}
