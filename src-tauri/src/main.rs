#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]
mod app;
mod commands;
mod crypto;
mod db;
mod device;
mod filesystem;
mod util;
use crate::app::menu;
use env_logger;
use futures::executor::block_on;
// use systemstat::{saturating_sub_bytes, Platform, System};

#[derive(Debug)]
#[repr(C)]
struct FFIString {
  // _phantom: u8,
  data: *const u8,
  length: u64,
}

#[derive(Debug)]
#[repr(C)]
struct FFIData {
  // _phantom: u8,
  data: *const u8,
  length: u64,
}

extern "C" {
  fn get_file_thumbnail(ptr: *const u8, length: u64) -> FFIData;
  fn test(ptr: *const u8, length: u64) -> FFIData;
}

fn main() {
  let path = "/Users/jamie/Downloads/Audio Hijack.app";
  // let string = FFIString {
  //   data: path.as_ptr(),
  //   length: path.len(),
  // };
  // let thumbnail = unsafe {
  //   println!("{:?}", string);
  //   get_file_thumbnail(&string)
  // };

  println!("Struct Data: {:?}", unsafe {
    let res = test(path.as_ptr(), path.len() as u64);

    let mut vec = Vec::<u8>::new();

    let mut pointer_str = String::new();

    for j in 8..16 {
      let num = *(res.data.add(j));
      vec.push(num);

      pointer_str = format!("{:x}", num) + &pointer_str;
    }

    let pointer_num = u64::from_str_radix(&pointer_str, 16).unwrap();
    let pointer = pointer_num as *const u8;

    let mut length = 0;
    for j in 16..24 {
      let num = *(res.data.add(j));
      length += num as usize * 256_usize.pow(j as u32 - 16_u32)
    }

    let mut data = Vec::new();

    for i in 0..length {
      data.push(*pointer.add(i));
    }

    res
  });
  // unsafe { get_icon("/Users/jamie/Downloads/Audio Hijack.app") }
  // let mounts = device::volumes_c::get_mounts();
  // println!("mounted drives: {:?}", &mounts);
  // env_logger::builder()
  //   .filter_level(log::LevelFilter::Debug)
  //   .is_test(true)
  //   .init();

  // create primary data base if not exists
  block_on(db::connection::create_primary_db()).unwrap();
  // init filesystem and create library if missing
  block_on(filesystem::init::init_library()).unwrap();

  // block_on(filesystem::device::discover_storage_devices()).unwrap();

  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      commands::scan_dir,
      commands::get_files
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
