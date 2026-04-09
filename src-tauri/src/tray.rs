use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, Runtime, WebviewUrl, WebviewWindowBuilder,
};

pub fn create_tray<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_item, &separator, &quit_item])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("LipService — Voice to Text")
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                // Try to show existing window first
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.center();
                    let _ = window.set_focus();
                    return;
                }

                // Window was closed — recreate it
                if let Ok(window) = WebviewWindowBuilder::new(
                    app,
                    "main",
                    WebviewUrl::default(),
                )
                .title("LipService Settings")
                .inner_size(440.0, 580.0)
                .resizable(false)
                .center()
                .skip_taskbar(true)
                .build()
                {
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
