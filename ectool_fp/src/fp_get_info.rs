use std::io;
use std::process::Command;

// TODO: Add more info if needed
#[derive(Debug, Clone)]
pub struct FpInfo {
    pub vendor: String,
    pub product: String,
    pub model: String,
    pub version: usize,
    pub image_size: (usize, usize),
    pub bpp: usize,
    pub templates_version: usize,
    pub template_size: usize,
    pub templates_slots_used: usize,
    pub templates_slots_total: usize,
}
impl FpInfo {
    /// Create a unique string based off of product info
    pub fn get_unique_string_for_templates(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.vendor, self.product, self.model, self.version, self.templates_version
        )
    }
}

pub fn fp_get_info() -> Result<FpInfo, io::Error> {
    let output = Command::new("sudo")
        .arg("ectool")
        .arg("--name=cros_fp")
        .arg("fpinfo")
        .output()?
        .stdout;
    let output_string = String::from_utf8(output).unwrap();
    let output = output_string.lines().collect::<Vec<_>>();

    let parts = output[0].split_whitespace().collect::<Vec<_>>();
    let vendor = parts[3].to_owned();
    let product = parts[5].to_owned();
    let model = parts[7].to_owned();
    let version = parts[9].parse::<usize>().unwrap();

    let parts = output[1].split_whitespace().collect::<Vec<_>>();
    let size = parts[2]
        .split("x")
        .map(|n| n.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    let image_size = (size[0], size[1]);
    let bpp = parts[3].parse::<usize>().unwrap();

    let parts = output[4].split_ascii_whitespace().collect::<Vec<_>>();
    let templates_version = parts[2].parse::<usize>().unwrap();
    let template_size = parts[4].parse::<usize>().unwrap();
    let count_info = parts[6]
        .split("/")
        .map(|n| n.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    let templates_slots_used = count_info[0];
    let templates_slots_total = count_info[1];

    Ok(FpInfo {
        vendor,
        image_size,
        bpp,
        templates_version,
        template_size,
        templates_slots_used,
        model,
        product,
        version,
        templates_slots_total,
    })
}
