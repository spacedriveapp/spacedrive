//! Quick Look: Space shows the original of the photo under the cursor in
//! macOS's own preview panel, as it does in Finder. The panel opens whatever
//! the system can, HEIC and RAW stills and video with playback among them, at
//! full size and straight from the file.
//!
//! The panel is shared by the process and shows what the first object in the
//! window's responder chain that accepts control hands it. Photos' window ends
//! its chain with a controller for that. It answers the panel with the one file
//! on show, and sends the keys the panel does not use back to the window, so
//! while the panel is up the arrows still move the cursor, the number keys
//! still tag, and the preview follows the cursor.

#[cfg(target_os = "macos")]
pub use macos::QuickLook;
#[cfg(not(target_os = "macos"))]
pub use unsupported::QuickLook;

#[cfg(target_os = "macos")]
mod macos {
	use std::cell::RefCell;
	use std::path::{Path, PathBuf};

	use gpui::Window;
	use objc2::rc::Retained;
	use objc2::runtime::{AnyObject, ProtocolObject};
	use objc2::{
		define_class, msg_send, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
	};
	use objc2_app_kit::{NSEvent, NSEventType, NSResponder, NSView, NSWindow, NSWindowDelegate};
	use objc2_foundation::{NSInteger, NSObjectProtocol, NSString, NSURL};
	use objc2_quick_look_ui::{
		QLPreviewItem, QLPreviewPanel, QLPreviewPanelDataSource, QLPreviewPanelDelegate,
	};
	use raw_window_handle::{HasWindowHandle, RawWindowHandle};

	struct Ivars {
		/// The file on show.
		item: RefCell<Option<Retained<NSURL>>>,
		/// The window's view, which gets back the keys the panel does not use.
		view: Retained<NSView>,
	}

