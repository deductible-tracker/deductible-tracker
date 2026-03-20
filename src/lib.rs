pub mod db;

#[cfg(feature = "server")]
mod auth;
#[cfg(feature = "server")]
mod observability;
#[cfg(feature = "server")]
mod ocr;
#[cfg(feature = "server")]
mod routes;
#[cfg(feature = "server")]
mod storage;

pub use crate::db as db_mod;

#[cfg(feature = "server")]
include!("main_sections/bootstrap/server_bootstrap.rs");
#[cfg(feature = "server")]
include!("main_sections/http/http_pipeline_and_assets.rs");
#[cfg(feature = "asset-pipeline")]
include!("main_sections/assets/asset_helpers.rs");
