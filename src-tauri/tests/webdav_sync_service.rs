use std::{
    collections::{HashMap, HashSet},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use cc_switch_lib::{
    set_webdav_sync_settings, WebDavSyncService, WebDavSyncSettings, WebDavSyncStatus,
};
use tokio::sync::oneshot;

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, lock_test_mutex, reset_test_fs};

const DAV_ROOT: &str = "/dav";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeReadback {
    Stored,
    Missing,
    Mismatch,
    Oversized,
    OversizedStreaming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeleteBehavior {
    Success,
    NotFound,
    ServerError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ServerConfig {
    probe_readback: ProbeReadback,
    manifest_readback: ProbeReadback,
    manifest_head_behavior: ManifestHeadBehavior,
    reject_dotfile_puts: bool,
    delete_behavior: DeleteBehavior,
}

impl ServerConfig {
    fn for_readback(readback: ProbeReadback) -> Self {
        Self {
            probe_readback: readback,
            manifest_readback: ProbeReadback::Stored,
            manifest_head_behavior: ManifestHeadBehavior::Present,
            reject_dotfile_puts: false,
            delete_behavior: DeleteBehavior::Success,
        }
    }

    fn for_manifest_readback(
        manifest_readback: ProbeReadback,
        manifest_head_behavior: ManifestHeadBehavior,
    ) -> Self {
        Self {
            probe_readback: ProbeReadback::Stored,
            manifest_readback,
            manifest_head_behavior,
            reject_dotfile_puts: false,
            delete_behavior: DeleteBehavior::Success,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManifestHeadBehavior {
    Present,
    Missing,
    ServerError,
}

#[derive(Debug, Default)]
struct ServerState {
    directories: HashSet<String>,
    files: HashMap<String, Vec<u8>>,
    put_paths: Vec<String>,
    get_paths: Vec<String>,
    head_paths: Vec<String>,
    delete_paths: Vec<String>,
    streamed_chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerSnapshot {
    put_paths: Vec<String>,
    get_paths: Vec<String>,
    head_paths: Vec<String>,
    delete_paths: Vec<String>,
    streamed_chunk_count: usize,
}

#[derive(Clone)]
struct AppState {
    config: ServerConfig,
    inner: Arc<Mutex<ServerState>>,
}

struct TestWebDavServer {
    base_url: String,
    state: Arc<Mutex<ServerState>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl TestWebDavServer {
    fn start(readback: ProbeReadback) -> Self {
        Self::start_with_config(ServerConfig::for_readback(readback))
    }

    fn start_with_config(config: ServerConfig) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test WebDAV listener");
        let port = listener
            .local_addr()
            .expect("read test WebDAV listener address")
            .port();
        listener
            .set_nonblocking(true)
            .expect("set test WebDAV listener nonblocking");

        let state = Arc::new(Mutex::new(ServerState {
            directories: HashSet::from([DAV_ROOT.to_string()]),
            ..ServerState::default()
        }));
        let app_state = AppState {
            config,
            inner: Arc::clone(&state),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let join_handle = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test WebDAV runtime");

            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener)
                    .expect("convert test WebDAV listener");
                let app = Router::new()
                    .route(DAV_ROOT, any(handle_webdav_request))
                    .route("/dav/*path", any(handle_webdav_request))
                    .with_state(app_state);

                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.await;
                    })
                    .await
                    .expect("run test WebDAV server");
            });
        });

        Self {
            base_url: format!("http://127.0.0.1:{port}{DAV_ROOT}"),
            state,
            shutdown_tx: Some(shutdown_tx),
            join_handle: Some(join_handle),
        }
    }

    fn snapshot(&self) -> ServerSnapshot {
        let state = self.state.lock().expect("lock test WebDAV state");
        ServerSnapshot {
            put_paths: state.put_paths.clone(),
            get_paths: state.get_paths.clone(),
            head_paths: state.head_paths.clone(),
            delete_paths: state.delete_paths.clone(),
            streamed_chunk_count: state.streamed_chunk_count,
        }
    }
}

