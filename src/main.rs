use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    process::Command,
};

const SOCKET_PATH: &str = "/tmp/mendess/aramanthinewall.socket";

#[derive(Serialize, Deserialize)]
struct HyprctlCmd {
    sub_command: String,
    args: Vec<String>,
}

fn handle_connection(mut connection: UnixStream) -> io::Result<()> {
    println!("handling connection");
    let mut reader = BufReader::new(&connection);
    let mut buffer = String::new();
    reader.read_line(&mut buffer)?;
    let command = serde_json::from_str::<HyprctlCmd>(&buffer)?;
    println!("running command: {buffer:?}");
    let status = match command.sub_command.as_str() {
        "reload-hyprland" => {
            std::fs::write("/tmp/reload-hyprland", "1")?;
            Command::new("hyprctl")
                .args(["dispatch", "exit"])
                .status()?
        }
        other => Command::new("hyprctl")
            .arg(other)
            .args(command.args)
            .status()?,
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

fn request(cmd: HyprctlCmd) -> io::Result<()> {
    let mut connection = UnixStream::connect(SOCKET_PATH)?;
    let bytes = serde_json::to_vec(&cmd).unwrap();
    connection.write_all(&bytes)?;
    connection.write_all(b"\n")?;
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
            let exec = dbg!(format!(r#"hl.dsp.exec_cmd('{}')"#, args[2..].join(" ")));
            request(HyprctlCmd {
                sub_command: "dispatch".to_string(),
                args: vec![exec],
            })
        }
        _ => request(HyprctlCmd {
            sub_command: args[1].to_string(),
            args: args[2..].to_vec(),
        }),
    };
    if let Err(e) = err {
        eprintln!("request failed: {e:?}");
    }
}
