pub mod drivers;
pub mod fingerprint_driver;


// use futures::future::join_all;
// use std::collections::HashMap;
// use std::error::Error;
// use std::fmt::{Display, Formatter};
// use std::io;
// use std::time::Duration;
// use async_std::fs::File;
// use async_std::task::sleep;
// use futures::AsyncReadExt;
// use crate::drivers::get_drivers;
// use crate::fingerprint_driver::{EnrollStepOutput, MatchOutput};
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     let drivers = get_drivers();
//     let compatibilities = join_all(drivers.iter().map(|driver| Box::pin((driver.is_compatible)()))).await;
//     let (oks, errors) = {
//         let mut oks = vec![];
//         let mut errors = vec![];
//         for (index, result) in compatibilities.into_iter().enumerate() {
//             match result {
//                 Ok(value) => {
//                     oks.push((index, value));
//                 }
//                 Err(e) => {
//                     errors.push((index, e));
//                 }
//             }
//         }
//         (oks, errors)
//     };
//     if !errors.is_empty() {
//         #[derive(Debug)]
//         struct ErrorCheckingCompatibleDrivers {
//             errors: HashMap<&'static str, io::Error>,
//         }
//
//         impl Display for ErrorCheckingCompatibleDrivers {
//             fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//                 write!(f, "Error checking compatible drivers: {:#?}", self.errors)
//             }
//         }
//         impl Error for ErrorCheckingCompatibleDrivers {}
//
//         return Err(ErrorCheckingCompatibleDrivers { errors: errors.into_iter().map(|(index, error)| (drivers[index].name, error)).collect() }.into());
//     }
//
//     enum CompatibleDrivers {
//         None,
//         One(usize),
//         Multiple(Vec<usize>),
//     }
//     let compatible_drivers = {
//         let compatible_drivers = oks.into_iter().filter_map(|(index, compatible)| match compatible {
//             true => Some(index),
//             false => None
//         }).collect::<Vec<_>>();
//         match compatible_drivers.len() {
//             0 => CompatibleDrivers::None,
//             1 => CompatibleDrivers::One(compatible_drivers[0]),
//             2.. => CompatibleDrivers::Multiple(compatible_drivers)
//         }
//     };
//     match compatible_drivers {
//         CompatibleDrivers::None => println!("No compatible drivers"),
//         CompatibleDrivers::One(index) => {
//             let compatible_driver = &drivers[index];
//             println!("Compatible driver found: {}", compatible_driver.name);
//             let mut opened_driver = (compatible_driver.open_and_init)().await?;
//             println!("Opened driver");
//             let mut templates = vec![];
//             for _ in 0..opened_driver.get_max_templates().unwrap().min(4) {
//                 println!("Enrolling a new finger");
//                 loop {
//                     match opened_driver.start_or_continue_enroll().await {
//                         Ok(output) => match output {
//                             EnrollStepOutput::InProgress(percentage) => {
//                                 println!("Enroll progress: {percentage}%");
//                             },
//                             EnrollStepOutput::Complete(template) => {
//                                 println!("Enroll complete. Got template of size {}", template.len());
//                                 templates.push(template);
//                                 break;
//                             }
//                         },
//                         Err(error) => {
//                             println!("Error: {error:?}. Please try again.");
//                         }
//                     }
//                 }
//             }
//             loop {
//                 println!("Press finger on fp sensor to match");
//                 match opened_driver.match_templates(&templates).await.unwrap() {
//                     MatchOutput::Match(matched_output) => {
//                         println!("Matched: {:?}", matched_output.index);
//                         if let Some(updated_template) = matched_output.updated_template {
//                             templates[matched_output.index] = updated_template;
//                             println!("Also updated template");
//                         }
//                     },
//                     MatchOutput::NoMatch(no_match) => {
//                         println!("No match: {no_match:?}");
//                     }
//                 }
//                 sleep(Duration::from_secs(1)).await;
//             }
//         },
//         CompatibleDrivers::Multiple(indexes) => {
//             println!("Too many compatible drivers: {:?}", indexes.into_iter().map(|index| drivers[index].name))
//         }
//     }
//     Ok(())
// }