impl Drop for TestWebDavServer {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().expect("join test WebDAV server thread");
        }
    }
}

async fn handle_webdav_request(State(state): State<AppState>, request: Request<Body>) -> Response {
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();

    match method.as_str() {
        "PROPFIND" => {
            let exists = state
                .inner
                .lock()
                .expect("lock PROPFIND state")
                .directories
                .contains(&path);
            if exists {
                multi_status_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        "MKCOL" => {
            state
                .inner
                .lock()
                .expect("lock MKCOL state")
                .directories
                .insert(path);
            StatusCode::CREATED.into_response()
        }
        "PUT" => {
            if state.config.reject_dotfile_puts
                && path
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name.starts_with('.'))
            {
                return StatusCode::FORBIDDEN.into_response();
            }
            let parent_exists = path.rsplit_once('/').is_some_and(|(parent, _)| {
                state
                    .inner
                    .lock()
                    .expect("lock PUT parent state")
                    .directories
                    .contains(parent)
            });
            if !parent_exists {
                return StatusCode::CONFLICT.into_response();
            }
            let bytes = to_bytes(request.into_body(), usize::MAX)
                .await
                .expect("read PUT body")
                .to_vec();
            let mut inner = state.inner.lock().expect("lock PUT state");
            inner.put_paths.push(path.clone());
            inner.files.insert(path, bytes);
            StatusCode::CREATED.into_response()
        }
        "GET" => {
            let mut inner = state.inner.lock().expect("lock GET state");
            inner.get_paths.push(path.clone());
            match readback_for_path(&state.config, &path) {
                ProbeReadback::Missing => StatusCode::NOT_FOUND.into_response(),
                ProbeReadback::Mismatch => {
                    (StatusCode::OK, b"mismatched-probe".to_vec()).into_response()
                }
                ProbeReadback::Oversized => (StatusCode::OK, vec![b'x'; 8192]).into_response(),
                ProbeReadback::OversizedStreaming => {
                    let inner = Arc::clone(&state.inner);
                    let stream = async_stream::stream! {
                        for _ in 0..8 {
                            inner
                                .lock()
                                .expect("lock streamed GET state")
                                .streamed_chunk_count += 1;
                            yield Ok::<_, std::io::Error>(bytes::Bytes::from(vec![b'y'; 1024]));
                            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                        }
                    };
                    (
                        [("content-type", "application/octet-stream")],
                        Body::from_stream(stream),
                    )
                        .into_response()
                }
                ProbeReadback::Stored => match inner.files.get(&path).cloned() {
                    Some(bytes) => (StatusCode::OK, bytes).into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                },
            }
        }
        "HEAD" => {
            let mut inner = state.inner.lock().expect("lock HEAD state");
            inner.head_paths.push(path.clone());
            if is_manifest_path(&path) {
                match state.config.manifest_head_behavior {
                    ManifestHeadBehavior::Present => StatusCode::OK.into_response(),
                    ManifestHeadBehavior::Missing => StatusCode::NOT_FOUND.into_response(),
                    ManifestHeadBehavior::ServerError => {
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            } else if inner.files.contains_key(&path) {
                StatusCode::OK.into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        "DELETE" => {
            let mut inner = state.inner.lock().expect("lock DELETE state");
            inner.delete_paths.push(path.clone());
            inner.files.remove(&path);
            match state.config.delete_behavior {
                DeleteBehavior::Success => StatusCode::NO_CONTENT.into_response(),
                DeleteBehavior::NotFound => StatusCode::NOT_FOUND.into_response(),
                DeleteBehavior::ServerError => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

fn multi_status_response() -> Response {
    StatusCode::from_u16(207)
        .expect("build 207 Multi-Status")
        .into_response()
}

fn readback_for_path(config: &ServerConfig, path: &str) -> ProbeReadback {
    if is_probe_path(path) {
        config.probe_readback
    } else if is_manifest_path(path) {
        config.manifest_readback
    } else {
        ProbeReadback::Stored
    }
}

fn is_probe_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with("cc-switch-probe-"))
}

fn is_manifest_path(path: &str) -> bool {
    path.ends_with("/manifest.json")
}

fn sample_settings(base_url: &str) -> WebDavSyncSettings {
    WebDavSyncSettings {
        enabled: true,
        base_url: base_url.to_string(),
        remote_root: "sync-root".to_string(),
        profile: "default-profile".to_string(),
        username: "demo".to_string(),
        password: "secret".to_string(),
        auto_sync: false,
        status: WebDavSyncStatus::default(),
    }
}

fn assert_probe_round_trip(snapshot: &ServerSnapshot) {
    assert_eq!(
        snapshot.put_paths.len(),
        1,
        "expected exactly one probe PUT: {snapshot:?}"
    );
    assert_eq!(
        snapshot.get_paths.len(),
        1,
        "expected exactly one probe GET: {snapshot:?}"
    );
    assert_eq!(
        snapshot.delete_paths.len(),
        1,
        "expected exactly one best-effort probe DELETE: {snapshot:?}"
    );

    let probe_path = &snapshot.put_paths[0];
    assert!(
        probe_path.starts_with("/dav/sync-root/v2/db-v6/default-profile/"),
        "unexpected probe path: {probe_path}"
    );
    assert!(
        !probe_path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with('.')),
        "probe file should not be hidden: {probe_path}"
    );
    assert_eq!(
        &snapshot.get_paths[0], probe_path,
        "GET should read back the probe file"
    );
    assert_eq!(
        &snapshot.delete_paths[0], probe_path,
        "DELETE should clean up the probe file"
    );
}

fn assert_upload_artifact_puts(snapshot: &ServerSnapshot) {
    assert_eq!(
        snapshot.put_paths,
        vec![
            "/dav/sync-root/v2/db-v6/default-profile/db.sql".to_string(),
            "/dav/sync-root/v2/db-v6/default-profile/skills.zip".to_string(),
            "/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()
        ],
        "unexpected upload PUT sequence: {snapshot:?}"
    );
}

#[test]
fn check_connection_succeeds_after_round_trip_probe() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start(ProbeReadback::Stored);
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    WebDavSyncService::check_connection().expect("round-trip probe should succeed");

    let snapshot = server.snapshot();
    assert_probe_round_trip(&snapshot);
}

#[test]
fn check_connection_fails_when_probe_readback_is_missing() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start(ProbeReadback::Missing);
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err = WebDavSyncService::check_connection()
        .expect_err("missing probe readback should fail connection check");

    let snapshot = server.snapshot();
    assert_eq!(
        snapshot.put_paths.len(),
        1,
        "probe write should happen before failure"
    );
    assert_eq!(
        snapshot.get_paths.len(),
        1,
        "probe readback should be attempted"
    );
    assert_eq!(
        snapshot.delete_paths.len(),
        1,
        "probe cleanup should be attempted"
    );
    assert!(
        err.to_string().contains("probe") || err.to_string().contains("GET"),
        "unexpected error: {err}"
    );
}

#[test]
fn check_connection_fails_when_probe_readback_mismatches() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start(ProbeReadback::Mismatch);
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err = WebDavSyncService::check_connection()
        .expect_err("mismatched probe readback should fail connection check");

    let snapshot = server.snapshot();
    assert_eq!(
        snapshot.put_paths.len(),
        1,
        "probe write should happen before failure"
    );
    assert_eq!(
        snapshot.get_paths.len(),
        1,
        "probe readback should be attempted"
    );
    assert_eq!(
        snapshot.delete_paths.len(),
        1,
        "probe cleanup should be attempted"
    );
    assert!(
        err.to_string().contains("probe") || err.to_string().contains("mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn check_connection_succeeds_when_server_rejects_hidden_probe_files() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig {
        probe_readback: ProbeReadback::Stored,
        manifest_readback: ProbeReadback::Stored,
        manifest_head_behavior: ManifestHeadBehavior::Present,
        reject_dotfile_puts: true,
        delete_behavior: DeleteBehavior::Success,
    });
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    WebDavSyncService::check_connection()
        .expect("non-hidden probe should succeed even when dotfiles are blocked");

    let snapshot = server.snapshot();
    assert_probe_round_trip(&snapshot);
}

#[test]
fn check_connection_succeeds_when_probe_cleanup_delete_fails_after_successful_round_trip() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig {
        probe_readback: ProbeReadback::Stored,
        manifest_readback: ProbeReadback::Stored,
        manifest_head_behavior: ManifestHeadBehavior::Present,
        reject_dotfile_puts: false,
        delete_behavior: DeleteBehavior::ServerError,
    });
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    WebDavSyncService::check_connection()
        .expect("probe cleanup delete failure should stay best-effort");

    let snapshot = server.snapshot();
    assert_probe_round_trip(&snapshot);
    assert_eq!(
        snapshot.delete_paths.len(),
        1,
        "cleanup should still be attempted"
    );
}

#[test]
fn check_connection_succeeds_when_probe_cleanup_delete_reports_missing_after_successful_round_trip()
{
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig {
        probe_readback: ProbeReadback::Stored,
        manifest_readback: ProbeReadback::Stored,
        manifest_head_behavior: ManifestHeadBehavior::Present,
        reject_dotfile_puts: false,
        delete_behavior: DeleteBehavior::NotFound,
    });
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    WebDavSyncService::check_connection()
        .expect("probe cleanup delete 404 should stay best-effort");

    let snapshot = server.snapshot();
    assert_probe_round_trip(&snapshot);
    assert_eq!(
        snapshot.delete_paths.len(),
        1,
        "cleanup should still be attempted"
    );
}

