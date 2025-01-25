use appearance::Appearance;

use anyhow::Error;
use std::fs::File;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;

fn handle_client(mut stream: TcpStream) -> Result<(), Error> {
    let buf: &mut [u8; 100] = &mut [0; 100];
    let mut file = File::create("foo")?;
    let len = stream.read(buf)?;
    let str = String::from_utf8(buf[0..len].to_vec());
    file.write_all(&buf[0..len])?;

    println!("wrote: {:?}", str);
    Ok(())
}

pub fn internal_main() {
    let _ = Appearance::new("Render Host");

    let listener = TcpListener::bind("127.0.0.1:34234").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || handle_client(stream));
            }
            Err(_) => {
                break;
            }
        }
    }
}
