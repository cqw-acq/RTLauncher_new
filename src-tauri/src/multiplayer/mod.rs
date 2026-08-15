mod codec;
mod logs;
mod paths;
mod process;

pub use process::{
    ensure_openp2p_stopped, mp_check_openp2p, mp_encode_room_info, mp_get_openp2p_dir,
    mp_get_openp2p_path, mp_install_openp2p, mp_is_openp2p_running, mp_poll_log,
    mp_start_openp2p_host, mp_start_openp2p_join, mp_stop_openp2p,
};

#[cfg(target_os = "windows")]
pub use process::quick_kill_openp2p;

const OPENP2P_BIN: &str = if cfg!(target_os = "windows") {
    "openp2p.exe"
} else {
    "openp2p"
};
