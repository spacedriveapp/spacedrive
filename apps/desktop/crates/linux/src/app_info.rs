use std::path::{Path, PathBuf};

use gtk::{
	gio::{
		content_type_guess, prelude::AppInfoExt, prelude::AppLaunchContextExt, AppInfo,
		AppLaunchContext, DesktopAppInfo, File as GioFile, ResourceError,
	},
	glib::error::Error as GlibError,
	prelude::IsA,
};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::env::remove_prefix_from_pathlist;

fn remove_prefix_from_env_in_ctx(
	ctx: &impl IsA<AppLaunchContext>,
	env_name: &str,
	prefix: &impl AsRef<Path>,
) {
	if let Some(value) = remove_prefix_from_pathlist(env_name, prefix) {
		ctx.setenv(env_name, value);
	} else {
		ctx.unsetenv(env_name);
	}
}

thread_local! {
	static LAUNCH_CTX: AppLaunchContext = {
		// TODO: Display supports requires GDK, which can only run on the main thread
		// let ctx = Display::default()
		// 		.and_then(|display| display.app_launch_context())
		// 		.map(|display| display.to_value().get::<AppLaunchContext>().expect(
		// 			"This is an Glib type conversion, it should never fail because GDKAppLaunchContext is a subclass of AppLaunchContext"
		// 		)).unwrap_or_default();

		let ctx = AppLaunchContext::default();

		if let Some(appdir) = std::env::var_os("APPDIR").map(PathBuf::from) {
			// Remove AppImage paths from environment variables to avoid external applications attempting to use the AppImage's libraries
			// https://github.com/AppImage/AppImageKit/blob/701b711f42250584b65a88f6427006b1d160164d/src/AppRun.c#L168-L194
			ctx.unsetenv("PYTHONHOME");
			remove_prefix_from_env_in_ctx(&ctx, "PATH", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "LD_LIBRARY_PATH", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "PYTHONPATH", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "XDG_DATA_DIRS", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "GSETTINGS_SCHEMA_DIR", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "QT_PLUGIN_PATH", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "GST_PLUGIN_SYSTEM_PATH", &appdir);
			remove_prefix_from_env_in_ctx(&ctx, "GST_PLUGIN_SYSTEM_PATH_1_0", &appdir);
		}

		ctx
	}
}

pub struct App {
	pub id: String,
	pub name: String,
	// pub icon: Option<Vec<u8>>,
}

async fn recommended_for_type(file_path: impl AsRef<Path>) -> Vec<AppInfo> {
	let data = if let Ok(mut file) = File::open(&file_path).await {
		let mut data = [0; 1024];
		if file.read_exact(&mut data).await.is_ok() {
			Some(data)
		} else {
			None
		}
	} else {
		None
	};

	let file_path = Some(file_path);
	let (content_type, uncertain) = if let Some(data) = data {
		content_type_guess(file_path, &data)
	} else {
		content_type_guess(file_path, &[])
	};

	if uncertain {
		vec![]
	} else {
		AppInfo::recommended_for_type(content_type.as_str())
	}
}

pub async fn list_apps_associated_with_ext(file_path: impl AsRef<Path>) -> Vec<App> {
	recommended_for_type(file_path)
		.await
		.iter()
		.flat_map(|app_info| {
			app_info.id().map(|id| App {
				id: id.to_string(),
				name: app_info.name().to_string(),
				// TODO: Icon supports requires GTK, which can only run on the main thread
				// icon: app_info
				// 	.icon()
				// 	.and_then(|icon| {
				// 		IconTheme::default().and_then(|icon_theme| {
				// 			icon_theme.lookup_by_gicon(&icon, 128, IconLookupFlags::empty())
				// 		})
				// 	})
				// 	.and_then(|icon_info| icon_info.load_icon().ok())
				// 	.and_then(|pixbuf| pixbuf.save_to_bufferv("png", &[]).ok()),
			})
		})
		.collect()
}

pub fn open_files_path_with(file_paths: &[impl AsRef<Path>], id: &str) -> Result<(), GlibError> {
	let Some(app) = DesktopAppInfo::new(id) else {
		return Err(GlibError::new(ResourceError::NotFound, "App not found"))
	};

	LAUNCH_CTX.with(|ctx| {
		app.launch(
			&file_paths.iter().map(GioFile::for_path).collect::<Vec<_>>(),
			Some(ctx),
		)
	})
}
