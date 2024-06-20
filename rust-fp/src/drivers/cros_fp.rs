use std::collections::HashSet;
use std::error::Error;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::io::ErrorKind;
use std::time::Duration;

use async_std::fs::File;
use async_std::task::sleep;
use crosec::commands::fp_download::{fp_download_template, FpTemplate};
use crosec::commands::fp_get_encryption_status::{fp_get_encryption_status, FpEncryptionStatus};
use crosec::commands::fp_info::{EcResponseFpInfo, fp_info};
use crosec::commands::fp_mode::{fp_mode, FpMode};
use crosec::commands::fp_set_seed::fp_set_seed;
use crosec::commands::fp_upload_template::fp_upload_template;
use crosec::commands::get_protocol_info::{EcResponseGetProtocolInfo, get_protocol_info};
use crosec::CROS_FP_PATH;
use crosec::wait_event::event::{EcMkbpEvent, EcMkbpEventType};
use crosec::wait_event::fingerprint::{EcMkbpEventFingerprintEnrollError, EcMkbpEventFingerprintMatchResult, EcMkbpEventFingerprintNoMatchError, EcMkbpEventFingerprintRust};
use crosec::wait_event::wait_event_async;
use futures::future::BoxFuture;

use crate::drivers::GetFingerprintDriver;
use crate::fingerprint_driver::{EnrollStepError, EnrollStepOutput, EnrollStepResult, FingerprintDriver, MatchedOutput, MatchOutput, NoMatchError, OpenedFingerprintDriver};

pub struct CrosFp;

impl GetFingerprintDriver for CrosFp {
    fn get_driver() -> FingerprintDriver {
        FingerprintDriver {
            name: "Chromebook",
            is_compatible: Box::new(|| Box::pin(async {
                match File::open(CROS_FP_PATH).await {
                    Ok(_file) => {
                        Ok(true)
                    }
                    Err(e) => {
                        match e.kind() {
                            ErrorKind::NotFound => Ok(false),
                            _ => Err(e)
                        }
                    }
                }
            })),
            open_and_init: Box::new(|| Box::pin(async {
                let opened_cros_fp = OpenedCrosFp::open_and_init().await?;
                Ok(Box::new(opened_cros_fp) as Box::<dyn OpenedFingerprintDriver>)
            })),
        }
    }
}

pub struct OpenedCrosFp {
    file: File,
    protocol_info: EcResponseGetProtocolInfo,
    loaded_templates_hashes: Vec<u64>,
    fp_info: EcResponseFpInfo
}

impl OpenedCrosFp {
    async fn open_and_init() -> io::Result<Self> {
        let mut file = File::open(CROS_FP_PATH).await?;
        fp_mode(&mut file, FpMode::Reset as u32).unwrap();
        fp_mode(&mut file, FpMode::ResetSensor as u32).unwrap();
        let protocol_info = get_protocol_info(&mut file).unwrap();
        let fp_info = fp_info(&mut file).unwrap();
        Ok(Self { file, loaded_templates_hashes: Default::default(), protocol_info, fp_info })
    }

    /// Sets the seed if it hasn't been set. Clears hashes if seed got un-set,
    /// which indicates that the FP sensor restarted, which happens during suspend.
    fn ensure_seed_is_set(&mut self) {
        let status = fp_get_encryption_status(&mut self.file).unwrap();
        if status.status & (FpEncryptionStatus::SeedSet as u32) == 0 {
            // Set the seed to "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" so that it can easily be typed manually too.
            // I'm pretty sure the tradition of using this seed was made by WeirdTreeThing.
            fp_set_seed(&mut self.file, [b'a'; 32]).unwrap();
            self.loaded_templates_hashes.clear();
        }
    }
}

impl OpenedFingerprintDriver for OpenedCrosFp {
    fn start_or_continue_enroll(&mut self) -> BoxFuture<EnrollStepResult> {
        Box::pin(async {
            self.ensure_seed_is_set();
            // Clear templates if there are no more slots left
            if self.loaded_templates_hashes.len() == self.fp_info.template_max as usize {
                fp_mode(&mut self.file, FpMode::ResetSensor as u32).map_err(|_e| EnrollStepError::GenericError)?;
                self.loaded_templates_hashes.clear();
            }
            // if self.loaded_templates_hashes.len() > 0 {
            //     fp_mode(&mut self.file, FpMode::ResetSensor as u32).map_err(|_e| EnrollStepError::GenericError)?;
            //     self.loaded_templates_hashes.clear();
            // }
            fp_mode(&mut self.file, FpMode::EnrollSession as u32 | FpMode::EnrollImage as u32).map_err(|_e| EnrollStepError::GenericError)?;
            let data = match wait_event_async(&mut self.file, EcMkbpEventType::Fingerprint).await.map_err(|_e| EnrollStepError::GenericError)? {
                EcMkbpEvent::Fingerprint(event) => {
                    match event.rust() {
                        EcMkbpEventFingerprintRust::Enroll(output) => {
                            Ok(output)
                        }
                        _ => Err(EnrollStepError::GenericError)
                    }
                }
                _ => Err(EnrollStepError::GenericError)
            }?;
            match data.error {
                None => Ok(match data.percentage {
                    100 => EnrollStepOutput::Complete({
                        let enrolled_template_index = self.loaded_templates_hashes.len();
                        let template = fp_download_template(&mut self.file, &self.fp_info, &self.protocol_info, enrolled_template_index);
                        let template_vec: Vec<u8> = template.into();
                        let hash = {
                            let mut hasher = DefaultHasher::default();
                            template_vec.hash(&mut hasher);
                            let hash = hasher.finish();
                            hash
                        };
                        self.loaded_templates_hashes.push(hash);
                        template_vec
                    }),
                    percentage => EnrollStepOutput::InProgress(percentage)
                }),
                // TODO: Add more specific errors. I didn't add any because I never experienced them
                Some(error) => Err(match error {
                    EcMkbpEventFingerprintEnrollError::LowQuality => EnrollStepError::LowQuality,
                    _ => EnrollStepError::GenericError
                })
            }
        })
    }

