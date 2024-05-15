use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::{io, thread};

use async_std::fs::{remove_file, File, OpenOptions};
use async_std::sync::{Mutex, RwLock};
use async_std::task;
use common::enrolling_status_data::EnrollingStatusData;
use event_listener::Event;
use mktemp::Temp;
use serde::{Deserialize, Serialize};
use zbus::zvariant::{Optional, Type};
use zbus::{connection::Builder, fdo, interface, message::Header};
use zbus::export::futures_util::AsyncWriteExt;

use common::seed::SEED;
use common::sensor_info::SensorInfo;
use ectool_fp::fp_enroll::fp_enroll;
use ectool_fp::fp_get_info::fp_get_info;
use ectool_fp::fp_get_stats::fp_get_stats;
use ectool_fp::fp_get_template::fp_get_template;
use ectool_fp::fp_hello::fp_hello;
use ectool_fp::fp_load_template::fp_load_template;
use ectool_fp::fp_mode::{fp_get_mode, fp_set_mode, FpModeInput, FpModeOutput};
use ectool_fp::fp_reset::fp_reset;
use ectool_fp::fp_reset_sensor::fp_reset_sensor;
use ectool_fp::fp_seed::fp_seed;

use crate::get_user_id::get_user_id;

mod get_user_id;

#[derive(Default, Clone, Type, Serialize, Deserialize)]
struct EnrollingData {
    user_id: u32,
    template: Optional<Vec<u8>>,
    images: u32,
}

#[derive(Clone, Eq, PartialEq, Debug)]
enum Operation {
    Enroll,
    Match(Vec<Vec<u8>>),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct OperationUniqueId {
    user_id: u32,
    operation_id: u32,
}

#[derive(Clone, Eq, PartialEq, Debug)]
struct OperationWithId {
    id: OperationUniqueId,
    operation: Operation,
}

#[derive(Default)]
struct EnrollingStatus {
    data: RwLock<EnrollingStatusData>,
    event: Event,
}

#[derive(Default)]
struct MatchingStatus {
    result: RwLock<Option<Option<u32>>>,
    event: Event,
}

enum OperationStatus {
    Enrolling(Arc<EnrollingStatus>),
    Matching(Arc<MatchingStatus>),
}

#[derive(Clone)]
struct SensorState {
    /// The user whose fingerprints are loaded into the sensor
    active_user: Option<u32>,
    /// The templates
    templates: Vec<Vec<u8>>,
}

#[derive(Default)]
struct CrosFp {
    operation_status: Arc<RwLock<HashMap<OperationUniqueId, OperationStatus>>>,
    operation_ids: Mutex<HashMap<u32, u32>>,
    queue: Arc<RwLock<VecDeque<OperationWithId>>>,
    queue_emitter: Arc<Event>,
    /// None means that it's in an unknown state. State can change from being known to unknown
    sensor_state: Arc<RwLock<Option<SensorState>>>,
    /// Info about the sensor that doesn't change until reboot
    sensor_info: Arc<RwLock<Option<SensorInfo>>>,
}

async fn get_sensor_info(
    sensor_info_rw_lock: &Arc<RwLock<Option<SensorInfo>>>,
) -> Result<SensorInfo, io::Error> {
    let guard = sensor_info_rw_lock.read().await;
    match guard.as_ref() {
        Some(sensor_info) => Ok(sensor_info.to_owned()),
        None => {
            drop(guard);
            let fp_info = fp_get_info()?;
            let sensor_info = SensorInfo::from(fp_info);
            *sensor_info_rw_lock.write().await = Some(sensor_info);
            Ok(sensor_info)
        }
    }
}

impl CrosFp {
    async fn create_operation(&self, user_id: u32) -> OperationUniqueId {
        let mut lock = self.operation_ids.lock().await;
        let last_operation_id = lock.get_mut(&user_id);
        let operation_id = last_operation_id
            .as_ref()
            .map_or::<u32, _>(0, |operation_id| **operation_id + 1);
        match last_operation_id {
            Some(operation_id) => {
                *operation_id += 1;
            }
            None => {
                lock.insert(user_id, operation_id);
            }
        };
        let unique_id = OperationUniqueId {
            user_id,
            operation_id,
        };
        unique_id
    }
}

#[interface(name = "org.crosfp.CrosFp")]
impl CrosFp {
    async fn get_fp_info(&self) -> fdo::Result<SensorInfo> {
        Ok(get_sensor_info(&self.sensor_info)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?)
    }

