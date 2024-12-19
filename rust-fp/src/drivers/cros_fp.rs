use std::{
    collections::HashSet,
    error::Error,
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, ErrorKind},
    time::Duration,
};

use async_std::{fs::File, task::sleep};
use crosec::{
    commands::{
        fp_download::{fp_download_template, FpTemplate},
        fp_get_encryption_status::{fp_get_encryption_status, FpEncryptionStatus},
        fp_info::{fp_info, EcResponseFpInfo},
        fp_mode::{fp_mode, FpMode},
        fp_set_context::fp_set_context,
        fp_set_seed::fp_set_seed,
        fp_upload_template::fp_upload_template,
        get_protocol_info::{get_protocol_info, EcResponseGetProtocolInfo},
        get_uptime_info::ec_cmd_get_uptime_info,
    },
    wait_event::{
        event::{EcMkbpEvent, EcMkbpEventType},
        fingerprint::{
            EcMkbpEventFingerprintEnrollError, EcMkbpEventFingerprintMatchResult,
            EcMkbpEventFingerprintNoMatchError, EcMkbpEventFingerprintRust,
        },
        host_event::HostEventCode,
        wait_event_async,
    },
    CROS_FP_PATH,
};
use futures::future::BoxFuture;

use crate::drivers::GetFingerprintDriver;
use crate::fingerprint_driver::{
    EnrollStepError, EnrollStepOutput, EnrollStepResult, FingerprintDriver, MatchOutput,
    MatchedOutput, NoMatchError, OpenedFingerprintDriver,
};

pub struct CrosFp;

impl GetFingerprintDriver for CrosFp {
    fn get_driver() -> FingerprintDriver {
        FingerprintDriver {
            name: "Chromebook",
            is_compatible: Box::new(|| {
                Box::pin(async {
                    match File::open(CROS_FP_PATH).await {
                        Ok(_file) => Ok(true),
                        Err(e) => match e.kind() {
                            ErrorKind::NotFound => Ok(false),
                            _ => Err(e),
                        },
                    }
                })
            }),
            open_and_init: Box::new(|| {
                Box::pin(async {
                    let opened_cros_fp = OpenedCrosFp::open_and_init().await?;
                    Ok(Box::new(opened_cros_fp) as Box<dyn OpenedFingerprintDriver>)
                })
            }),
        }
    }
}

pub struct OpenedCrosFp {
    file: File,
    protocol_info: EcResponseGetProtocolInfo,
    loaded_templates_hashes: Vec<u64>,
    fp_info: EcResponseFpInfo,
}

impl OpenedCrosFp {
    async fn open_and_init() -> io::Result<Self> {
        let mut file = File::open(CROS_FP_PATH).await?;
        fp_mode(&mut file, FpMode::Reset as u32).unwrap();
        fp_mode(&mut file, FpMode::ResetSensor as u32).unwrap();
        let protocol_info = get_protocol_info(&mut file).unwrap();
        let fp_info = fp_info(&mut file).unwrap();
        Ok(Self {
            file,
            loaded_templates_hashes: Default::default(),
            protocol_info,
            fp_info,
        })
    }

    async fn wait_until_duration_after_fpmcu_boot(&mut self, duration: Duration) {
        let uptime_info = ec_cmd_get_uptime_info(&mut self.file).unwrap();
        if let Some(sleep_duration) = duration.checked_sub(Duration::from_millis(
            uptime_info.time_since_ec_boot_ms.into(),
        )) {
            sleep(sleep_duration).await;
        }
    }

    /// Sets the seed if it hasn't been set. Clears hashes if seed got un-set,
    /// which indicates that the FP sensor restarted, which happens during suspend.
    async fn ensure_seed_is_set(&mut self) {
        // This number was found through trial and error. 1250 failed, but there may be some room for improvement after even more trial and error.
        self.wait_until_duration_after_fpmcu_boot(Duration::from_millis(1350))
            .await;
        let status = fp_get_encryption_status(&mut self.file).unwrap();
        if status.status & (FpEncryptionStatus::SeedSet as u32) == 0 {
            // Set the seed to "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" so that it can easily be typed manually too.
            // I'm pretty sure the tradition of using this seed was made by WeirdTreeThing.
            fp_set_seed(&mut self.file, [b'a'; 32]).unwrap();
            self.loaded_templates_hashes.clear();
        }
    }

