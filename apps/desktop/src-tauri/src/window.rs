use tauri::{Runtime, Window};

pub trait WindowExt {
  #[cfg(target_os = "macos")]
  fn set_transparent_titlebar(&self, transparent: bool);
}

impl<R: Runtime> WindowExt for Window<R> {
  #[cfg(target_os = "macos")]
  fn set_transparent_titlebar(&self, transparent: bool) {
    use cocoa::{appkit::{NSWindow, NSWindowStyleMask, NSWindowTitleVisibility, NSToolbar}, foundation::NSString, base::nil};

    unsafe {
      let id = self.ns_window().unwrap() as cocoa::base::id;

      let mut style_mask = id.styleMask();
      style_mask.set(
        NSWindowStyleMask::NSFullSizeContentViewWindowMask | NSWindowStyleMask::NSUnifiedTitleAndToolbarWindowMask,
        transparent,
      );
      id.setStyleMask_(style_mask);

      // TODO: figure out if this is how to correctly hide the toolbar in full screen
      // and if so, figure out why tf it panics:

      // let mut presentation_options = id.presentationOptions_();
      // presentation_options.set(
      //   NSApplicationPresentationOptions::NSApplicationPresentationAutoHideToolbar,
      //   transparent,
      // );
      // id.setPresentationOptions_(presentation_options);

      let toolbar = NSToolbar::alloc(nil).initWithIdentifier_(NSString::alloc(nil).init_str("wat"));
      toolbar.setShowsBaselineSeparator_(false);
      id.setToolbar_(toolbar);

      id.setTitleVisibility_(if transparent {
        NSWindowTitleVisibility::NSWindowTitleHidden
      } else {
        NSWindowTitleVisibility::NSWindowTitleVisible
      });

      id.setTitlebarAppearsTransparent_(if transparent {
        cocoa::base::YES
      } else {
        cocoa::base::NO
      });
    }
  }
}

