extern crate pam;
extern crate rand;

use std::ffi::CStr;

use pam::constants::{PAM_ERROR_MSG, PamFlag, PamResultCode};
use pam::constants::PamResultCode::{PAM_AUTH_ERR, PAM_SUCCESS};
use pam::conv::Conv;
use pam::module::{PamHandle, PamHooks};
use pam::pam_try;
use pollster::block_on;
use postcard::from_bytes;
use zbus::blocking::Connection;

use rust_fp_common::get_templates::get_templates;
use rust_fp_common::rust_fp_proxy::RustFpProxyBlocking;
use rust_fp_common::set_templates::set_templates;
use rust_fp::fingerprint_driver::{MatchedOutput, MatchOutput};

struct PamSober;
pam::pam_hooks!(PamSober);

impl PamHooks for PamSober {
    // This function performs the task of authenticating the user.
    fn sm_authenticate(pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        let conv = match pamh.get_item::<Conv>() {
            Ok(Some(conv)) => conv,
            Ok(None) => todo!(),
            Err(err) => {
                println!("Couldn't get pam_conv");
                return err;
            }
        };
        let mut templates = block_on(get_templates()).unwrap();
        if templates.len() > 0 {
            let connection = Connection::system().unwrap();
            let proxy = RustFpProxyBlocking::new(&connection).unwrap();
            let templates_vec = templates.iter().collect::<Vec<_>>();
            let max_attempts = 5;
            for attempt in 0..max_attempts {
                let output: MatchOutput = from_bytes(&proxy.match_templates(templates_vec.iter().map::<Vec<u8>, _>(|(_k, v)| v.to_vec()).collect()).unwrap()).unwrap();
                match output {
                    MatchOutput::Match(MatchedOutput { index, updated_template }) => {
                        let matched_label = templates_vec[index].0;
                        println!("Matched: {matched_label}.");
                        if let Some(updated_template) = updated_template {
                            println!("Template was updated. Saving updated template...");
                            templates.insert(matched_label.to_owned(), updated_template);
                            block_on(set_templates(&templates)).unwrap();
                            println!("Saved updated template");
                        }
                        return PAM_SUCCESS
                    }
                    MatchOutput::NoMatch(error) => {
                        let remaining_attempts = max_attempts - attempt - 1;
                        pam_try!(conv.send(PAM_ERROR_MSG, &format!("No match. {remaining_attempts} attempts remaining.")));
                        if let Some(error) = error {
                            pam_try!(conv.send(PAM_ERROR_MSG, &format!("Error matching: {error:?}")));
                        }
                    }
                }
            }
            PAM_AUTH_ERR
        } else {
            pam_try!(conv.send(PAM_ERROR_MSG, "No templates saved. Not matching."));
            PAM_AUTH_ERR
        }
    }
}
