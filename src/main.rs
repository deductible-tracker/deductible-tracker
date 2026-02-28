// Organized by responsibility for maintainability.
mod db;
mod routes;
mod auth;
mod ocr;

include!("main_sections/bootstrap/server_bootstrap.rs");
include!("main_sections/http/http_pipeline_and_assets.rs");
include!("main_sections/assets/asset_helpers.rs");