    /// Sets the context. The context must be set before enrolling and uploading
    /// The context gets reset when the sensor gets reset
    /// It is possible that the seed doesn't get reset but the context does
    fn set_context(&mut self) {
        fp_set_context(&mut self.file, [0xaa; 32]).unwrap();
        // Setting context always clears templates, even if the context was previously set to the same value
        self.loaded_templates_hashes.clear();
    }

    fn check_if_templates_got_cleared(&mut self) {
        if !self.loaded_templates_hashes.is_empty() {
            let info = fp_info(&mut self.file).unwrap();
            let stored_loaded_templates_count = self.loaded_templates_hashes.len() as u16;
            let actual_loaded_templates_count = info.template_valid;
            if actual_loaded_templates_count == stored_loaded_templates_count {
                // Assume templates have not changed
            } else if info.template_valid == 0 {
                self.loaded_templates_hashes.clear();
            } else {
                // Something is wrong. Panic instead of causing undefined behaviour
                panic!("Expected {stored_loaded_templates_count} or 0 templates to be loaded, but actually {actual_loaded_templates_count} templates are loaded.")
            }
        }
    }
}

impl OpenedFingerprintDriver for OpenedCrosFp {
    fn start_or_continue_enroll(&mut self) -> BoxFuture<EnrollStepResult> {
        Box::pin(async {
            self.ensure_seed_is_set().await;
            self.check_if_templates_got_cleared();
            // Clear templates if there are no more slots left
            if self.loaded_templates_hashes.len() == self.fp_info.template_max as usize {
                // TODO: We may need to set fp mode to Reset before doing this
                self.set_context();
            } else if self.loaded_templates_hashes.is_empty() {
                // Unless we already started enrolling, set the context since it may not be set
                let fp_mode = fp_mode(&mut self.file, FpMode::DontChange as u32).unwrap();
                match FpMode::from_repr(fp_mode) {
                    Some(FpMode::EnrollSession) => {
                        // Don't set context because we assume it is already set and you can't set context while enrolling
                    }
                    Some(FpMode::Reset) => {
                        self.set_context();
                    }
                    _ => {
                        // The fp should not be in any other mode. We can't set context unless it's reset
                        panic!("Unknown fp mode: {fp_mode}")
                    }
                }
            }
            fp_mode(
                &mut self.file,
                FpMode::EnrollSession as u32 | FpMode::EnrollImage as u32,
            )
            .map_err(|_e| EnrollStepError::GenericError)?;
            let data = match wait_event_async(&mut self.file, [EcMkbpEventType::Fingerprint])
                .await
                .map_err(|_e| EnrollStepError::GenericError)?
            {
                EcMkbpEvent::Fingerprint(event) => match event.rust() {
                    EcMkbpEventFingerprintRust::Enroll(output) => Ok(output),
                    _ => Err(EnrollStepError::GenericError),
                },
                _ => Err(EnrollStepError::GenericError),
            }?;
            match data.error {
                None => Ok(match data.percentage {
                    100 => EnrollStepOutput::Complete({
                        let enrolled_template_index = self.loaded_templates_hashes.len();
                        let template = fp_download_template(
                            &mut self.file,
                            &self.fp_info,
                            &self.protocol_info,
                            enrolled_template_index,
                        );
                        let template_vec: Vec<u8> = template.into();
                        let hash = {
                            let mut hasher = DefaultHasher::default();
                            template_vec.hash(&mut hasher);
                            hasher.finish()
                        };
                        self.loaded_templates_hashes.push(hash);
                        template_vec
                    }),
                    percentage => EnrollStepOutput::InProgress(percentage),
                }),
                // TODO: Add more specific errors. I didn't add any because I never experienced them
                Some(error) => Err(match error {
                    EcMkbpEventFingerprintEnrollError::LowQuality => EnrollStepError::LowQuality,
                    _ => EnrollStepError::GenericError,
                }),
            }
        })
    }

    fn get_max_templates(&mut self) -> anyhow::Result<usize> {
        Ok(self.fp_info.template_max as usize)
    }

