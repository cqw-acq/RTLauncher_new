use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSColor, NSWindow};
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;

#[cfg(target_os = "macos")]
const NS_WINDOW_TITLE_HIDDEN: i64 = 1;
#[cfg(target_os = "macos")]
const NS_WINDOW_STYLE_MASK_FULL_SIZE_CONTENT_VIEW: u64 = 1 << 15;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let window = if let Some(window) = app.get_webview_window("main") {
                window
            } else {
                let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                    .title("RTLauncher")
                    .inner_size(1200.0, 800.0)
                    .center()
                    .resizable(true)
                    .fullscreen(false)
                    .shadow(true);

                #[cfg(target_os = "macos")]
                let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);

                #[cfg(not(target_os = "macos"))]
                let win_builder = win_builder.decorations(false);

                win_builder.build()?
            };

            #[cfg(not(target_os = "macos"))]
            let _ = &window;

            #[cfg(target_os = "macos")]
            unsafe {
                let ns_window_ptr = window.ns_window().unwrap() as *mut AnyObject;
                let ns_window = &*(ns_window_ptr as *const NSWindow);

                // Transparent title bar and hidden native title text.
                let () = msg_send![ns_window_ptr, setTitlebarAppearsTransparent: true];
                let () = msg_send![ns_window_ptr, setTitleVisibility: NS_WINDOW_TITLE_HIDDEN];

                // Extend content into the title bar region.
                let style_mask: u64 = msg_send![ns_window_ptr, styleMask];
                let style_mask = style_mask | NS_WINDOW_STYLE_MASK_FULL_SIZE_CONTENT_VIEW;
                let () = msg_send![ns_window_ptr, setStyleMask: style_mask];
                let () = msg_send![ns_window_ptr, setMovableByWindowBackground: true];

                let bg_color = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.0);
                ns_window.setBackgroundColor(Some(&bg_color));
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
