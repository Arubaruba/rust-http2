extern crate http2;

use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io::{Read, Write};
use std::str;

use http2::request::Request;

fn handle_client<'a>(mut stream: TcpStream) {
	let mut buffer = [0u8; 1000];
	stream.read(&mut buffer).unwrap();
	let buffer_text = str::from_utf8(&buffer).unwrap();

	println!("{}", buffer_text);

    let request = Request::from_str(&buffer_text).unwrap();

	stream.write(format!("got path: {}", request.url).as_bytes()).unwrap();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    println!("Started server on port 8080");

    for stream in listener.incoming() {
        thread::spawn(|| {
            handle_client(stream.unwrap());
        });
    }

    println!("todo - start simple server");
}
