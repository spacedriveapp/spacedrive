use std::{collections::HashMap, sync::Arc};

use once_cell::sync::{Lazy, OnceCell};
use rspc::internal::jsonrpc::{RequestId, Response};
use sd_core::{api::Router, Node};
use tokio::{
	runtime::Runtime,
	sync::{mpsc::UnboundedSender, oneshot, Mutex},
};

#[allow(dead_code)]
pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

type NodeType = Lazy<Mutex<Option<(Arc<Node>, Arc<Router>)>>>;

#[allow(dead_code)]
pub(crate) static NODE: NodeType = Lazy::new(|| Mutex::new(None));

#[allow(dead_code)]
pub(crate) static SUBSCRIPTIONS: Lazy<Mutex<HashMap<RequestId, oneshot::Sender<()>>>> =
	Lazy::new(Default::default);

#[allow(dead_code)]
pub(crate) static EVENT_SENDER: OnceCell<UnboundedSender<Response>> = OnceCell::new();

#[cfg(target_os = "ios")]
mod ios;

/// This is `not(ios)` instead of `android` because of https://github.com/mozilla/rust-android-gradle/issues/93
#[cfg(not(target_os = "ios"))]
mod android;
