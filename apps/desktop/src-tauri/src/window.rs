use tauri::{GlobalWindowEvent, Runtime, Window, Wry};

pub(crate) fn handle_window_event(event: GlobalWindowEvent<Wry>) {
  match event.event() {
    tauri::WindowEvent::Focused(_focus) => {
      // let fullscreen = event.window().is_fullscreen().unwrap_or(false);

      // println!("fullscreen, {}", fullscreen);

      // #[cfg(target_os = "macos")]
      // event
      //   .window()
      //   .set_transparent_titlebar(!fullscreen, !fullscreen);
    }
    _ => {}
  }
}

pub trait WindowExt {
  #[cfg(target_os = "macos")]
  fn set_toolbar(&self, shown: bool);
  #[cfg(target_os = "macos")]
  fn set_transparent_titlebar(&self, transparent: bool, large: bool);
  #[cfg(target_os = "macos")]
  fn fix_shadow(&self);
}

impl<R: Runtime> WindowExt for Window<R> {
  #[cfg(target_os = "macos")]
  fn set_toolbar(&self, shown: bool) {
    use cocoa::{
      appkit::{NSToolbar, NSWindow},
      base::nil,
      foundation::NSString,
    };

    unsafe {
      let id = self.ns_window().unwrap() as cocoa::base::id;

      if shown {
        let toolbar =
          NSToolbar::alloc(nil).initWithIdentifier_(NSString::alloc(nil).init_str("wat"));
        toolbar.setShowsBaselineSeparator_(false);
        id.setToolbar_(toolbar);
      } else {
        id.setToolbar_(nil);
      }
    }
  }

  #[cfg(target_os = "macos")]
  fn set_transparent_titlebar(&self, transparent: bool, large: bool) {
    use cocoa::appkit::{NSWindow, NSWindowStyleMask, NSWindowTitleVisibility};

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

      id.setTitlebarAppearsTransparent_(if transparent {
        cocoa::base::YES
      } else {
        cocoa::base::NO
      });
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
