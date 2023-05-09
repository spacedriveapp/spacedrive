use async_stream::stream;
use rspc::alpha::AlphaRouter;

use crate::api::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|ctx, _: ()| async move {
				let notifications = ctx.notifier.get_notifications().await;
				Ok(notifications)
			})
		})
		.procedure("clearAll", {
			R.query(|ctx, _: ()| async move {
				ctx.notifier.clear_notifications().await;
			})
		})
		.procedure("listen", {
			R.subscription(|ctx, _: ()| async move {
				let mut sub = ctx.notifier.subscribe();

				stream! {
					while let Ok(notification) = sub.recv().await {
						yield notification;
					}
				}
			})
		})
}
