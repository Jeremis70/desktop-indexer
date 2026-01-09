use crate::ipc::{Request, Response};
use crate::xdg::socket_path;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

pub fn try_request(req: &Request) -> Option<Response> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).ok()?;
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

    let mut stream = stream;
    let line = serde_json::to_string(req).ok()? + "\n";
    stream.write_all(line.as_bytes()).ok()?;
    stream.flush().ok()?;

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).ok()?;
    if resp_line.trim().is_empty() {
        return None;
    }

    serde_json::from_str::<Response>(resp_line.trim()).ok()
}
