use std::net::{TcpListener, TcpStream};
use std::thread;

use autodiscover_rs::{self, Method};
use env_logger;

fn handle_client(stream: std::io::Result<TcpStream>) {
    println!("Got a connection from {:?}", stream.unwrap().peer_addr());
}

pub fn listen() -> std::io::Result<()> {
    env_logger::init();
    // make sure to bind before announcing ready
    let listener = TcpListener::bind(":::0")?;
    // get the port we were bound too; note that the trailing :0 above gives us a random unused port
    let socket = listener.local_addr()?;
    thread::spawn(move || {
        // this function blocks forever; running it a separate thread
        autodiscover_rs::run(
            &socket,
            Method::Multicast("[ff0e::1]:1337".parse().unwrap()),
            |s| {
                // change this to task::spawn if using async_std or tokio
                thread::spawn(|| handle_client(s));
            },
        )
        .unwrap();
    });
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next() {
        // if you are using an async library, such as async_std or tokio, you can convert the stream to the
        // appropriate type before using task::spawn from your library of choice.
        thread::spawn(|| handle_client(stream));
    }
    Ok(())
}
