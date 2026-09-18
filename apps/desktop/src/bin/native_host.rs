use std::{
    io::{self, Read, Write},
    process::ExitCode,
};

use dragonforge_desktop::{
    BROWSER_PROTOCOL_VERSION, BrowserRequest, BrowserResponse, forward_native_request,
};
use zeroize::Zeroize;

const MAX_NATIVE_MESSAGE_BYTES: usize = 1024 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn run() -> io::Result<()> {
    let origin = std::env::args().nth(1).unwrap_or_default();
    if !valid_extension_origin(&origin) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native host caller is not a browser extension",
        ));
    }

    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();

    while let Some(mut message) = read_native_message(&mut input)? {
        let response = match serde_json::from_slice::<BrowserRequest>(&message) {
            Ok(request) if request.version == BROWSER_PROTOCOL_VERSION => {
                forward_native_request(&request)
            }
            Ok(_) => {
                BrowserResponse::error("unsupportedVersion", "unsupported browser protocol version")
            }
            Err(_) => BrowserResponse::error("protocolError", "invalid native messaging request"),
        };
        message.zeroize();

        let mut encoded = serde_json::to_vec(&response)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_native_message(&mut output, &encoded)?;
        encoded.zeroize();
    }

    Ok(())
}

fn read_native_message(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0_u8; 4];
    match reader.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }

    let length = u32::from_ne_bytes(length) as usize;
    if length == 0 || length > MAX_NATIVE_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native messaging request exceeds DragonForge limits",
        ));
    }

    let mut message = vec![0_u8; length];
    reader.read_exact(&mut message)?;
    Ok(Some(message))
}

fn write_native_message(writer: &mut impl Write, message: &[u8]) -> io::Result<()> {
    if message.len() > MAX_NATIVE_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native messaging response exceeds DragonForge limits",
        ));
    }

    writer.write_all(&(message.len() as u32).to_ne_bytes())?;
    writer.write_all(message)?;
    writer.flush()
}

fn valid_extension_origin(origin: &str) -> bool {
    (origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://"))
        && !origin.contains('\n')
        && !origin.contains('\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_message_round_trip() {
        let payload = br#"{"version":1,"action":"status"}"#;
        let mut framed = Vec::new();
        write_native_message(&mut framed, payload).unwrap();

        let decoded = read_native_message(&mut framed.as_slice())
            .unwrap()
            .unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn native_origin_requires_extension_scheme() {
        assert!(valid_extension_origin("chrome-extension://abcdef/"));
        assert!(valid_extension_origin("moz-extension://abcdef/"));
        assert!(!valid_extension_origin("https://example.com"));
        assert!(!valid_extension_origin(""));
    }
}
