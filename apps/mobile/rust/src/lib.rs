use std::sync::Arc;

use once_cell::sync::{Lazy, OnceCell};
use sdcore::{
	api::Router,
	rspc::{ClientContext, Response},
	Node,
};
use tokio::{
	runtime::Runtime,
	sync::{mpsc::UnboundedSender, Mutex},
};

#[allow(dead_code)]
pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

#[allow(dead_code)]
pub(crate) static NODE: Lazy<Mutex<Option<(Arc<Node>, Arc<Router>)>>> =
	Lazy::new(|| Mutex::new(None));

#[allow(dead_code)]
pub(crate) static CLIENT_CONTEXT: Lazy<ClientContext> = Lazy::new(|| ClientContext {
	subscriptions: Default::default(),
});

#[allow(dead_code)]
pub(crate) static EVENT_SENDER: OnceCell<UnboundedSender<Response>> = OnceCell::new();

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "android")]
mod android;