#[test]
fn check_connection_reports_probe_failure_even_when_cleanup_delete_fails() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig {
        probe_readback: ProbeReadback::Mismatch,
        manifest_readback: ProbeReadback::Stored,
        manifest_head_behavior: ManifestHeadBehavior::Present,
        reject_dotfile_puts: false,
        delete_behavior: DeleteBehavior::ServerError,
    });
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err = WebDavSyncService::check_connection()
        .expect_err("probe mismatch should remain the main error");

    let snapshot = server.snapshot();
    assert_eq!(
        snapshot.delete_paths.len(),
        1,
        "cleanup should still be attempted"
    );
    assert!(
        err.to_string().contains("probe") || err.to_string().contains("mismatch"),
        "unexpected error: {err}"
    );
    assert!(
        !err.to_string().contains("DELETE"),
        "cleanup failure should not mask the probe error: {err}"
    );
}

#[test]
fn upload_succeeds_when_manifest_readback_matches() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig::for_manifest_readback(
        ProbeReadback::Stored,
        ManifestHeadBehavior::Missing,
    ));
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let summary = WebDavSyncService::upload().expect("matching manifest readback should succeed");

    assert_eq!(summary.decision, cc_switch_lib::SyncDecision::Upload);
    let snapshot = server.snapshot();
    assert_upload_artifact_puts(&snapshot);
    assert_eq!(
        snapshot.get_paths,
        vec!["/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()],
        "upload should verify manifest bytes via GET"
    );
    assert_eq!(
        snapshot.head_paths,
        vec!["/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()],
        "HEAD should remain best-effort metadata only"
    );
}

#[test]
fn upload_succeeds_when_manifest_head_returns_server_error() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig::for_manifest_readback(
        ProbeReadback::Stored,
        ManifestHeadBehavior::ServerError,
    ));
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let summary = WebDavSyncService::upload()
        .expect("manifest HEAD errors should stay best-effort after matching GET readback");

    assert_eq!(summary.decision, cc_switch_lib::SyncDecision::Upload);
    let snapshot = server.snapshot();
    assert_upload_artifact_puts(&snapshot);
    assert_eq!(
        snapshot.get_paths,
        vec!["/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()],
        "upload success should remain gated by manifest GET readback"
    );
    assert_eq!(
        snapshot.head_paths,
        vec!["/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()],
        "HEAD should still be attempted as best-effort metadata"
    );
}

#[test]
fn upload_fails_when_manifest_readback_is_missing() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig::for_manifest_readback(
        ProbeReadback::Missing,
        ManifestHeadBehavior::Present,
    ));
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err =
        WebDavSyncService::upload().expect_err("missing manifest readback should fail upload");

    let snapshot = server.snapshot();
    assert_upload_artifact_puts(&snapshot);
    assert_eq!(
        snapshot.get_paths,
        vec!["/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()],
        "upload should attempt manifest readback before failing"
    );
    assert!(
        snapshot.head_paths.is_empty(),
        "HEAD should not decide success"
    );
    assert!(
        err.to_string().contains("manifest") || err.to_string().contains("readback"),
        "unexpected error: {err}"
    );
}

