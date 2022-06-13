use tauri::{GlobalWindowEvent, Runtime, Window, Wry};

#[cfg(target_os = "macos")]
use cocoa::{
	appkit::{
		NSToolbar, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
		NSWindow, NSWindowStyleMask, NSWindowTitleVisibility,
	},
	base::{id, nil, NO, YES},
	delegate,
	foundation::NSString,
};
#[cfg(target_os = "macos")]
use objc::{
	class, msg_send,
	runtime::{Object, Sel},
	sel, sel_impl,
};

pub(crate) fn handle_window_event(event: GlobalWindowEvent<Wry>) {
	match event.event() {
		_ => {}
	}
}

pub trait WindowExt {
	#[cfg(target_os = "macos")]
	fn set_toolbar(&self, shown: bool);
	#[cfg(target_os = "macos")]
	fn set_transparent_titlebar(&self, transparent: bool, large: bool);
	#[cfg(target_os = "macos")]
	fn set_blurs_behind(&self, blurs: bool);
	#[cfg(target_os = "macos")]
	fn fix_shadow(&self);
}

impl<R: Runtime> WindowExt for Window<R> {
	#[cfg(target_os = "macos")]
	fn set_toolbar(&self, shown: bool) {
		unsafe {
			let id = self.ns_window().unwrap() as cocoa::base::id;

			if shown {
				let toolbar =
					NSToolbar::alloc(nil).initWithIdentifier_(NSString::alloc(nil).init_str("wat"));
				toolbar.setShowsBaselineSeparator_(NO);
				id.setToolbar_(toolbar);
			} else {
				id.setToolbar_(nil);
			}
		}
	}

	#[cfg(target_os = "macos")]
	fn set_blurs_behind(&self, blurs: bool) {
		let our_ns_window = self.ns_window().unwrap() as id;

		if !blurs {
			()
		}

		// window_set_blurry_background(our_ns_window);

		#[cfg(target_os = "macos")]
		extern "C" fn on_window_loaded(this: &Object, _cmd: Sel, _notification: id) {
			println!("Window loaded! Setting blurry background...");

			unsafe {
				let window_object: id = *this.get_ivar("window");
				window_set_blurry_background(window_object);
			}
		}

		unsafe {
			let delegate: id = delegate!("SpacedriveWindowBlurryDelegate", {
				window: id = our_ns_window,
				(windowDidLoad:) => on_window_loaded as extern fn(&Object, Sel, id)
			});

			our_ns_window.setDelegate_(delegate);
		};
	}

	#[cfg(target_os = "macos")]
	fn set_transparent_titlebar(&self, transparent: bool, large: bool) {
		unsafe {
			let window = self.ns_window().unwrap() as id;

			let mut style_mask = window.styleMask();
			// println!("existing style mask, {:#?}", style_mask);
			style_mask.set(
				NSWindowStyleMask::NSFullSizeContentViewWindowMask,
				transparent,
			);
			style_mask.set(
				NSWindowStyleMask::NSTexturedBackgroundWindowMask,
				transparent,
			);
			style_mask.set(
				NSWindowStyleMask::NSUnifiedTitleAndToolbarWindowMask,
				transparent && large,
			);
			window.setStyleMask_(style_mask);

			if large {
				self.set_toolbar(true);
			}

			window.setTitleVisibility_(if transparent {
				NSWindowTitleVisibility::NSWindowTitleHidden
			} else {
				NSWindowTitleVisibility::NSWindowTitleVisible
			});

			window.setTitlebarAppearsTransparent_(if transparent { YES } else { NO });
		}
	}

	#[cfg(target_os = "macos")]
	fn fix_shadow(&self) {
		unsafe {
			let id = self.ns_window().unwrap() as cocoa::base::id;

			println!("recomputing shadow for window {:?}", id.title());

			id.invalidateShadow();
		}
	}
}

// I tried going raw with the objc package here instead of relying on bindings...
// unfortunately this still isn't working.
// I try the delegate up above and its action for window load never seems to run
// And calling this manually does nothing. I wish the next person to attempt making this work the best of luck.
// - maxichrome | 12 jun 2022
#[cfg(target_os = "macos")]
pub(crate) fn window_set_blurry_background(window: id) {
	println!("setting blurry background on {:#?}", window);

	#[allow(non_snake_case)]
	let NSVisualEffectView = class!(NSVisualEffectView);

	unsafe {
		let content_view: id = msg_send![window, contentView];

		let visual_effect: *mut Object = msg_send![NSVisualEffectView, new];
		let _: () = msg_send![
			visual_effect,
			setMaterial: NSVisualEffectMaterial::NSVisualEffectMaterialSidebar
		];
		let _: () = msg_send![
			visual_effect,
			setState: NSVisualEffectState::NSVisualEffectStateFollowsWindowActiveState
		];
		let _: () = msg_send![
			visual_effect,
			setBlendingMode: NSVisualEffectBlendingMode::NSVisualEffectBlendingModeBehindWindow
		];
		let _: () = msg_send![visual_effect, setWantsLayer: YES];

		let _: () = msg_send![visual_effect, addSubview: content_view];
		let _: () = msg_send![window, setContentView: visual_effect];

		// let content_frame: id = msg_send![window, frame];
		// let _: () = msg_send![content_view, setFrame: content_frame];
	}
}