    fn get_max_templates(&mut self) -> Result<usize, ()> {
        Ok(self.fp_info.template_max as usize)
    }

    fn match_templates<'a>(&'a mut self, templates: &'a Vec<Vec<u8>>) -> BoxFuture<Result<MatchOutput, Box<dyn Error>>> {
        Box::pin(async move {
            self.ensure_seed_is_set();
            let hashes = templates.iter().map(|template| {
                let mut hasher = DefaultHasher::default();
                template.hash(&mut hasher);
                let hash = hasher.finish();
                hash
            }).collect::<Vec<_>>();
            println!("Hashes: {hashes:?}. Loaded hashes: {:?}", self.loaded_templates_hashes);
            if hashes.iter().collect::<HashSet<_>>() != self.loaded_templates_hashes.iter().collect() {
                fp_mode(&mut self.file, FpMode::Reset as u32).map_err(|_e| format!("Error doing {:?}", FpMode::Reset))?;
                fp_mode(&mut self.file, FpMode::ResetSensor as u32).map_err(|_e| format!("Error doing {:?}", FpMode::ResetSensor))?;
                // Without waiting a bit, the template uploading can fail.
                // Maybe 10ms is enough, idk what is the smallest amount that works 99% of the time.
                sleep(Duration::from_millis(10)).await;
                for template in templates {
                    fp_upload_template(
                        &mut self.file,
                        &self.protocol_info,
                        &self.fp_info,
                        &unsafe { FpTemplate::from_vec_unchecked(template.to_vec()) }
                    ).map_err(|e| format!("Error uploading template: {e:?}"))?;
                }
                self.loaded_templates_hashes = hashes.clone();
            }
            fp_mode(&mut self.file, FpMode::Match as u32).map_err(|_e| format!("Error doing {:?}", FpMode::Match))?;
            let event = wait_event_async(&mut self.file, EcMkbpEventType::Fingerprint).await.map_err(|e| format!("Error waiting for event: {e:?}"))?;
            match event {
                EcMkbpEvent::Fingerprint(data) => {
                    match data.rust() {
                        EcMkbpEventFingerprintRust::Match(data) => {
                            Ok(match data {
                                EcMkbpEventFingerprintMatchResult::Match(data) => {
                                    MatchOutput::Match(MatchedOutput {
                                        index: {
                                            // The order of templates in the sensor may be different from the order of given templates.
                                            // We need to return the index based on the given templates.
                                            let matched_hash =  self.loaded_templates_hashes[data.index];
                                            let input_hash_index = hashes.iter().position(|&hash| hash == matched_hash)
                                                // We unwrap because it's impossible for there to be a match for a template that was not inputted
                                                .unwrap();
                                            input_hash_index
                                        },
                                        updated_template: match data.update {
                                            Some(Ok(_)) => {
                                                let template = fp_download_template(&mut self.file, &self.fp_info, &self.protocol_info, data.index);
                                                let template: Vec<u8> = template.into();
                                                self.loaded_templates_hashes[data.index] = {
                                                    let mut hasher = DefaultHasher::default();
                                                    template.hash(&mut hasher);
                                                    let hash = hasher.finish();
                                                    hash
                                                };
                                                Some(template.into())
                                            },
                                            _ => None
                                        },
                                    })
                                }
                                EcMkbpEventFingerprintMatchResult::NoMatch(result) => {
                                    MatchOutput::NoMatch(match result {
                                        Ok(_) => None,
                                        Err(error) => Some(match error {
                                            EcMkbpEventFingerprintNoMatchError::LowQuality => NoMatchError::LowQuality,
                                            _ => NoMatchError::Other
                                        })
                                    })
                                }
                            })
                        }
                        fp_event => Err(format!("Unexpected fp event: {fp_event:?}").into())
                    }
                }
                event => Err(format!("Unexpected event: {event:?}").into())
            }
        })
    }
}
