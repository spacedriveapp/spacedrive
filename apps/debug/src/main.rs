use sdcorelib::sys::volumes::get;

fn main() {
    let mounts = get().unwrap();
    for mount in mounts {
        println!("{:?}", mount);
    }
}