	define_class!(
		// SAFETY: NSResponder has no subclassing requirements, and the
		// controller does not implement `Drop`.
		#[unsafe(super(NSResponder))]
		#[thread_kind = MainThreadOnly]
		#[name = "SDPhotosQuickLookController"]
		#[ivars = Ivars]
		struct Controller;

		impl Controller {
			#[unsafe(method(acceptsPreviewPanelControl:))]
			fn accepts_preview_panel_control(&self, _panel: Option<&QLPreviewPanel>) -> bool {
				true
			}

			#[unsafe(method(beginPreviewPanelControl:))]
			fn begin_preview_panel_control(&self, panel: Option<&QLPreviewPanel>) {
				let Some(panel) = panel else {
					return;
				};
				let this: &AnyObject = self.as_ref();
				// SAFETY: the panel keeps neither reference alive. The
				// controller outlives its control: [`QuickLook`] holds it while
				// it is in the window's responder chain, and on drop takes it
				// out and has the panel look again, which ends this control.
				unsafe {
					panel.setDataSource(Some(ProtocolObject::from_ref(self)));
					panel.setDelegate(Some(this));
				}
			}

			#[unsafe(method(endPreviewPanelControl:))]
			fn end_preview_panel_control(&self, panel: Option<&QLPreviewPanel>) {
				if let Some(panel) = panel {
					// SAFETY: clearing the references only ever removes them.
					unsafe {
						panel.setDataSource(None);
						panel.setDelegate(None);
					}
				}
			}
		}

		unsafe impl NSObjectProtocol for Controller {}

		unsafe impl QLPreviewPanelDataSource for Controller {
			#[unsafe(method(numberOfPreviewItemsInPreviewPanel:))]
			fn number_of_preview_items(&self, _panel: Option<&QLPreviewPanel>) -> NSInteger {
				NSInteger::from(self.ivars().item.borrow().is_some())
			}

			#[unsafe(method_id(previewPanel:previewItemAtIndex:))]
			fn preview_item(
				&self,
				_panel: Option<&QLPreviewPanel>,
				_index: NSInteger,
			) -> Option<Retained<ProtocolObject<dyn QLPreviewItem>>> {
				self.ivars()
					.item
					.borrow()
					.clone()
					.map(ProtocolObject::from_retained)
			}
		}

		unsafe impl NSWindowDelegate for Controller {}

		unsafe impl QLPreviewPanelDelegate for Controller {
			#[unsafe(method(previewPanel:handleEvent:))]
			fn handle_event(&self, _panel: Option<&QLPreviewPanel>, event: Option<&NSEvent>) -> bool {
				let view = &self.ivars().view;
				match event.map(|event| (event, event.r#type())) {
					Some((event, NSEventType::KeyDown)) => {
						view.keyDown(event);
						true
					}
					Some((event, NSEventType::KeyUp)) => {
						view.keyUp(event);
						true
					}
					_ => false,
				}
			}
		}
	);

	/// A window's hold on the shared preview panel.
	pub struct QuickLook {
		controller: Retained<Controller>,
		window: Retained<NSWindow>,
		/// The file on show, so a cursor that has not moved costs nothing.
		shown: RefCell<Option<PathBuf>>,
		mtm: MainThreadMarker,
	}

	impl QuickLook {
		/// Put a controller at the end of `window`'s responder chain. `None`
		/// off the main thread, or for a window with no AppKit view.
		pub fn attach(window: &Window) -> Option<Self> {
			let mtm = MainThreadMarker::new()?;
			// gpui's own `window_handle` shadows the trait's.
			let RawWindowHandle::AppKit(handle) =
				HasWindowHandle::window_handle(window).ok()?.as_raw()
			else {
				return None;
			};
			// SAFETY: the handle names the window's live content view.
			let view: Retained<NSView> =
				unsafe { Retained::retain(handle.ns_view.as_ptr().cast())? };
			let ns_window = view.window()?;
			let controller = Controller::alloc(mtm).set_ivars(Ivars {
				item: RefCell::new(None),
				view,
			});
			// SAFETY: NSResponder's designated initializer.
			let controller: Retained<Controller> = unsafe { msg_send![super(controller), init] };
			// SAFETY: a responder's next responder is not retained. The
			// controller lives in this struct, which unlinks it on drop.
			unsafe {
				controller.setNextResponder(ns_window.nextResponder().as_deref());
				ns_window.setNextResponder(Some(controller.as_super()));
			}
			Some(QuickLook {
				controller,
				window: ns_window,
				shown: RefCell::new(None),
				mtm,
			})
		}

		fn panel(&self) -> Option<Retained<QLPreviewPanel>> {
			// SAFETY: called on the main thread, as the marker proves.
			unsafe { QLPreviewPanel::sharedPreviewPanel(self.mtm) }
		}

		pub fn is_open(&self) -> bool {
			// SAFETY: as in [`Self::panel`]. Asking first keeps a window that
			// never previewed from creating the panel.
			let exists = unsafe { QLPreviewPanel::sharedPreviewPanelExists(self.mtm) };
			exists && self.panel().is_some_and(|panel| panel.isVisible())
		}

		/// Show the file at `path`, opening the panel if it is closed.
		pub fn show(&self, path: &Path) {
			let Some(panel) = self.panel() else {
				return;
			};
			let open = panel.isVisible();
			if open && self.shown.borrow().as_deref() == Some(path) {
				return;
			}
			let Some(path_str) = path.to_str() else {
				return;
			};
			let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
			*self.controller.ivars().item.borrow_mut() = Some(url);
			*self.shown.borrow_mut() = Some(path.to_path_buf());
			if open {
				// SAFETY: this window's controller is the one in control.
				unsafe { panel.reloadData() };
			} else {
				panel.makeKeyAndOrderFront(None);
			}
		}

		pub fn close(&self) {
			if self.is_open() {
				if let Some(panel) = self.panel() {
					panel.orderOut(None);
				}
			}
		}
	}

	impl Drop for QuickLook {
		fn drop(&mut self) {
			self.close();
			// SAFETY: the window goes back to answering through whatever
			// followed it before the controller was put in.
			unsafe {
				self.window
					.setNextResponder(self.controller.nextResponder().as_deref());
			}
			// The panel holds its controller unretained, so it has to let go
			// before the controller goes. Searching the chain again, which no
			// longer holds this one, ends its control.
			// SAFETY: as in [`Self::panel`].
			if unsafe { QLPreviewPanel::sharedPreviewPanelExists(self.mtm) } {
				if let Some(panel) = self.panel() {
					unsafe { panel.updateController() };
				}
			}
		}
	}
}

#[cfg(not(target_os = "macos"))]
mod unsupported {
	use std::path::Path;

	use gpui::Window;

	/// Where the platform has no preview panel, there is nothing to hold.
	pub struct QuickLook;

	impl QuickLook {
		pub fn attach(_window: &Window) -> Option<Self> {
			None
		}

		pub fn is_open(&self) -> bool {
			false
		}

		pub fn show(&self, _path: &Path) {}

		pub fn close(&self) {}
	}
}
