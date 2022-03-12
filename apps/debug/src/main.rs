use sdcorelib::sys::volumes::get;

fn main() {
    let mounts = match get() {
        Ok(mounts) => mounts,
        Err(e) => {
            dbg!(e);
            return;
        }
    };

    for mount in mounts {
        dbg!(mount);
    }
}
