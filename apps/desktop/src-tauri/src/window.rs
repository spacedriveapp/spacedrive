use tauri::{GlobalWindowEvent, Runtime, Window, Wry};

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
		use cocoa::{
			appkit::{NSToolbar, NSWindow},
			base::{nil, NO},
			foundation::NSString,
		};

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
		use cocoa::{
			appkit::{
				NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
				NSVisualEffectView, NSWindow,
			},
			base::{id, nil},
			delegate,
		};
		use objc::{
			class, msg_send,
			runtime::{Object, Sel},
			sel, sel_impl,
		};

		unsafe {
			let id = self.ns_window().unwrap() as cocoa::base::id;

			println!("ASDFGHJKL; Running set_blurs_behind");

			if !blurs {
				()
			}

			println!("ASDFGHJKL; Still running set_blurs_behind!");

			extern "C" fn on_window_loaded(this: &Object, _cmd: Sel, _notification: id) {
				println!("ASDFGHJKL; Window loaded!");

				unsafe {
					let window: id = *this.get_ivar("window");

					window.setOpaque_(false);
					// window.setAlphaValue_(0.98 as _);

					let visual_effect = NSVisualEffectView::alloc(nil);
					visual_effect.setMaterial(
						NSVisualEffectMaterial::NSVisualEffectMaterialContentBackground,
					);
					visual_effect.setState(NSVisualEffectState::NSVisualEffectStateActive);
					visual_effect.setBlendingMode(
						NSVisualEffectBlendingMode::NSVisualEffectBlendingModeBehindWindow,
					);
					visual_effect.setWantsLayer(true);

					window.addSubview_(visual_effect);
				}
			}

			println!("ASDFGHJKL; Setting delegate!");
			id.setDelegate_(delegate!("SpacedriveMainWindowDelegate", {
					window: id = id,
					(windowDidLoad:) => on_window_loaded as extern fn(&Object, Sel, id)
			}));

			// visual_effect.setFrameSize(id.bounds());
		}
	}

	#[cfg(target_os = "macos")]
	fn set_transparent_titlebar(&self, transparent: bool, large: bool) {
		use cocoa::{
			appkit::{NSWindow, NSWindowStyleMask, NSWindowTitleVisibility},
			base::{NO, YES},
		};

		unsafe {
			let id = self.ns_window().unwrap() as cocoa::base::id;

			let mut style_mask = id.styleMask();
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
			id.setStyleMask_(style_mask);

			if large {
				self.set_toolbar(true);
			}

			id.setTitleVisibility_(if transparent {
				NSWindowTitleVisibility::NSWindowTitleHidden
			} else {
				NSWindowTitleVisibility::NSWindowTitleVisible
			});

			id.setTitlebarAppearsTransparent_(if transparent { YES } else { NO });
		}
	}

	#[cfg(target_os = "macos")]
	fn fix_shadow(&self) {
		use cocoa::appkit::NSWindow;

		unsafe {
			let id = self.ns_window().unwrap() as cocoa::base::id;

			println!("recomputing shadow for window {:?}", id.title());

			id.invalidateShadow();
		}
	}
}
