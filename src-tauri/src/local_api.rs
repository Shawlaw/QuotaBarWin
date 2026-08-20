use std::{
    ffi::CStr,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde::Serialize;
use tauri::AppHandle;
#[cfg(not(test))]
use tauri::Emitter;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use windows_sys::Win32::{
    Foundation::ERROR_BUFFER_OVERFLOW,
    NetworkManagement::{
        IpHelper::{GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH},
        Ndis::NET_IF_OPER_STATUS_UP,
    },
    Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6},
};

use crate::{
    app_info,
    config::{
        config_path_for_app, load_or_create_config, local_api_requires_access_token,
        LocalApiBindTarget, LocalApiSettings,
    },
    local_api_token,
    logger::{LogLevel, LogSink},
    quota::{get_cached_snapshot_from_config_path, AppSnapshot},
    redact::redact_sensitive,
};
#[cfg(not(test))]
use crate::{
    quota::build_app_snapshot_from_config_path, refresh_scheduler::SNAPSHOT_REFRESHED_EVENT,
};

const API_VERSION: u8 = 1;
const SERVER_RECV_TIMEOUT: Duration = Duration::from_millis(250);

static SERVER_MANAGER: OnceLock<Mutex<LocalApiServerManager>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiNetworkInterface {
    pub id: String,
    pub name: String,
    pub addresses: Vec<LocalApiNetworkAddress>,
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiNetworkAddress {
    pub address: String,
    pub scope_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiStatus {
    pub enabled: bool,
    pub running: bool,
    pub endpoints: Vec<String>,
    pub requires_auth: bool,
    pub token_configured: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiAccessToken {
    pub token: String,
}

#[derive(Default)]
struct LocalApiServerManager {
    active: Option<ActiveServer>,
    last_error: Option<String>,
}

struct ActiveServer {
    effective_config: EffectiveServerConfig,
    endpoints: Vec<String>,
    listeners: Vec<ActiveListener>,
}

struct ActiveListener {
    server: Arc<Server>,
    shutdown: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

#[derive(Clone, PartialEq, Eq)]
struct EffectiveServerConfig {
    config_path: std::path::PathBuf,
    listeners: Vec<EffectiveListenerConfig>,
}

#[derive(Clone, PartialEq, Eq)]
struct EffectiveListenerConfig {
    address: SocketAddr,
    token: Option<String>,
}

#[derive(Clone)]
struct RequestState {
    config_path: std::path::PathBuf,
    #[cfg(not(test))]
    app: AppHandle,
    token: Option<String>,
    refresh_in_flight: Arc<AtomicBool>,
    log: LogSink,
    #[cfg(test)]
    test_snapshot: Option<AppSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    api_version: u8,
    app_version: String,
    snapshot_available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshResponse {
    accepted: bool,
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

fn server_manager() -> &'static Mutex<LocalApiServerManager> {
    SERVER_MANAGER.get_or_init(|| Mutex::new(LocalApiServerManager::default()))
}

pub fn start(app: AppHandle) {
    reconfigure(&app);
}

pub fn reconfigure(app: &AppHandle) {
    let path = match config_path_for_app(app) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Unable to resolve local integration API config path: {error}");
            return;
        }
    };
    let loaded = match load_or_create_config(&path) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("Unable to load local integration API settings: {error}");
            return;
        }
    };
    let log = LogSink::from_config_path(&path, &loaded.config);

    if let Err(error) =
        reconfigure_from_settings(app.clone(), path, loaded.config.local_api, log.clone())
    {
        if let Ok(mut manager) = server_manager().lock() {
            stop_active_server(&mut manager);
            manager.last_error = Some(error.clone());
        }
        let _ = log.write_unfiltered(
            LogLevel::Warn,
            "local_api",
            &format!("local integration API unavailable: {error}"),
        );
    }
}

pub fn status_for_app(app: &AppHandle) -> Result<LocalApiStatus, String> {
    let path = config_path_for_app(app)?;
    let config = load_or_create_config(&path)?.config;
    let requires_auth = local_api_requires_access_token(&config.local_api);
    let token_configured = local_api_token::read_token(&path)?.is_some();
    let manager = server_manager()
        .lock()
        .map_err(|_| "Local integration API state is unavailable".to_string())?;
    let active = manager
        .active
        .as_ref()
        .filter(|active| active.effective_config.config_path == path);

    Ok(LocalApiStatus {
        enabled: config.local_api.enabled,
        running: active.is_some(),
        endpoints: active
            .map(|active| active.endpoints.clone())
            .unwrap_or_default(),
        requires_auth,
        token_configured,
        error: manager.last_error.clone(),
    })
}

#[tauri::command]
pub async fn get_local_api_status(app: AppHandle) -> Result<LocalApiStatus, String> {
    tauri::async_runtime::spawn_blocking(move || status_for_app(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn list_local_api_network_interfaces() -> Result<Vec<LocalApiNetworkInterface>, String> {
    tauri::async_runtime::spawn_blocking(list_network_interfaces)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_local_api_access_token(app: AppHandle) -> Result<LocalApiAccessToken, String> {
    let token = {
        let path = config_path_for_app(&app)?;
        tauri::async_runtime::spawn_blocking(move || {
            local_api_token::read_token(&path)?.ok_or_else(|| {
                "Save or generate a local integration API access token before showing it"
                    .to_string()
            })
        })
        .await
        .map_err(|error| error.to_string())??
    };
    Ok(LocalApiAccessToken { token })
}

#[tauri::command]
pub async fn set_local_api_access_token(
    app: AppHandle,
    token: Option<String>,
) -> Result<LocalApiAccessToken, String> {
    let path = config_path_for_app(&app)?;
    let token = tauri::async_runtime::spawn_blocking(move || {
        let token = token.unwrap_or_else(local_api_token::generate_token);
        local_api_token::write_token(&path, &token)?;
        Ok::<String, String>(token)
    })
    .await
    .map_err(|error| error.to_string())??;
    reconfigure(&app);
    Ok(LocalApiAccessToken { token })
}

fn reconfigure_from_settings(
    app: AppHandle,
    config_path: std::path::PathBuf,
    settings: LocalApiSettings,
    log: LogSink,
) -> Result<(), String> {
    #[cfg(test)]
    let _ = &app;

    let mut manager = server_manager()
        .lock()
        .map_err(|_| "Local integration API state is unavailable".to_string())?;

    if !settings.enabled {
        stop_active_server(&mut manager);
        manager.last_error = None;
        let _ = log.write_unfiltered(
            LogLevel::Info,
            "local_api",
            "local integration API disabled",
        );
        return Ok(());
    }

    let network_token = if local_api_requires_access_token(&settings) {
        Some(local_api_token::read_token(&config_path)?.ok_or_else(|| {
            "Set and save a local integration API access token before enabling network listeners"
                .to_string()
        })?)
    } else {
        None
    };
    let listener_configs = listener_configs_for_settings(&settings, network_token.as_deref())?;
    let effective_config = EffectiveServerConfig {
        config_path: config_path.clone(),
        listeners: listener_configs.clone(),
    };

    if manager
        .active
        .as_ref()
        .is_some_and(|active| active.effective_config == effective_config)
    {
        manager.last_error = None;
        return Ok(());
    }

    stop_active_server(&mut manager);

    let servers = listener_configs
        .iter()
        .map(|listener| {
            Server::http(listener.address)
                .map(Arc::new)
                .map_err(|error| format!("Unable to listen on {}: {error}", listener.address))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refresh_in_flight = Arc::new(AtomicBool::new(false));
    let endpoints = listener_configs
        .iter()
        .map(|listener| endpoint_for_socket_address(listener.address))
        .collect::<Vec<_>>();
    let listeners = servers
        .into_iter()
        .zip(listener_configs)
        .map(|(server, listener_config)| {
            let shutdown = Arc::new(AtomicBool::new(false));
            let state = RequestState {
                config_path: config_path.clone(),
                #[cfg(not(test))]
                app: app.clone(),
                token: listener_config.token,
                refresh_in_flight: refresh_in_flight.clone(),
                log: log.clone(),
                #[cfg(test)]
                test_snapshot: None,
            };
            let worker = spawn_server_worker(server.clone(), shutdown.clone(), state);
            ActiveListener {
                server,
                shutdown,
                worker,
            }
        })
        .collect();

    let requires_auth = network_token.is_some();
    manager.active = Some(ActiveServer {
        effective_config,
        endpoints: endpoints.clone(),
        listeners,
    });
    manager.last_error = None;
    let _ = log.write_unfiltered(
        LogLevel::Info,
        "local_api",
        &format!(
            "local integration API listening endpoints={} authenticationRequired={requires_auth}",
            endpoints.join(",")
        ),
    );
    Ok(())
}

fn stop_active_server(manager: &mut LocalApiServerManager) {
    if let Some(active) = manager.active.take() {
        for listener in &active.listeners {
            listener.shutdown.store(true, Ordering::Release);
            listener.server.unblock();
        }
        for listener in active.listeners {
            let _ = listener.worker.join();
        }
    }
}

fn spawn_server_worker(
    server: Arc<Server>,
    shutdown: Arc<AtomicBool>,
    state: RequestState,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("quotabarwin-local-api".to_string())
        .spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                match server.recv_timeout(SERVER_RECV_TIMEOUT) {
                    Ok(Some(request)) => handle_request(request, &state),
                    Ok(None) => {}
                    Err(_) if shutdown.load(Ordering::Acquire) => break,
                    Err(error) => {
                        let _ = state.log.write(
                            LogLevel::Warn,
                            "local_api",
                            &format!("local integration API request receive failed: {error}"),
                        );
                    }
                }
            }
        })
        .expect("failed to start local integration API thread")
}

fn handle_request(request: Request, state: &RequestState) {
    if !is_authorized(&request, state.token.as_deref()) {
        let _ = request.respond(json_response(
            401,
            &ErrorResponse {
                error: ErrorBody {
                    code: "unauthorized",
                    message: "A valid Authorization bearer token is required.",
                },
            },
        ));
        return;
    }

    let path = request.url().split('?').next().unwrap_or(request.url());
    let response = match (request.method(), path) {
        (&Method::Get, "/v1/health") => health_response(state),
        (&Method::Get, "/v1/snapshot") => snapshot_response(state),
        (&Method::Post, "/v1/refresh") => refresh_response(state),
        _ => json_response(
            404,
            &ErrorResponse {
                error: ErrorBody {
                    code: "not_found",
                    message: "The requested local integration API endpoint does not exist.",
                },
            },
        ),
    };
    let _ = request.respond(response);
}

fn health_response(state: &RequestState) -> Response<std::io::Cursor<Vec<u8>>> {
    let snapshot_available = get_cached_snapshot_from_config_path(&state.config_path)
        .ok()
        .flatten()
        .is_some();
    json_response(
        200,
        &HealthResponse {
            api_version: API_VERSION,
            app_version: app_info::app_display_version(),
            snapshot_available,
        },
    )
}

fn snapshot_response(state: &RequestState) -> Response<std::io::Cursor<Vec<u8>>> {
    #[cfg(test)]
    if let Some(snapshot) = state.test_snapshot.clone() {
        return json_response(200, &public_snapshot(snapshot));
    }

    match get_cached_snapshot_from_config_path(&state.config_path) {
        Ok(Some(snapshot)) => json_response(200, &public_snapshot(snapshot)),
        Ok(None) => json_response(
            404,
            &ErrorResponse {
                error: ErrorBody {
                    code: "snapshot_unavailable",
                    message: "No quota snapshot is available yet.",
                },
            },
        ),
        Err(_) => json_response(
            503,
            &ErrorResponse {
                error: ErrorBody {
                    code: "snapshot_unavailable",
                    message: "The quota snapshot is temporarily unavailable.",
                },
            },
        ),
    }
}

#[cfg(test)]
fn refresh_response(state: &RequestState) -> Response<std::io::Cursor<Vec<u8>>> {
    if state
        .refresh_in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return json_response(
            202,
            &RefreshResponse {
                accepted: false,
                status: "refresh-in-progress",
            },
        );
    }
    state.refresh_in_flight.store(false, Ordering::Release);
    json_response(
        202,
        &RefreshResponse {
            accepted: true,
            status: "scheduled",
        },
    )
}

#[cfg(not(test))]
fn refresh_response(state: &RequestState) -> Response<std::io::Cursor<Vec<u8>>> {
    if state
        .refresh_in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return json_response(
            202,
            &RefreshResponse {
                accepted: false,
                status: "refresh-in-progress",
            },
        );
    }

    let state = state.clone();
    thread::Builder::new()
        .name("quotabarwin-local-api-refresh".to_string())
        .spawn(move || {
            let result = build_app_snapshot_from_config_path(&state.config_path);
            match result {
                Ok(snapshot) => {
                    let _ = state.app.emit(SNAPSHOT_REFRESHED_EVENT, snapshot);
                    let _ = state
                        .log
                        .write(LogLevel::Info, "local_api", "API refresh completed");
                }
                Err(error) => {
                    let _ = state.log.write(
                        LogLevel::Warn,
                        "local_api",
                        &format!("API refresh failed: {error}"),
                    );
                }
            }
            state.refresh_in_flight.store(false, Ordering::Release);
        })
        .expect("failed to start local integration API refresh");

    json_response(
        202,
        &RefreshResponse {
            accepted: true,
            status: "scheduled",
        },
    )
}

fn public_snapshot(mut snapshot: AppSnapshot) -> AppSnapshot {
    for provider in &mut snapshot.providers {
        provider.metadata = None;
        provider.diagnostics = None;
        provider.error = provider.error.take().map(|error| redact_sensitive(&error));
    }
    snapshot
}

fn json_response<T: Serialize>(status: u16, body: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(body).unwrap_or_else(|_| {
        br#"{"error":{"code":"serialization_failed","message":"Unable to serialize API response."}}"#
            .to_vec()
    });
    Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("valid content type header"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..])
                .expect("valid cache control header"),
        )
}

