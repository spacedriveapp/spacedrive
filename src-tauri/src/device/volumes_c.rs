use bytesize::ByteSize;
use libc::{c_int, statfs};
use std::{ffi, io, ptr, slice};

#[derive(Debug)]
pub struct FileSystem {
  files: usize,
  files_total: usize,
  files_avail: usize,
  free: ByteSize,
  avail: ByteSize,
  total: ByteSize,
  name_max: u32,
  fs_type: String,
  fs_mounted_from: String,
  fs_mounted_on: String,
}

pub fn get_mounts() -> io::Result<Vec<FileSystem>> {
  let mut mptr: *mut statfs = ptr::null_mut();
  let len = unsafe { getmntinfo(&mut mptr, 2 as i32) };
  if len < 1 {
    return Err(io::Error::new(io::ErrorKind::Other, "getmntinfo() failed"));
  }
  let mounts = unsafe { slice::from_raw_parts(mptr, len as usize) };
  Ok(mounts.iter().map(statfs_to_fs).collect::<Vec<_>>())
}

fn statfs_to_fs(x: &statfs) -> FileSystem {
  x.f_fsid;
  FileSystem {
    files: (x.f_files as usize).saturating_sub(x.f_ffree as usize),
    files_total: x.f_files as usize,
    files_avail: x.f_ffree as usize,
    free: ByteSize::b(x.f_bfree * x.f_bsize as u64),
    avail: ByteSize::b(x.f_bavail * x.f_bsize as u64),
    total: ByteSize::b(x.f_blocks * x.f_bsize as u64),
    name_max: 256,
    fs_type: unsafe {
      ffi::CStr::from_ptr(&x.f_fstypename[0])
        .to_string_lossy()
        .into_owned()
    },
    fs_mounted_from: unsafe {
      ffi::CStr::from_ptr(&x.f_mntfromname[0])
        .to_string_lossy()
        .into_owned()
    },
    fs_mounted_on: unsafe {
      ffi::CStr::from_ptr(&x.f_mntonname[0])
        .to_string_lossy()
        .into_owned()
    },
  }
}

#[link(name = "c")]
extern "C" {
  #[link_name = "getmntinfo$INODE64"]
  fn getmntinfo(mntbufp: *mut *mut statfs, flags: c_int) -> c_int;
}
