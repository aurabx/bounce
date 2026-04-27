//! Outbound C-STORE send module.
//!
//! Implements the second half of the Aurabox → remote PACS push pipeline.
//! The first half — fetching DICOM bytes from Uhura's WADO-RS surface — is in
//! [`uhura_client`]; the C-STORE SCU itself lives in [`crate::query::cstore`];
//! per-job orchestration is in [`worker`].

pub mod uhura_client;
pub mod worker;

#[cfg(test)]
mod uhura_client_tests;