fn is_authorized(request: &Request, expected_token: Option<&str>) -> bool {
    let Some(expected_token) = expected_token else {
        return true;
    };
    let Some(header) = request.headers().iter().find(|header| {
        header
            .field
            .to_string()
            .eq_ignore_ascii_case("authorization")
    }) else {
        return false;
    };
    let Some(token) = header.value.as_str().strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_equals(token, expected_token)
}

fn constant_time_equals(left: &str, right: &str) -> bool {
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

fn listener_configs_for_settings(
    settings: &LocalApiSettings,
    network_token: Option<&str>,
) -> Result<Vec<EffectiveListenerConfig>, String> {
    let interfaces = if bind_target_needs_interface_enumeration(&settings.bind_target) {
        list_network_interfaces()?
    } else {
        Vec::new()
    };
    listener_configs_for_bind_target(
        &settings.bind_target,
        settings.port,
        &interfaces,
        network_token,
    )
}

fn bind_target_needs_interface_enumeration(bind_target: &LocalApiBindTarget) -> bool {
    match bind_target {
        LocalApiBindTarget::Loopback => false,
        LocalApiBindTarget::NetworkInterfaces { adapter_ids, .. } => !adapter_ids.is_empty(),
        LocalApiBindTarget::NetworkInterface { .. }
        | LocalApiBindTarget::AllNetworkInterfaces { .. } => true,
    }
}

fn listener_configs_for_bind_target(
    bind_target: &LocalApiBindTarget,
    port: u16,
    interfaces: &[LocalApiNetworkInterface],
    network_token: Option<&str>,
) -> Result<Vec<EffectiveListenerConfig>, String> {
    let mut listeners = match bind_target {
        LocalApiBindTarget::Loopback => vec![EffectiveListenerConfig {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            token: None,
        }],
        _ => {
            let addresses = addresses_for_bind_target(bind_target, interfaces)?;
            if addresses.is_empty() {
                Vec::new()
            } else {
                let token = network_token.ok_or_else(|| {
                    "Local integration API network token is unavailable".to_string()
                })?;
                addresses
                    .into_iter()
                    .map(|address| {
                        Ok(EffectiveListenerConfig {
                            address: socket_address_for_network_address(&address, port)?,
                            token: Some(token.to_string()),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?
            }
        }
    };

    if bind_target_includes_loopback(bind_target) {
        listeners.push(EffectiveListenerConfig {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            token: None,
        });
    }
    listeners.sort_by(|left, right| left.address.cmp(&right.address));
    listeners.dedup_by(|left, right| left.address == right.address);
    Ok(listeners)
}

fn bind_target_includes_loopback(bind_target: &LocalApiBindTarget) -> bool {
    match bind_target {
        LocalApiBindTarget::Loopback => false,
        LocalApiBindTarget::NetworkInterface {
            include_loopback, ..
        }
        | LocalApiBindTarget::NetworkInterfaces {
            include_loopback, ..
        }
        | LocalApiBindTarget::AllNetworkInterfaces { include_loopback } => *include_loopback,
    }
}

fn addresses_for_bind_target(
    bind_target: &LocalApiBindTarget,
    interfaces: &[LocalApiNetworkInterface],
) -> Result<Vec<LocalApiNetworkAddress>, String> {
    let selected_ids: Option<Vec<&str>> = match bind_target {
        LocalApiBindTarget::Loopback => {
            return Ok(vec![LocalApiNetworkAddress {
                address: Ipv4Addr::LOCALHOST.to_string(),
                scope_id: None,
            }]);
        }
        LocalApiBindTarget::NetworkInterface { adapter_id, .. } => Some(vec![adapter_id]),
        LocalApiBindTarget::NetworkInterfaces {
            adapter_ids,
            include_loopback,
        } => {
            if adapter_ids.is_empty() {
                if *include_loopback {
                    return Ok(Vec::new());
                }
                return Err(
                    "Select at least one local integration API network interface".to_string(),
                );
            }
            Some(adapter_ids.iter().map(String::as_str).collect())
        }
        LocalApiBindTarget::AllNetworkInterfaces { .. } => None,
    };
    let selected_interfaces = match selected_ids {
        Some(ids) => ids
            .into_iter()
            .map(|id| {
                interfaces
                    .iter()
                    .find(|interface| interface.id == id)
                    .ok_or_else(|| {
                        "The selected local integration API network interface is unavailable"
                            .to_string()
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => interfaces.iter().collect(),
    };

    let mut addresses = selected_interfaces
        .into_iter()
        .flat_map(|interface| interface.addresses.iter())
        .cloned()
        .collect::<Vec<_>>();
    addresses.sort_by(|left, right| {
        left.address
            .cmp(&right.address)
            .then_with(|| left.scope_id.cmp(&right.scope_id))
    });
    addresses.dedup();
    if addresses.is_empty() {
        return Err("No active local integration API network interfaces are available".to_string());
    }
    Ok(addresses)
}

fn socket_address_for_network_address(
    network_address: &LocalApiNetworkAddress,
    port: u16,
) -> Result<SocketAddr, String> {
    match network_address
        .address
        .parse::<IpAddr>()
        .map_err(|_| "The selected local integration API address is invalid".to_string())?
    {
        IpAddr::V4(ip) => Ok(SocketAddr::new(IpAddr::V4(ip), port)),
        IpAddr::V6(ip) => Ok(SocketAddr::V6(SocketAddrV6::new(
            ip,
            port,
            0,
            network_address.scope_id.unwrap_or(0),
        ))),
    }
}

fn endpoint_for_socket_address(address: SocketAddr) -> String {
    match address {
        SocketAddr::V4(address) => format!("http://{address}"),
        SocketAddr::V6(address) if address.scope_id() == 0 => {
            format!("http://[{}]:{}", address.ip(), address.port())
        }
        SocketAddr::V6(address) => {
            format!(
                "http://[{}%25{}]:{}",
                address.ip(),
                address.scope_id(),
                address.port()
            )
        }
    }
}

fn list_network_interfaces() -> Result<Vec<LocalApiNetworkInterface>, String> {
    let mut buffer_length = 15_000_u32;
    let mut buffer = Vec::<u8>::new();
    loop {
        buffer.resize(buffer_length as usize, 0);
        let result = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC as u32,
                0,
                std::ptr::null(),
                buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(),
                &mut buffer_length,
            )
        };
        if result == ERROR_BUFFER_OVERFLOW {
            buffer.clear();
            continue;
        }
        if result != 0 {
            return Err(format!(
                "Unable to enumerate local network interfaces (Windows error {result})"
            ));
        }
        break;
    }

    let mut interfaces: Vec<LocalApiNetworkInterface> = Vec::new();
    let mut adapter = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    while !adapter.is_null() {
        let current = unsafe { &*adapter };
        if current.OperStatus == NET_IF_OPER_STATUS_UP {
            let id = unsafe { CStr::from_ptr(current.AdapterName.cast()) }
                .to_string_lossy()
                .into_owned();
            let name = unsafe { wide_string(current.FriendlyName) };
            let name = if name.is_empty() { id.clone() } else { name };
            let mut unicast = current.FirstUnicastAddress;
            while !unicast.is_null() {
                let current_unicast = unsafe { &*unicast };
                if let Some((address, is_private)) = unsafe {
                    usable_network_address_with_interface_index(
                        current_unicast.Address.lpSockaddr,
                        current.Ipv6IfIndex,
                    )
                } {
                    if let Some(interface) =
                        interfaces.iter_mut().find(|interface| interface.id == id)
                    {
                        interface.addresses.push(address);
                        interface.is_private |= is_private;
                    } else {
                        interfaces.push(LocalApiNetworkInterface {
                            id: id.clone(),
                            name: name.clone(),
                            addresses: vec![address],
                            is_private,
                        });
                    }
                }
                unicast = current_unicast.Next;
            }
        }
        adapter = current.Next;
    }

    for interface in &mut interfaces {
        interface.addresses.sort_by(|left, right| {
            left.address
                .cmp(&right.address)
                .then_with(|| left.scope_id.cmp(&right.scope_id))
        });
        interface.addresses.dedup();
    }
    interfaces.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(interfaces)
}

unsafe fn wide_string(pointer: *mut u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0;
    while unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(pointer, length) })
}

unsafe fn usable_network_address_with_interface_index(
    pointer: *mut windows_sys::Win32::Networking::WinSock::SOCKADDR,
    ipv6_interface_index: u32,
) -> Option<(LocalApiNetworkAddress, bool)> {
    if pointer.is_null() {
        return None;
    }
    match unsafe { (*pointer).sa_family } {
        AF_INET => {
            let address = unsafe { &*(pointer.cast::<SOCKADDR_IN>()) };
            let bytes = unsafe { address.sin_addr.S_un.S_un_b };
            let ip = Ipv4Addr::new(bytes.s_b1, bytes.s_b2, bytes.s_b3, bytes.s_b4);
            if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
                return None;
            }
            Some((
                LocalApiNetworkAddress {
                    address: ip.to_string(),
                    scope_id: None,
                },
                ip.is_private() || ip.is_link_local(),
            ))
        }
        AF_INET6 => {
            let address = unsafe { &*(pointer.cast::<SOCKADDR_IN6>()) };
            let bytes = unsafe { address.sin6_addr.u.Byte };
            let ip = Ipv6Addr::from(bytes);
            if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
                return None;
            }
            let scope_id = if ip.is_unicast_link_local() {
                let scope_id = unsafe { address.Anonymous.sin6_scope_id };
                let scope_id = if scope_id == 0 {
                    ipv6_interface_index
                } else {
                    scope_id
                };
                (scope_id != 0).then_some(scope_id)
            } else {
                None
            };
            if ip.is_unicast_link_local() && scope_id.is_none() {
                return None;
            }
            Some((
                LocalApiNetworkAddress {
                    address: ip.to_string(),
                    scope_id,
                },
                ip.is_unique_local() || ip.is_unicast_link_local(),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };

    fn start_test_server(
        snapshot: AppSnapshot,
        token: Option<String>,
    ) -> (
        tempfile::TempDir,
        SocketAddr,
        Arc<Server>,
        Arc<AtomicBool>,
        JoinHandle<()>,
    ) {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("config.quotaBarWin.json");
        let config = crate::config::default_config();
        crate::config::save_config_to_path(&config_path, &config).expect("save config");
        let log = LogSink::from_config_path(&config_path, &config);
        let server = Arc::new(
            Server::http(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
                .expect("start test server"),
        );
        let address = server.server_addr().to_ip().expect("IP listener");
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = spawn_server_worker(
            server.clone(),
            shutdown.clone(),
            RequestState {
                config_path,
                token,
                refresh_in_flight: Arc::new(AtomicBool::new(false)),
                log,
                test_snapshot: Some(snapshot),
            },
        );

        (temp, address, server, shutdown, worker)
    }

    fn request(address: SocketAddr, request: &str) -> String {
        let mut stream = TcpStream::connect(address).expect("connect to local API");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        stream.write_all(request.as_bytes()).expect("write request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        response
    }

    fn stop_test_server(server: Arc<Server>, shutdown: Arc<AtomicBool>, worker: JoinHandle<()>) {
        shutdown.store(true, Ordering::Release);
        server.unblock();
        worker.join().expect("join server worker");
    }

    fn successful_snapshot() -> AppSnapshot {
        AppSnapshot {
            schema_version: 1,
            refreshed_at: "2026-08-19T00:00:00Z".to_string(),
            providers: vec![crate::quota::ProviderSnapshot {
                id: "fixture-provider".to_string(),
                name: "Fixture Provider".to_string(),
                status: "ok".to_string(),
                source: "remote".to_string(),
                updated_at: None,
                windows: Vec::new(),
                error: None,
                diagnostics: Some(crate::quota::ProviderDiagnostics {
                    checked_at: "2026-08-19T00:00:00Z".to_string(),
                    messages: vec!["internal".to_string()],
                    command_path: None,
                    exit_code: None,
                    duration_ms: None,
                    timed_out: None,
                    stderr: None,
                }),
                metadata: Some(serde_json::json!({ "private": true })),
            }],
        }
    }

    #[test]
    fn http_api_serves_health_snapshot_and_refresh_over_loopback() {
        let (_temp, address, server, shutdown, worker) =
            start_test_server(successful_snapshot(), None);

        let health = request(
            address,
            "GET /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(health.starts_with("HTTP/1.1 200"), "{health}");
        assert!(health.contains("\"apiVersion\":1"), "{health}");

        let snapshot = request(
            address,
            "GET /v1/snapshot HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(snapshot.starts_with("HTTP/1.1 200"), "{snapshot}");
        assert!(snapshot.contains("fixture-provider"), "{snapshot}");
        assert!(snapshot.contains("\"metadata\":null"), "{snapshot}");
        assert!(snapshot.contains("\"diagnostics\":null"), "{snapshot}");
        assert!(!snapshot.contains("\"private\":true"), "{snapshot}");

        let refresh = request(
            address,
            "POST /v1/refresh HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        assert!(refresh.starts_with("HTTP/1.1 202"), "{refresh}");
        assert!(refresh.contains("\"accepted\":true"), "{refresh}");

        stop_test_server(server, shutdown, worker);
    }

    #[test]
    fn http_api_rejects_missing_network_access_token() {
        let token = "x".repeat(32);
        let (_temp, address, server, shutdown, worker) =
            start_test_server(successful_snapshot(), Some(token.clone()));

        let missing_token = request(
            address,
            "GET /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(missing_token.starts_with("HTTP/1.1 401"), "{missing_token}");

        let authorized = request(
            address,
            &format!(
                "GET /v1/health HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(authorized.starts_with("HTTP/1.1 200"), "{authorized}");

        stop_test_server(server, shutdown, worker);
    }

    #[test]
    fn token_comparison_requires_exact_matching_values() {
        assert!(constant_time_equals("abcd", "abcd"));
        assert!(!constant_time_equals("abcd", "abce"));
        assert!(!constant_time_equals("abcd", "abc"));
    }

    #[test]
    fn public_snapshot_excludes_private_provider_details() {
        let token = "x".repeat(40);
        let snapshot = AppSnapshot {
            schema_version: 1,
            refreshed_at: "2026-08-19T00:00:00Z".to_string(),
            providers: vec![crate::quota::ProviderSnapshot {
                id: "provider".to_string(),
                name: "Provider".to_string(),
                status: "error".to_string(),
                source: "remote".to_string(),
                updated_at: None,
                windows: Vec::new(),
                error: Some(format!("Authorization: Bearer {token}")),
                diagnostics: Some(crate::quota::ProviderDiagnostics {
                    checked_at: "2026-08-19T00:00:00Z".to_string(),
                    messages: vec!["internal".to_string()],
                    command_path: Some("C:\\secret.exe".to_string()),
                    exit_code: Some(1),
                    duration_ms: Some(1),
                    timed_out: Some(false),
                    stderr: Some("internal".to_string()),
                }),
                metadata: Some(serde_json::json!({ "private": true })),
            }],
        };

        let sanitized = public_snapshot(snapshot);
        let provider = &sanitized.providers[0];
        assert!(provider.metadata.is_none());
        assert!(provider.diagnostics.is_none());
        assert!(!provider
            .error
            .as_deref()
            .unwrap_or_default()
            .contains(&token));
    }

    fn enabled_settings(bind_target: LocalApiBindTarget) -> LocalApiSettings {
        LocalApiSettings {
            enabled: true,
            bind_target,
            port: 41833,
        }
    }

    #[test]
    fn loopback_does_not_require_authentication() {
        assert!(!local_api_requires_access_token(&enabled_settings(
            LocalApiBindTarget::Loopback
        )));
        assert!(local_api_requires_access_token(&enabled_settings(
            LocalApiBindTarget::NetworkInterface {
                adapter_id: "adapter".to_string(),
                include_loopback: true,
            }
        )));
        assert!(local_api_requires_access_token(&enabled_settings(
            LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: vec!["adapter".to_string()],
                include_loopback: true,
            }
        )));
        assert!(local_api_requires_access_token(&enabled_settings(
            LocalApiBindTarget::AllNetworkInterfaces {
                include_loopback: true,
            }
        )));
        assert!(!local_api_requires_access_token(&enabled_settings(
            LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: Vec::new(),
                include_loopback: true,
            }
        )));
    }

    #[test]
    fn selected_or_all_network_interfaces_resolve_to_every_active_address() {
        let interfaces = vec![
            LocalApiNetworkInterface {
                id: "ethernet".to_string(),
                name: "Ethernet".to_string(),
                addresses: vec![
                    LocalApiNetworkAddress {
                        address: "192.168.1.10".to_string(),
                        scope_id: None,
                    },
                    LocalApiNetworkAddress {
                        address: "10.0.0.10".to_string(),
                        scope_id: None,
                    },
                ],
                is_private: true,
            },
            LocalApiNetworkInterface {
                id: "wifi".to_string(),
                name: "Wi-Fi".to_string(),
                addresses: vec![LocalApiNetworkAddress {
                    address: "192.168.50.10".to_string(),
                    scope_id: None,
                }],
                is_private: true,
            },
        ];

        let selected = addresses_for_bind_target(
            &LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: vec!["wifi".to_string(), "ethernet".to_string()],
                include_loopback: true,
            },
            &interfaces,
        )
        .expect("selected interfaces resolve");
        assert_eq!(
            selected,
            vec![
                LocalApiNetworkAddress {
                    address: "10.0.0.10".to_string(),
                    scope_id: None,
                },
                LocalApiNetworkAddress {
                    address: "192.168.1.10".to_string(),
                    scope_id: None,
                },
                LocalApiNetworkAddress {
                    address: "192.168.50.10".to_string(),
                    scope_id: None,
                },
            ]
        );

        let all = addresses_for_bind_target(
            &LocalApiBindTarget::AllNetworkInterfaces {
                include_loopback: true,
            },
            &interfaces,
        )
        .expect("all interfaces resolve");
        assert_eq!(all, selected);
    }

    #[test]
    fn loopback_and_network_listeners_use_independent_authentication() {
        let interfaces = vec![LocalApiNetworkInterface {
            id: "ethernet".to_string(),
            name: "Ethernet".to_string(),
            addresses: vec![LocalApiNetworkAddress {
                address: "192.168.1.10".to_string(),
                scope_id: None,
            }],
            is_private: true,
        }];
        let listeners = listener_configs_for_bind_target(
            &LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: vec!["ethernet".to_string()],
                include_loopback: true,
            },
            41833,
            &interfaces,
            Some("x"),
        )
        .expect("listeners resolve");

        assert_eq!(listeners.len(), 2);
        assert!(listeners.iter().any(|listener| {
            listener.address == "127.0.0.1:41833".parse().unwrap() && listener.token.is_none()
        }));
        assert!(listeners.iter().any(|listener| {
            listener.address == "192.168.1.10:41833".parse().unwrap()
                && listener.token.as_deref() == Some("x")
        }));
    }

    #[test]
    fn an_empty_network_selection_can_listen_on_loopback_only() {
        assert!(!bind_target_needs_interface_enumeration(
            &LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: Vec::new(),
                include_loopback: true,
            }
        ));
        let listeners = listener_configs_for_bind_target(
            &LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: Vec::new(),
                include_loopback: true,
            },
            41833,
            &[],
            None,
        )
        .expect("loopback-only selection resolves");

        assert_eq!(listeners.len(), 1);
        assert_eq!(listeners[0].address, "127.0.0.1:41833".parse().unwrap());
        assert!(listeners[0].token.is_none());
    }

    #[test]
    fn ipv6_network_listener_preserves_scope_and_uses_a_valid_url() {
        let interfaces = vec![LocalApiNetworkInterface {
            id: "ethernet".to_string(),
            name: "Ethernet".to_string(),
            addresses: vec![LocalApiNetworkAddress {
                address: "fe80::1234".to_string(),
                scope_id: Some(12),
            }],
            is_private: true,
        }];
        let listeners = listener_configs_for_bind_target(
            &LocalApiBindTarget::NetworkInterfaces {
                adapter_ids: vec!["ethernet".to_string()],
                include_loopback: false,
            },
            41833,
            &interfaces,
            Some("x"),
        )
        .expect("IPv6 listener resolves");

        assert_eq!(listeners.len(), 1);
        assert_eq!(
            endpoint_for_socket_address(listeners[0].address),
            "http://[fe80::1234%2512]:41833"
        );
        assert_eq!(listeners[0].token.as_deref(), Some("x"));
    }
}
