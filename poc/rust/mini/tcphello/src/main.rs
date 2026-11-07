use std::env;
use std::error::Error;
use std::io::{BufRead, BufReader, Error as stdError, Write, stdin};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("client") => main_client(),
        Some("server") => main_server(),
        _ => {
            eprintln!("Usage: tcphello [client|server]");
            std::process::exit(1);
        }
    }
}

fn main_server() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:7878";
    let listener = TcpListener::bind(addr)?;
    println!("[Server listening on {addr}]");

    let clients = Arc::new(Mutex::new(Vec::<TcpStream>::new()));

    for stream in listener.incoming() {
        let stream = stream?;

        let clients = Arc::clone(&clients);
        clients
            .lock()
            .map_err(|e| stdError::other(format!("Mutex poisoned: {}", e)))?
            .push(stream.try_clone()?);

        thread::spawn(move || {
            match server_handle_clients(stream, clients) {
                Ok(_) => (),
                Err(e) => eprintln!("[Client Error: {}]", e),
            };
        });
    }

    Ok(())
}

fn server_handle_clients(
    mut stream: TcpStream,
    clients: Arc<Mutex<Vec<TcpStream>>>,
) -> Result<(), std::io::Error> {
    let peer_addr = stream.peer_addr()?;
    println!("[New client: {}]", peer_addr);

    let reader = BufReader::new(&mut stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        println!("<< {}", line);

        let clients = clients
            .lock()
            .map_err(|e| stdError::other(format!("Mutex poisoned: {}", e)))?;

        for mut c in clients.iter() {
            if c.peer_addr()? != peer_addr {
                c.write_all(line.as_bytes())?;
                c.write_all(b"\n")?;
            }
        }
    }

    println!("[Client disconnected: {}]", peer_addr);
    clients
        .lock()
        .map_err(|e| stdError::other(format!("Mutex poisoned: {}", e)))?
        .retain(|c| match c.peer_addr() {
            Ok(addr) => addr != peer_addr,
            Err(_) => false,
        });

    Ok(())
}

fn main_client() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:7878")?;
    let mut stream_for_read = stream.try_clone()?;

    thread::spawn(move || {
        let reader = BufReader::new(&mut stream_for_read);
        for line in reader.lines() {
            match line {
                Ok(l) => println!("<< {}", l),
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    loop {
        let mut s = String::new();
        stdin().lock().read_line(&mut s)?;
        stream.write_all(s.as_bytes())?;
    }
}
