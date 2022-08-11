use std::sync::Arc;

use once_cell::sync::Lazy;
use sdcore::{api::Router, Node};
use tokio::{runtime::Runtime, sync::Mutex};

#[allow(dead_code)]
pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

#[allow(dead_code)]
pub(crate) static NODE: Lazy<Mutex<Option<(Arc<Node>, Arc<Router>)>>> =
	Lazy::new(|| Mutex::new(None));

#[cfg(all(not(feature = "ios"), not(feature = "android")))]
compile_error!("You can't compile with the 'ios' and 'android' features both disabled.");

#[cfg(all(feature = "ios", feature = "android"))]
compile_error!("You can't compile with the 'ios' and 'android' features both enabled.");

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "android")]
mod android;
