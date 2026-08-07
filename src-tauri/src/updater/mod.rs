pub mod config;
pub mod fetcher;
pub mod handler;

pub use config::UpdateConfig;
pub use handler::{
    check_for_updates, download_update, get_update_status, install_update,
};