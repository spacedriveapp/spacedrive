use sdcore::Core;
use std::{env, path::Path};

#[tokio::main]
async fn main() {
	let data_dir_var = "DATA_DIR";
	let data_dir = match env::var(data_dir_var) {
		Ok(path) => path,
		Err(e) => panic!("${} is not set ({})", data_dir_var, e),
	};

	let data_dir_path = Path::new(&data_dir);

	let (mut core, mut event_receiver) = Core::new(data_dir_path.to_path_buf()).await;

	core.initializer().await;

	let controller = core.get_controller();

	tokio::spawn(async move {
		core.start().await;
	});
}