    fn match_templates<'a>(
        &'a mut self,
        templates: &'a [Vec<u8>],
    ) -> BoxFuture<Result<MatchOutput, Box<dyn Error>>> {
        Box::pin(async move {
            let hashes = templates
                .iter()
                .map(|template| {
                    let mut hasher = DefaultHasher::default();
                    template.hash(&mut hasher);
                    hasher.finish()
                })
                .collect::<Vec<_>>();
            self.check_if_templates_got_cleared();
            // FIXME: Figure out why the uploading is in a loop
            let fingerprint_event = loop {
                self.ensure_seed_is_set().await;
                println!(
                    "Hashes: {hashes:?}. Loaded hashes: {:?}",
                    self.loaded_templates_hashes
                );
                if hashes.iter().collect::<HashSet<_>>()
                    != self.loaded_templates_hashes.iter().collect()
                {
                    fp_mode(&mut self.file, FpMode::Reset as u32)
                        .map_err(|_e| format!("Error doing {:?}", FpMode::Reset))?;
                    self.set_context();
                    // Without waiting a bit, the template uploading can fail.
                    // Maybe 10ms is enough, idk what is the smallest amount that works 99% of the time.
                    sleep(Duration::from_millis(10)).await;
                    for template in templates {
                        fp_upload_template(
                            &mut self.file,
                            &self.protocol_info,
                            &self.fp_info,
                            &unsafe { FpTemplate::from_vec_unchecked(template.to_vec()) },
                        )
                        .map_err(|e| format!("Error uploading template: {e:?}"))?;
                    }
                    self.loaded_templates_hashes.clone_from(&hashes);
                }
                fp_mode(&mut self.file, FpMode::Match as u32)
                    .map_err(|_e| format!("Error doing {:?}", FpMode::Match))?;
                let event = wait_event_async(
                    &mut self.file,
                    [EcMkbpEventType::Fingerprint, EcMkbpEventType::HostEvent],
                )
                .await?;
                match event {
                    EcMkbpEvent::Fingerprint(fingerprint_event) => {
                        break fingerprint_event;
                    }
                    EcMkbpEvent::HostEvent(host_event) => {
                        match host_event.rust().map_err(|unexpected_host_event| {
                            format!("Unexpected host event: {unexpected_host_event}")
                        })? {
                            HostEventCode::InterfaceReady => {}
                            event => unreachable!("Unexpected host event: {event:?}"),
                        }
                    }
                    event => unreachable!("Unknown event: {event:?}"),
                };
            };
            match fingerprint_event.rust() {
                EcMkbpEventFingerprintRust::Match(data) => {
                    Ok(match data {
                        EcMkbpEventFingerprintMatchResult::Match(data) => {
                            MatchOutput::Match(MatchedOutput {
                                index: {
                                    // The order of templates in the sensor may be different from the order of given templates.
                                    // We need to return the index based on the given templates.
                                    let matched_hash = self.loaded_templates_hashes[data.index];
                                    let input_hash_index = hashes
                                        .iter()
                                        .position(|&hash| hash == matched_hash)
                                        // We unwrap because it's impossible for there to be a match for a template that was not inputted
                                        .unwrap();
                                    input_hash_index
                                },
                                updated_template: match data.update {
                                    Some(Ok(_)) => {
                                        let template = fp_download_template(
                                            &mut self.file,
                                            &self.fp_info,
                                            &self.protocol_info,
                                            data.index,
                                        );
                                        let template: Vec<u8> = template.into();
                                        self.loaded_templates_hashes[data.index] = {
                                            let mut hasher = DefaultHasher::default();
                                            template.hash(&mut hasher);
                                            hasher.finish()
                                        };
                                        Some(template)
                                    }
                                    _ => None,
                                },
                            })
                        }
                        EcMkbpEventFingerprintMatchResult::NoMatch(result) => {
                            MatchOutput::NoMatch(match result {
                                Ok(_) => None,
                                Err(error) => Some(match error {
                                    EcMkbpEventFingerprintNoMatchError::LowQuality => {
                                        NoMatchError::LowQuality
                                    }
                                    _ => NoMatchError::Other,
                                }),
                            })
                        }
                    })
                }
                fp_event => Err(format!("Unexpected fp event: {fp_event:?}").into()),
            }
        })
    }
}
