use appearance::Appearance;

use std::io::prelude::*;
use std::net::TcpStream;

pub fn internal_main() {
    let _appearance = Appearance::new("Render Node");

    let mut client = TcpStream::connect("127.0.0.1:34234").unwrap();
    match client.write(b"I'm a teapot!") {
        Ok(len) => println!("wrote {} bytes", len),
        Err(e) => println!("error parsing header: {:?}", e),
    }
}