    async fn start_enroll(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<u32> {
        println!("Start enroll");
        let user_id = get_user_id(header).await?;
        let unique_id = self.create_operation(user_id).await;
        let operation = OperationWithId {
            id: unique_id,
            operation: Operation::Enroll,
        };
        self.queue.write().await.push_back(operation);

        let queue = self.queue.clone();
        let queue_emitter = self.queue_emitter.clone();
        let operation_status_map = self.operation_status.clone();
        let sensor_state = self.sensor_state.clone();
        let sensor_info = self.sensor_info.clone();
        task::spawn(async move {
            let operation_status = Arc::new(EnrollingStatus::default());
            println!("Writing operation status");
            operation_status_map.write().await.insert(
                unique_id,
                OperationStatus::Enrolling(operation_status.clone()),
            );
            println!("Done Writing operation status");

            // Wait for queue
            println!(
                "Waiting in queue for enroll: {:#?} {:#?}",
                queue.read().await,
                unique_id
            );
            loop {
                if queue.read().await.front().unwrap().id == unique_id {
                    break;
                } else {
                    queue_emitter.listen().await;
                }
            }
            let _ = fp_seed(SEED);

            println!("Set seed");
            let slots_used = sensor_state
                .read()
                .await
                .as_ref()
                .map_or(0, |state| state.templates.len());
            let slots_total = get_sensor_info(&sensor_info).await.unwrap().templates;
            println!(
                "Slots used: {:#?}. Total slots: {:#?}",
                slots_used, slots_total
            );
            // Clear templates if all slots are used
            let slots_used = {
                if slots_used as u32 == slots_total {
                    fp_reset_sensor().unwrap();
                    *sensor_state.write().await = Some(SensorState {
                        active_user: Some(user_id),
                        templates: vec![],
                    });
                    0
                } else {
                    slots_used
                }
            };
            fp_enroll().unwrap();

            println!("Waiting for finger");
            loop {
                match fp_get_mode().unwrap() {
                    FpModeOutput::EnrollPlusImage => {
                        // Keep waiting for finger
                    }
                    FpModeOutput::Enroll => {
                        // Next stage
                        println!("Finger image done");
                        operation_status.data.write().await.images += 1;
                        fp_enroll().unwrap();
                        operation_status.event.notify(usize::MAX);
                        println!("Waiting for finger");
                    }
                    FpModeOutput::Reset => {
                        // This means it's done
                        println!("Enroll done");
                        let mut guard = operation_status.data.write().await;
                        println!("Got operation status write guard");
                        guard.images += 1;
                        let template = fp_get_template(slots_used).unwrap();
                        guard.template = Some(template.clone()).into();
                        println!("Getting sensor_state lock");
                        let mut sensor_state = sensor_state.write().await;
                        println!("Got sensor_state lock");
                        match sensor_state.as_mut() {
                            Some(sensor_state) => {
                                match sensor_state.active_user {
                                    Some(previous_active_user) => {
                                        if previous_active_user != user_id {
                                            sensor_state.active_user = None;
                                        }
                                    }
                                    None => {
                                        sensor_state.active_user = Some(user_id);
                                    }
                                };
                                sensor_state.templates.push(template);
                            }
                            None => {
                                *sensor_state = Some(SensorState {
                                    active_user: Some(user_id),
                                    templates: vec![template],
                                });
                            }
                        }
                        println!("Getting queue write guard");
                        queue.write().await.pop_front();
                        println!("Got queue write guard");
                        operation_status.event.notify(usize::MAX);
                        queue_emitter.notify(usize::MAX);
                        break;
                    }
                    _ => panic!("Unexpected fp mode"),
                }
            }
        });
        println!("Spawned thread");

        Ok(unique_id.operation_id)

        // match self.mode {
        //     Some(_) => Err(fdo::Error::AccessDenied("Sensor is busy".into())),
        //     None => {
        //         let user_id = get_user_id(header).await?;
        //         // seed
        //         let _ = fp_seed(SEED);
        //         fp_enroll()
        //             .map_err(|e| fdo::Error::Failed(format!("Error enrolling: {:#?}", e)))?;
        //         self.mode = Some(Enrolling(EnrollingData {
        //             user_id,
        //             template: Default::default(),
        //             images: Default::default()
        //         }));
        //         Ok(())
        //     }
        // }
    }

    async fn get_enroll_progress(
        &self,
        #[zbus(header)] header: Header<'_>,
        operation_id: u32,
        wait_for_next: bool,
    ) -> fdo::Result<Optional<EnrollingStatusData>> {
        let user_id = get_user_id(header).await?;
        let id = OperationUniqueId {
            user_id,
            operation_id,
        };
        let read_guard = self.operation_status.read().await;
        println!("Got read guard");
        let status = read_guard.get(&id);
        Ok(match status {
            Some(operation_status) => Some({
                match operation_status.to_owned() {
                    OperationStatus::Enrolling(enrolling_data) => {
                        if wait_for_next {
                            let enrolling_data = enrolling_data.clone();
                            drop(read_guard);
                            println!("Waiting for event");
                            enrolling_data.event.listen().await;
                            let x = enrolling_data.data.read().await.to_owned();
                            x
                        } else {
                            println!("Reading enrolling data");
                            let result = enrolling_data.data.read().await.to_owned();
                            println!("Got enrolling data");
                            result
                        }
                    }
                    _ => panic!("Wrong operation status"),
                }
            }),
            None => None,
        }
        .into())
    }

    async fn wait_for_operation_start(
        &self,
        #[zbus(header)] header: Header<'_>,
        operation_id: u32,
    ) -> fdo::Result<()> {
        let user_id = get_user_id(header).await?;
        let id = OperationUniqueId {
            user_id,
            operation_id,
        };
        let read_guard = self.queue.read().await;
        let is_in_queue = read_guard
            .iter()
            .find(|operation| operation.id == id)
            .is_some();
        match is_in_queue {
            true => {
                loop {
                    let read_guard = self.queue.read().await;
                    let will_start = read_guard
                        .front()
                        .map_or(false, |operation| operation.id == id);
                    if will_start {
                        break;
                    } else {
                        self.queue_emitter.listen().await;
                    }
                }
                Ok(())
            }
            false => Err(fdo::Error::Failed("Not in queue".into())),
        }
    }

    async fn clear_operation_result(
        &self,
        #[zbus(header)] header: Header<'_>,
        operation_id: u32,
    ) -> fdo::Result<()> {
        let user_id = get_user_id(header).await?;
        let unique_id = OperationUniqueId {
            operation_id,
            user_id,
        };
        match self.operation_status.write().await.remove(&unique_id) {
            Some(_operation_status) => Ok(()),
            None => Err(fdo::Error::Failed("Operation not found".into())),
        }
    }

    async fn match_finger(
        &self,
        #[zbus(header)] header: Header<'_>,
        templates: Vec<Vec<u8>>,
    ) -> fdo::Result<u32> {
        let user_id = get_user_id(header).await?;
        let unique_id = self.create_operation(user_id).await;
        let info = get_sensor_info(&self.sensor_info).await.unwrap();
        if templates.len() as u32 <= info.templates {
            self.queue.write().await.push_back(OperationWithId {
                id: unique_id,
                operation: Operation::Match(templates.clone()),
            });
            let operation_status: Arc<MatchingStatus> = Default::default();
            self.operation_status.write().await.insert(
                unique_id,
                OperationStatus::Matching(operation_status.clone()),
            );
            let queue = self.queue.clone();
            let queue_event = self.queue_emitter.clone();
            let sensor_state = self.sensor_state.clone();
            task::spawn(async move {
                loop {
                    if queue
                        .read()
                        .await
                        .front()
                        .map_or(false, |operation| operation.id == unique_id)
                    {
                        break;
                    }
                    queue_event.listen().await;
                }
                // Update the templates
                println!("Updating templates: {:#?}", templates.len());
                let templates = &templates;
                let reload_all_templates = move || async move {
                    fp_reset().unwrap();
                    fp_reset_sensor().unwrap();
                    let temp_file = Temp::new_file().unwrap();
                    // TODO: Write files in parallel
                    for template in templates {
                        let mut file = OpenOptions::new().write(true).open(temp_file.as_path()).await.unwrap();
                        file.write_all(template).await.unwrap();
                        file.flush().await.unwrap();
                        // TODO: Properly handle errors
                        fp_load_template(temp_file.as_path().to_str().unwrap()).unwrap();
                        // println!("Loaded template: {:#?} of size {:#?}. File: {:#?}", temp_file.as_os_str(), template.len(), file);
                    }
                    SensorState {
                        active_user: Some(user_id),
                        templates: templates.clone(),
                    }
                };
                let mut sensor_state = sensor_state.write().await;
                match sensor_state.as_mut() {
                    Some(sensor_state) => {
                        match sensor_state.active_user {
                            Some(previous_user) => {
                                if previous_user == user_id {
                                    if sensor_state.templates.len() > templates.len() {
                                        // It's not possible to delete loaded templates without deleting all loaded templates
                                        *sensor_state = reload_all_templates().await;
                                    } else {
                                        let (existing, new) =
                                            templates.split_at(sensor_state.templates.len());
                                        let same_exact_templates =
                                            sensor_state.templates.eq(existing);
                                        match same_exact_templates {
                                            true => {
                                                let temp_file = Temp::new_file().unwrap();
                                                let mut file =
                                                    File::open(temp_file.as_path()).await.unwrap();
                                                for template in new {
                                                    file.write_all(&template).await.unwrap();
                                                    fp_load_template(
                                                        temp_file.as_os_str().to_str().unwrap(),
                                                    )
                                                    .unwrap();
                                                    println!("Loaded template");
                                                    sensor_state
                                                        .templates
                                                        .push(template.to_owned());
                                                }
                                            }
                                            false => {
                                                *sensor_state = reload_all_templates().await;
                                            }
                                        }
                                    }
                                } else {
                                    *sensor_state = reload_all_templates().await;
                                }
                            }
                            None => {
                                *sensor_state = reload_all_templates().await;
                            }
                        }
                    }
                    None => {
                        *sensor_state = Some(reload_all_templates().await);
                    }
                }
                // Actually match
                println!("Matching");
                fp_set_mode(FpModeInput::Match).unwrap();
                // Wait until done matching
                loop {
                    match fp_get_mode().unwrap() {
                        FpModeOutput::Match => {
                            // Still waiting
                        }
                        FpModeOutput::Reset => {
                            // Done
                            break;
                        }
                        _ => panic!("Unexpected fp mode"),
                    };
                }
                println!("Done matching");
                let fp_stats = fp_get_stats().unwrap();
                println!("Getting lock");
                *operation_status.result.write().await =
                    Some(fp_stats.last_matching_finger.map(|index| index as u32));
                println!("Got lock");
                println!("Getting queue lock");
                queue.write().await.pop_front();
                println!("Got queue lock");
                operation_status.event.notify(usize::MAX);
            });
            Ok(unique_id.operation_id)
        } else {
            Err(fdo::Error::Failed(
                "Provided more templates than the sensor can hold".into(),
            ))
        }
    }

    async fn get_match_result(
        &self,
        #[zbus(header)] header: Header<'_>,
        operation_id: u32,
        wait: bool,
    ) -> fdo::Result<Vec<u8>> {
        let user_id = get_user_id(header).await?;
        let id = OperationUniqueId {
            user_id,
            operation_id,
        };
        let is_in_queue =  self.queue.read().await.iter().find(|operation| operation.id == id).map(|v| v.to_owned());
        match is_in_queue {
            Some(operation) => match operation.operation.borrow() {
                Operation::Match(_templates) => {
                    let operation_status_read_guard = self.operation_status.read().await;
                    match operation_status_read_guard
                        .get(&id)
                        .map(|v| v.to_owned())
                        .ok_or(fdo::Error::Failed("This shouldn't happen".into()))?
                    {
                        OperationStatus::Matching(status) => {
                            let result = status.result.read().await.to_owned();
                            Ok(match result {
                                Some(result) => postcard::to_allocvec(&Some(result)).unwrap(),
                                None => {
                                    if wait {
                                        let status = status.clone();
                                        drop(operation_status_read_guard);
                                        status.event.listen().await;
                                        let x = postcard::to_allocvec(&Some(status.result.read().await.unwrap().to_owned())).unwrap();
                                        x
                                    } else {
                                        postcard::to_allocvec(&None::<Option<u32>>).unwrap()
                                    }
                                }
                            })
                        }
                        _ => panic!("Unexpected operation type"),
                    }
                }
                _ => Err(fdo::Error::Failed(
                    "The operation is not an match operation".into(),
                )),
            },
            None => Err(fdo::Error::Failed(format!(
                "No operation with the id: {} for the user: {}",
                operation_id, user_id
            ))),
        }
    }

    // /// Proceeds to next step or completes
    // async fn continue_enrolling(
    //     &mut self,
    //     #[zbus(header)] header: Header<'_>,
    // ) -> fdo::Result<EnrollingData> {
    //     match &mut self.mode {
    //         Some(Enrolling(enrolling_data)) => {
    //             let user_id = get_user_id(header).await?;
    //             if user_id == enrolling_data.user_id{
    //                 let fp_mode = fp_get_mode().map_err(|e| {
    //                     fdo::Error::Failed(format!("Error getting fp mode: {:#?}", e))
    //                 })?;
    //                 match fp_mode {
    //                     FpModeOutput::Enroll => {
    //                         enrolling_data.images += 1;
    //                         fp_enroll()
    //                             .map_err(|e| fdo::Error::Failed(format!("Error enrolling: {:#?}", e)))?;
    //                         Ok(enrolling_data.to_owned())
    //                     },
    //                     FpModeOutput::EnrollPlusImage => Ok(enrolling_data.to_owned()),
    //                     FpModeOutput::Reset => {
    //                         // TODO: Handle sensor being reseted from other things such as suspend
    //                         // This means enrolling is done
    //                         enrolling_data.images += 1;
    //                         let info = fp_get_info().map_err(|e| {
    //                             fdo::Error::Failed(format!("Error getting fp info: {:#?}", e))
    //                         })?;
    //                         let template =
    //                             fp_get_template(info.templates_slots_used - 1).map_err(|e| {
    //                                 fdo::Error::Failed(format!("Error getting template: {:#?}", e))
    //                             })?;
    //                         enrolling_data.template = Some(template).into();
    //                         let enrolling_data = enrolling_data.to_owned();
    //                         self.mode = None;
    //                         Ok(enrolling_data)
    //                     }
    //                     _ => Err(fdo::Error::Failed(format!(
    //                         "Unexpected fp mode: {:#?}",
    //                         fp_mode
    //                     ))),
    //                 }
    //             } else {
    //                 Err(fdo::Error::AccessDenied(
    //                     "A different user is using the fp sensor".into(),
    //                 ))
    //             }
    //         }
    //         _ => Err(fdo::Error::AccessDenied("Not enrolling".into())),
    //     }
    // }

    // async fn enroll_next_step(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
    //     match &self.enrolling {
    //         Some(enrolling) => {
    //             let user_id = get_user_id(header).await?;
    //             if user_id == enrolling.user_id {
    //                 fp_enroll()
    //                     .map_err(|e| fdo::Error::Failed(format!("Error enrolling: {:#?}", e)))?;
    //                 Ok(())
    //             } else {
    //                 Err(fdo::Error::AccessDenied(
    //                     "A different user is using the sensor".into(),
    //                 ))
    //             }
    //         }
    //         None => Err(fdo::Error::AccessDenied("Not enrolling".into())),
    //     }
    // }
}

// Although we use `async-std` here, you can use any async runtime of choice.
#[async_std::main]
async fn main() -> zbus::Result<()> {
    println!("Starting dbus interface");
    match fp_hello() {
        Ok(_) => {
            let _connection = Builder::system()?
                .name("org.crosfp.CrosFp")?
                .serve_at("/org/crosfp/CrosFp", CrosFp::default())?
                .build()
                .await?;

            loop {
                thread::park();
            }
        }
        Err(e) => {
            println!("Error: {:#?}", e);
            Err(zbus::Error::Failure(format!(
                "Couldn't communicate with fp sensor: {:#?}",
                e
            )))
        }
    }
}
