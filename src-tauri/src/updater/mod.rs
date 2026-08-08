pub mod config;
pub mod fetcher;
pub mod handler;

pub use config::UpdateConfig;
pub use handler::{
    can_check_update, cancel_update, check_for_updates, create_updater_state, download_update,
    get_target_version, get_update_status, install_update,
};