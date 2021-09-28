// one possible implementation of walking a directory only visiting files
// pub fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> io::Result<()> {
//   if dir.is_dir() {
//     for entry in fs::read_dir(dir)? {
//       let entry = entry?;
//       let path = entry.path();
//       if path.is_dir() {
//         visit_dirs(&path, cb)?;
//       } else {
//         cb(&entry);
//       }
//     }
//   }
//   Ok(())
// }

// pub fn current_dir() -> io::Result<()> {
//   let raw_entries = fs::read_dir(".")?
//     .map(|res| res.map(|e| e.path()))
//     .collect::<Result<Vec<_>, io::Error>>()?;

//   println!("Entries: {:?}", raw_entries);

//   Ok(())
// }
