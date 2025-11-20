mod photo_grid_view;

use gpui::*;
use photo_grid_view::PhotoGridView;
use std::env;
use std::sync::Arc;

fn main() {
    env_logger::init();

    // Get configuration from environment
    let socket_path = env::var("SD_SOCKET_PATH").unwrap_or_else(|_| {
        let home = env::var("HOME").expect("HOME not set");
        format!("{}/Library/Application Support/spacedrive/daemon/daemon.sock", home)
    });

    let http_url = env::var("SD_HTTP_URL").unwrap_or_else(|_e| "http://127.0.0.1:56851".to_string());

    let library_id = env::var("SD_LIBRARY_ID").expect("SD_LIBRARY_ID environment variable must be set");

    let initial_path = env::var("SD_INITIAL_PATH").unwrap_or_else(|_e| "/Users/jamespine/Desktop".to_string());

    println!("Starting GPUI Photo Grid");
    println!("  Socket: {}", socket_path);
    println!("  HTTP: {}", http_url);
    println!("  Library: {}", library_id);
    println!("  Path: {}", initial_path);

    // Create HTTP client for image loading
    let http_client = Arc::new(
        reqwest_client::ReqwestClient::user_agent("spacedrive-gpui")
            .expect("Failed to create HTTP client"),
    );

    Application::new()
        .with_http_client(http_client)
        .run(move |cx: &mut App| {
            cx.activate(true);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1200.0), px(800.0)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Spacedrive Media Grid".into()),
                        appears_transparent: false,
                        ..Default::default()
                    }),
                    focus: true,
                    show: true,
                    kind: WindowKind::Normal,
                    is_movable: true,
                    display_id: None,
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|cx| {
                        PhotoGridView::new(
                            socket_path.into(),
                            http_url,
                            library_id,
                            initial_path,
                            cx,
                        )
                    })
                },
            )
            .expect("Failed to open window");
        });
}