#[test]
fn upload_fails_when_manifest_readback_mismatches() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig::for_manifest_readback(
        ProbeReadback::Mismatch,
        ManifestHeadBehavior::Present,
    ));
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err =
        WebDavSyncService::upload().expect_err("mismatched manifest readback should fail upload");

    let snapshot = server.snapshot();
    assert_upload_artifact_puts(&snapshot);
    assert_eq!(
        snapshot.get_paths,
        vec!["/dav/sync-root/v2/db-v6/default-profile/manifest.json".to_string()],
        "upload should attempt manifest readback before failing"
    );
    assert!(
        snapshot.head_paths.is_empty(),
        "HEAD should not decide success"
    );
    assert!(
        err.to_string().contains("manifest") || err.to_string().contains("mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn upload_fails_when_manifest_readback_exceeds_expected_size() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig::for_manifest_readback(
        ProbeReadback::Oversized,
        ManifestHeadBehavior::Present,
    ));
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err =
        WebDavSyncService::upload().expect_err("oversized manifest readback should fail upload");

    assert!(
        err.to_string().contains("大小限制") || err.to_string().contains("size limit"),
        "unexpected error: {err}"
    );
}

#[test]
fn upload_fails_when_manifest_stream_readback_exceeds_expected_size() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let server = TestWebDavServer::start_with_config(ServerConfig::for_manifest_readback(
        ProbeReadback::OversizedStreaming,
        ManifestHeadBehavior::Present,
    ));
    set_webdav_sync_settings(Some(sample_settings(&server.base_url)))
        .expect("save test WebDAV settings");

    let err = WebDavSyncService::upload()
        .expect_err("oversized streamed manifest readback should fail upload");

    let snapshot = server.snapshot();
    assert!(
        snapshot.streamed_chunk_count < 8,
        "bounded streaming readback should stop early: {snapshot:?}"
    );
    assert!(
        err.to_string().contains("大小限制") || err.to_string().contains("size limit"),
        "unexpected error: {err}"
    );
}

#[test]
fn server_rejects_put_when_parent_directory_is_missing() {
    let server = TestWebDavServer::start(ProbeReadback::Stored);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build reqwest runtime");

    let status = runtime.block_on(async {
        reqwest::Client::new()
            .put(format!(
                "{}/sync-root/v2/db-v6/default-profile/db.sql",
                server.base_url
            ))
            .body("db")
            .send()
            .await
            .expect("PUT missing parent")
            .status()
    });

    assert_eq!(status, StatusCode::CONFLICT);
}
