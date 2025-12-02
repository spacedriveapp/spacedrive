use gpui::*;
use sd_client::{File, SdPath, SpacedriveClient};
use std::sync::Arc;

pub struct PhotoGridView {
    client: Arc<SpacedriveClient>,
    files: Vec<File>,
    columns: usize,
    thumb_size: f32,
    gap: f32,
    loading: bool,
    error: Option<String>,
}

impl PhotoGridView {
    pub fn new(
        socket_addr: String,
        http_url: String,
        library_id: String,
        initial_path: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut client = SpacedriveClient::new(socket_addr, http_url);
        client.set_library(library_id);
        let client = Arc::new(client);

        let mut view = Self {
            client: client.clone(),
            files: Vec::new(),
            columns: 6,
            thumb_size: 200.0,
            gap: 4.0,
            loading: true,
            error: None,
        };

        // Load files asynchronously
        view.load_files(initial_path, cx);

        view
    }

    fn load_files(&mut self, path: String, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        cx.notify();

        let client = self.client.clone();

        cx.spawn(async move |this, cx| {
            // Query files from daemon in background
            let result = {
                // Use Tokio runtime for the client call
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    client
                        .media_listing(
                            SdPath::Physical {
                                device_slug: "james-s-macbook-pro".to_string(),
                                path: path.clone().into(),
                            },
                            Some(1000),
                        )
                        .await
                })
            };

            // Update view with results on main thread
            _ = this.update(cx, |this, cx| {
                this.loading = false;

                match result {
                    Ok(files) => {
                        println!("Loaded {} files", files.len());
                        this.files = files;
                        this.error = None;
                    }
                    Err(e) => {
                        eprintln!("Error loading files: {}", e);
                        this.error = Some(format!("Failed to load files: {}", e));
                        this.files = Vec::new();
                    }
                }

                cx.notify();
            });
        })
        .detach();
    }

    fn render_thumbnail(&self, file: &File) -> impl IntoElement {
        // Find best thumbnail
        let thumbnail = self.client.select_best_thumbnail(&file.sidecars, self.thumb_size);

        let thumb_size = px(self.thumb_size);

        if let (Some(content_id), Some(thumb)) = (&file.content_identity, thumbnail) {
            let url = self.client.thumbnail_url(
                &content_id.uuid.to_string(),
                &thumb.variant,
                &thumb.format,
            );

            // Debug: print first URL
            static PRINTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("First thumbnail URL: {}", url);
            }

            div()
                .size(thumb_size)
                .flex_shrink_0()
                .bg(rgb(0x1a1a1a))
                .rounded(px(4.0))
                .overflow_hidden()
                .child(
                    img(url)
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(|| {
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(0x2a2a2a))
                                .text_color(rgb(0x888888))
                                .child("✗")
                                .into_any_element()
                        }),
                )
        } else {
            // No thumbnail available
            div()
                .size(thumb_size)
                .flex_shrink_0()
                .bg(rgb(0x2a2a2a))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0x666666))
                .child("No preview")
        }
    }

    fn render_grid(&self) -> impl IntoElement {
        if self.loading {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xcccccc))
                .child("Loading files...")
                .into_any_element();
        }

        if let Some(error) = &self.error {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_color(rgb(0xff6b6b))
                        .text_size(px(16.0))
                        .child("Error"),
                )
                .child(div().text_color(rgb(0xcccccc)).child(error.clone()))
                .into_any_element();
        }

        if self.files.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0x888888))
                .child("No media files found")
                .into_any_element();
        }

        // Render grid rows
        let mut rows = Vec::new();
        for chunk in self.files.chunks(self.columns) {
            let row = div()
                .flex()
                .gap(px(self.gap))
                .children(chunk.iter().map(|file| self.render_thumbnail(file)));
            rows.push(row);
        }

        div()
            .flex()
            .flex_col()
            .gap(px(self.gap))
            .p(px(8.0))
            .children(rows)
            .into_any_element()
    }

    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .p(px(12.0))
            .bg(rgb(0x1a1a1a))
            .border_b_1()
            .border_color(rgb(0x333333))
            .child(
                div()
                    .text_color(rgb(0xcccccc))
                    .text_size(px(14.0))
                    .child(format!("{} items", self.files.len())),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .text_color(rgb(0x888888))
                    .text_size(px(12.0))
                    .child(format!("Columns: {}", self.columns))
                    .child("·")
                    .child(format!("Size: {}px", self.thumb_size as i32)),
            )
    }
}

impl Render for PhotoGridView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0f0f0f))
            .text_color(rgb(0xffffff))
            .child(self.render_header())
            .child(
                div()
                    .id("scroll-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(self.render_grid()),
            )
    }
}
