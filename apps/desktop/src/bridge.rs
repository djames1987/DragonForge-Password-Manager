use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::{BrowserCredential, BrowserLoginSummary, DesktopError, DesktopService};

pub const BROWSER_PROTOCOL_VERSION: u16 = 1;
pub const NATIVE_HOST_NAME: &str = "com.dragonforge.passwordmanager";
const MAX_BRIDGE_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEndpoint {
    pub version: u16,
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeEnvelope {
    token: String,
    request: BrowserRequest,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRequest {
    pub version: u16,
    #[serde(flatten)]
    pub action: BrowserAction,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum BrowserAction {
    Status,
    Search {
        #[serde(rename = "pageUrl")]
        page_url: String,
        query: Option<String>,
    },
    Credential {
        #[serde(rename = "pageUrl")]
        page_url: String,
        #[serde(rename = "itemId")]
        item_id: String,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub connected: bool,
    pub unlocked: bool,
    pub item_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserError {
    pub code: String,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserResponse {
    pub version: u16,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<BrowserStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<BrowserLoginSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<BrowserCredential>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BrowserError>,
}

impl BrowserResponse {
    fn status(unlocked: bool, item_count: usize) -> Self {
        Self {
            version: BROWSER_PROTOCOL_VERSION,
            ok: true,
            status: Some(BrowserStatus {
                connected: true,
                unlocked,
                item_count,
            }),
            items: None,
            credential: None,
            error: None,
        }
    }

    fn items(items: Vec<BrowserLoginSummary>) -> Self {
        Self {
            version: BROWSER_PROTOCOL_VERSION,
            ok: true,
            status: None,
            items: Some(items),
            credential: None,
            error: None,
        }
    }

    fn credential(credential: BrowserCredential) -> Self {
        Self {
            version: BROWSER_PROTOCOL_VERSION,
            ok: true,
            status: None,
            items: None,
            credential: Some(credential),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            version: BROWSER_PROTOCOL_VERSION,
            ok: false,
            status: None,
            items: None,
            credential: None,
            error: Some(BrowserError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

pub struct BrowserBridge {
    endpoint_path: PathBuf,
}

impl BrowserBridge {
    pub fn start(service: DesktopService) -> io::Result<Self> {
        Self::start_at(service, bridge_endpoint_path()?)
    }

    pub fn start_at(service: DesktopService, endpoint_path: PathBuf) -> io::Result<Self> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let token = random_token();
        write_endpoint(
            &endpoint_path,
            &BridgeEndpoint {
                version: BROWSER_PROTOCOL_VERSION,
                port,
                token: token.clone(),
                pid: std::process::id(),
            },
        )?;

        thread::Builder::new()
            .name("dragonforge-browser-bridge".to_owned())
            .spawn(move || {
                for incoming in listener.incoming() {
                    match incoming {
                        Ok(stream) => {
                            let service = service.clone();
                            let token = token.clone();
                            let _ = thread::Builder::new()
                                .name("dragonforge-browser-client".to_owned())
                                .spawn(move || handle_bridge_stream(stream, &service, &token));
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Self { endpoint_path })
    }
}

impl Drop for BrowserBridge {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.endpoint_path);
    }
}

#[allow(clippy::needless_return)]
pub fn bridge_endpoint_path() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("DRAGONFORGE_BRIDGE_ENDPOINT") {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not available")
        })?;
        return Ok(PathBuf::from(base)
            .join("DragonForge Password Manager")
            .join("bridge.json"));
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not available"))?;
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("DragonForge Password Manager")
            .join("bridge.json"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
            return Ok(PathBuf::from(runtime)
                .join("dragonforge-password-manager")
                .join("bridge.json"));
        }
        let home = std::env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not available"))?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("dragonforge-password-manager")
            .join("bridge.json"))
    }
}

pub fn read_endpoint(path: &Path) -> io::Result<BridgeEndpoint> {
    let bytes = fs::read(path)?;
    let endpoint: BridgeEndpoint = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if endpoint.version != BROWSER_PROTOCOL_VERSION || endpoint.token.len() != 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported or malformed DragonForge bridge endpoint",
        ));
    }
    Ok(endpoint)
}

pub fn forward_native_request(request: &BrowserRequest) -> BrowserResponse {
    let path = match bridge_endpoint_path() {
        Ok(path) => path,
        Err(error) => {
            return BrowserResponse::error(
                "desktopUnavailable",
                format!("DragonForge desktop bridge path is unavailable: {error}"),
            );
        }
    };
    let endpoint = match read_endpoint(&path) {
        Ok(endpoint) => endpoint,
        Err(_) => {
            return BrowserResponse::error(
                "desktopUnavailable",
                "DragonForge desktop is not running or browser integration is unavailable",
            );
        }
    };

    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, endpoint.port);
    let mut stream = match TcpStream::connect_timeout(&address.into(), Duration::from_secs(2)) {
        Ok(stream) => stream,
        Err(_) => {
            return BrowserResponse::error(
                "desktopUnavailable",
                "DragonForge desktop is not reachable",
            );
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let envelope = serde_json::json!({
        "token": endpoint.token,
        "request": request,
    });
    let mut encoded = match serde_json::to_vec(&envelope) {
        Ok(encoded) => encoded,
        Err(error) => return BrowserResponse::error("protocolError", error.to_string()),
    };
    if encoded.len() > MAX_BRIDGE_MESSAGE_BYTES {
        return BrowserResponse::error("requestTooLarge", "browser bridge request is too large");
    }
    encoded.push(b'\n');

    if stream.write_all(&encoded).is_err() || stream.flush().is_err() {
        return BrowserResponse::error(
            "desktopUnavailable",
            "failed to contact DragonForge desktop",
        );
    }

    let mut reader = BufReader::new(stream);
    let mut response = Vec::new();
    match reader
        .by_ref()
        .take((MAX_BRIDGE_MESSAGE_BYTES + 1) as u64)
        .read_until(b'\n', &mut response)
    {
        Ok(0) | Err(_) => {
            return BrowserResponse::error(
                "desktopUnavailable",
                "DragonForge desktop returned no response",
            );
        }
        Ok(_) => {}
    }
    if response.len() > MAX_BRIDGE_MESSAGE_BYTES || !response.ends_with(b"\n") {
        return BrowserResponse::error("protocolError", "invalid DragonForge desktop response");
    }
    response.pop();

    serde_json::from_slice(&response).unwrap_or_else(|_| {
        BrowserResponse::error("protocolError", "invalid DragonForge desktop response")
    })
}

fn handle_bridge_stream(mut stream: TcpStream, service: &DesktopService, expected_token: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let cloned = match stream.try_clone() {
        Ok(cloned) => cloned,
        Err(_) => return,
    };
    let mut reader = BufReader::new(cloned);
    let mut request = Vec::new();
    let read = reader
        .by_ref()
        .take((MAX_BRIDGE_MESSAGE_BYTES + 1) as u64)
        .read_until(b'\n', &mut request);
    if read.is_err() || request.len() > MAX_BRIDGE_MESSAGE_BYTES || !request.ends_with(b"\n") {
        let _ = write_bridge_response(
            &mut stream,
            &BrowserResponse::error("protocolError", "invalid bridge request"),
        );
        return;
    }
    request.pop();

    let envelope: BridgeEnvelope = match serde_json::from_slice(&request) {
        Ok(envelope) => envelope,
        Err(_) => {
            let _ = write_bridge_response(
                &mut stream,
                &BrowserResponse::error("protocolError", "invalid bridge request"),
            );
            return;
        }
    };

    if !tokens_equal(&envelope.token, expected_token) {
        let _ = write_bridge_response(
            &mut stream,
            &BrowserResponse::error("unauthorized", "browser bridge authentication failed"),
        );
        return;
    }

    let response = handle_browser_request(service, envelope.request);
    let _ = write_bridge_response(&mut stream, &response);
    let _ = stream.shutdown(Shutdown::Both);
}

fn handle_browser_request(service: &DesktopService, request: BrowserRequest) -> BrowserResponse {
    if request.version != BROWSER_PROTOCOL_VERSION {
        return BrowserResponse::error(
            "unsupportedVersion",
            "unsupported browser protocol version",
        );
    }

    match request.action {
        BrowserAction::Status => match service.status() {
            Ok(status) => BrowserResponse::status(status.unlocked, status.item_count),
            Err(error) => map_desktop_error(error),
        },
        BrowserAction::Search { page_url, query } => {
            match service.browser_search(&page_url, query.as_deref()) {
                Ok(items) => BrowserResponse::items(items),
                Err(error) => map_desktop_error(error),
            }
        }
        BrowserAction::Credential { page_url, item_id } => {
            match service.browser_credential(&item_id, &page_url) {
                Ok(credential) => BrowserResponse::credential(credential),
                Err(error) => map_desktop_error(error),
            }
        }
    }
}

fn map_desktop_error(error: DesktopError) -> BrowserResponse {
    match error {
        DesktopError::Locked => BrowserResponse::error(
            "vaultLocked",
            "Unlock DragonForge Password Manager to use browser filling",
        ),
        DesktopError::InvalidInput(message) => BrowserResponse::error("invalidRequest", message),
        DesktopError::StateUnavailable => BrowserResponse::error(
            "desktopUnavailable",
            "DragonForge desktop state is unavailable",
        ),
        DesktopError::Vault(error) => BrowserResponse::error("vaultError", error.to_string()),
        DesktopError::Sync(_) => BrowserResponse::error(
            "desktopUnavailable",
            "DragonForge desktop synchronization state is unavailable",
        ),
        DesktopError::InvalidAccountSecret => {
            BrowserResponse::error("invalidRequest", "invalid Account Secret")
        }
    }
}

fn write_bridge_response(stream: &mut TcpStream, response: &BrowserResponse) -> io::Result<()> {
    let mut encoded = serde_json::to_vec(response)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if encoded.len() > MAX_BRIDGE_MESSAGE_BYTES {
        encoded = serde_json::to_vec(&BrowserResponse::error(
            "responseTooLarge",
            "browser bridge response exceeded the size limit",
        ))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    encoded.push(b'\n');
    stream.write_all(&encoded)?;
    stream.flush()
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn tokens_equal(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }

    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn write_endpoint(path: &Path, endpoint: &BridgeEndpoint) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bytes = serde_json::to_vec(endpoint)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file: File = options.open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_requires_exact_match() {
        assert!(tokens_equal("abc", "abc"));
        assert!(!tokens_equal("abc", "abd"));
        assert!(!tokens_equal("abc", "ab"));
    }

    #[test]
    fn browser_request_deserializes_extension_camel_case_fields() {
        let search: BrowserRequest = serde_json::from_str(
            r#"{"version":1,"action":"search","pageUrl":"https://example.com/login","query":"alice"}"#,
        )
        .unwrap();
        match search.action {
            BrowserAction::Search { page_url, query } => {
                assert_eq!(page_url, "https://example.com/login");
                assert_eq!(query.as_deref(), Some("alice"));
            }
            _ => panic!("expected search request"),
        }

        let credential: BrowserRequest = serde_json::from_str(
            r#"{"version":1,"action":"credential","pageUrl":"https://example.com/login","itemId":"item-1"}"#,
        )
        .unwrap();
        match credential.action {
            BrowserAction::Credential { page_url, item_id } => {
                assert_eq!(page_url, "https://example.com/login");
                assert_eq!(item_id, "item-1");
            }
            _ => panic!("expected credential request"),
        }
    }

    #[test]
    fn protocol_rejects_unknown_version() {
        let service = DesktopService::default();
        let response = handle_browser_request(
            &service,
            BrowserRequest {
                version: 99,
                action: BrowserAction::Status,
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "unsupportedVersion");
    }
}
