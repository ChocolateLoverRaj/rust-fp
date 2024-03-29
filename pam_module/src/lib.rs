extern crate pam;
extern crate rand;

use std::ffi::CStr;
use std::fs::read_dir;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use common::get_templates_dir::get_templates_dir;
use crossbeam::channel::internal::SelectHandle;
use crossbeam::channel::unbounded;
use crossbeam::select;
use ectool_fp::fp_get_stats::fp_get_stats;
use ectool_fp::fp_load_template::fp_load_template;
use ectool_fp::fp_mode::{fp_get_mode, fp_set_mode, FpModeInput, FpModeOutput};
use ectool_fp::fp_reset::fp_reset;
use ectool_fp::fp_reset_sensor::fp_reset_sensor;
use pam::constants::PamResultCode::{PAM_AUTH_ERR, PAM_IGNORE, PAM_SUCCESS};
use pam::constants::{PamFlag, PamResultCode, PAM_PROMPT_ECHO_OFF, PAM_TEXT_INFO};
use pam::conv::Conv;
use pam::module::{PamHandle, PamHooks};
use pam::pam_try;
use pam_client::conv_mock::Conversation;
use pam_client::{Context, Flag};
use rand::Rng;
use common::seed::SEED;

use crate::fp_seed::fp_seed;

mod fp_seed;

struct PamSober;
pam::pam_hooks!(PamSober);

impl PamHooks for PamSober {
    fn acct_mgmt(_pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        PAM_IGNORE
    }

    // This function performs the task of authenticating the user.
    fn sm_authenticate(pamh: &mut PamHandle, args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        if whoami::username() != "root" {
            // return PAM_AUTH_ERR;
        };


        let single_thread = args
            .get(0)
            .map(|arg| arg.to_str().unwrap() == "single-thread")
            .unwrap_or(false);

        let conv = match pamh.get_item::<Conv>() {
            Ok(Some(conv)) => conv,
            Ok(None) => todo!(),
            Err(err) => {
                println!("Couldn't get pam_conv");
                return err;
            }
        };

        if single_thread && false {
            let timeout = Duration::from_secs(25);
            pam_try!(conv.send(PAM_TEXT_INFO, &format!("Since KDE has a skill issue, you get {:#?} to unlock with fingerprint. After that, you will have to type your password.", timeout)));
            let username = pam_try!(pamh.get_user(None));
            let start_instant = Instant::now();
            let _ = fp_seed(SEED);
            fp_reset().unwrap();
            fp_reset_sensor().unwrap();
            // Running another command after resetting the sensor too early will result in corrupted data. Specifically, a corrupted model string.
            thread::sleep(Duration::from_millis(500));
            let templates_dir = get_templates_dir(&username);
            for file in pam_try!(read_dir(templates_dir).map_err(|e| PAM_AUTH_ERR)) {
                let file = file.unwrap();
                fp_load_template(file.path().to_str().unwrap()).unwrap();
                println!("Added template: {:#?}", file.path());
            }
            fp_set_mode(FpModeInput::Match).unwrap();
            pam_try!(conv.send(PAM_TEXT_INFO, "Fingerprint sensor ready"));
            let max_attempts = 5;
            let mut attempt = 0;
            loop {
                let now = Instant::now();
                if now - start_instant >= timeout {
                    println!("Timeout");
                    return PAM_AUTH_ERR;
                }
                let fp_mode = fp_get_mode().unwrap();
                if fp_mode == FpModeOutput::Reset {
                    let stats = fp_get_stats().unwrap();
                    println!("Stats: {:#?}", stats);
                     match stats.last_matching_finger {
                        Some(_) => return PAM_SUCCESS,
                        None => {
                            attempt += 1;
                            if attempt == max_attempts {
                                return PAM_AUTH_ERR
                            } else {
                                pam_try!(conv.send(PAM_TEXT_INFO, "Invalid fingerprint"));
                                thread::sleep(Duration::from_millis(500));
                                fp_set_mode(FpModeInput::Match).unwrap();
                            }
                        },
                    };
                }
            }
        } else {
            let (text_tx, text_rx) = unbounded::<bool>();
            let user = pamh.get_user(None).unwrap();

            let thread = thread::spawn(move || {
                let binding = conv.lock().unwrap();

                let result = binding.conv.send(
                    PAM_PROMPT_ECHO_OFF,
                    format!("[cros-fp]: password for {}: ", user).as_str(),
                );

                match result {
                    Ok(option) => {
                        let password = option.unwrap().to_str().unwrap().to_owned();
                        if password.is_empty() {
                            return;
                        }

                        let mut context = Context::new(
                            "login",
                            Some(user.as_str()),
                            Conversation::with_credentials(user.clone(), password),
                        )
                            .expect("Failed to initialize PAM context");

                        // Authenticate the user
                        if context.authenticate(Flag::NONE).is_err() {
                            binding
                                .conv
                                .send(PAM_TEXT_INFO, "Incorrect Password")
                                .unwrap();
                            text_tx.send(false).unwrap();
                            return;
                        }

                        text_tx.send(true).unwrap();
                    }
                    _ => {}
                }
            });

            let (fp_tx, fp_rx) = unbounded::<bool>();
            let _ = thread::spawn(move || {
                thread::sleep(Duration::from_secs(5));
                match fp_tx.send(true) {
                    Ok(_) => {
                        // Message sent successfully
                    }
                    Err(_) => {
                        // This may be because the authentication function already ended. No need to panic.
                    }
                };
            });

            let result = select! {
            recv(text_rx) -> v => v,
            recv(fp_rx) -> v => v
        }
                .unwrap_or_else(|e| fp_rx.recv().unwrap());
            match result {
                true => PAM_SUCCESS,
                false => PAM_AUTH_ERR,
            }
        }
    }

    fn sm_setcred(_pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        PAM_IGNORE
    }
}
