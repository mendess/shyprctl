use std::{
    io::{self, BufRead, BufReader, Read, Write},
    iter::repeat,
    os::unix::net::{UnixListener, UnixStream},
    process::Command,
};

const SOCKET_PATH: &str = "/tmp/mendess/aramanthinewall.socket";

fn handle_connection(mut connection: UnixStream) -> io::Result<()> {
    println!("handling connection");
    let mut reader = BufReader::new(&connection);
    let mut buffer = String::new();
    reader.read_line(&mut buffer)?;
    let command = buffer.split_whitespace().collect::<Vec<_>>();
    println!("running command: {buffer:?}");
    let status = match &command[..] {
        ["reload-hyprland"] => {
            std::fs::write("/tmp/reload-hyprland", "1")?;
            Command::new("hyprctl")
                .args(["dispatch", "exit"])
                .status()?
        }
        rest => Command::new("hyprctl").args(rest).status()?,
    };
    println!("got status: {status:?}");
    connection.write_all(status.to_string().as_bytes())?;
    Ok(())
}

fn daemon() {
    const HYPR_SIGNATURE_VAR: &str = "HYPRLAND_INSTANCE_SIGNATURE";
    match std::env::var(HYPR_SIGNATURE_VAR) {
        Ok(signature) => {
            std::fs::write("/tmp/hyprland-instance-signature", signature).unwrap();
        }
        Err(e) => {
            eprintln!("{HYPR_SIGNATURE_VAR} not set! (is hyprland running?): {e:?}");
            return;
        }
    };
    let _ = std::fs::remove_file(SOCKET_PATH);
    let socket = UnixListener::bind(SOCKET_PATH).unwrap();
    for stream in socket.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("failed to accept: {e:?}");
                continue;
            }
        };
        if let Err(e) = handle_connection(stream) {
            eprintln!("failed to handle connection: {e:?}");
        }
    }
}

fn request(args: &[impl AsRef<str>]) -> io::Result<()> {
    let mut connection = UnixStream::connect(SOCKET_PATH)?;
    let mut buffer = args
        .iter()
        .map(|s| s.as_ref())
        .zip(repeat(" "))
        .flat_map(|(a, b)| [a, b])
        .collect::<String>();
    buffer.pop();
    buffer.push('\n');
    connection.write_all(buffer.as_bytes())?;
    let mut buffer = Vec::new();
    connection.read_to_end(&mut buffer)?;
    println!(
        "returned: {}",
        String::from_utf8(buffer).map_err(io::Error::other)?
    );
    Ok(())
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    if args.len() < 2 {
        return;
    }

    let err = match args[1].as_str() {
        "--version" | "-v" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "daemon" => {
            daemon();
            Ok(())
        }
        "l" | "launch" => {
            let mut opts = vec!["dispatch", "exec"];
            opts.extend(args[2..].iter().map(|x| x.as_str()));
            request(&opts)
        }
        _ => request(&args[1..]),
    };
    if let Err(e) = err {
        eprintln!("request failed: {e:?}");
    }
}
