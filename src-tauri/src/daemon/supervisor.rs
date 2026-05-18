//! The supervisor: spawns and watches the proxy worker, owns the daemon's
//! shared `ProxyService`, and translates IPC requests into actions.
//!
//! Most of the heavy lifting (config rewrites, restoration, common-config
//! preservation) lives in `ProxyService`. The supervisor's job is to keep one
//! worker per active app route, keep the `proxy_runtime_session` row aligned
//! with the actual workers, and survive worker crashes via the restart policy.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex, Notify};

use crate::app_config::AppType;
use crate::database::Database;
use crate::services::ProxyService;

use super::ipc::protocol::{Request, Response, TakeoverFlags, WorkerState};
use super::ipc::server::Handler;
use super::restart::{Decision, RestartPolicy};

const PROXY_RUNTIME_SESSION_KEY: &str = "proxy_runtime_session";
pub const DAEMON_SOCKET_ENV: &str = "CC_SWITCH_DAEMON_SOCKET";
pub const SESSION_TOKEN_ENV: &str = "CC_SWITCH_PROXY_SESSION_TOKEN";
pub const RESTORE_GUARD_BYPASS_ENV: &str = "CC_SWITCH_RESTORE_GUARD_BYPASS";
/// Mirrors `services::proxy::PROXY_RUNTIME_KIND_ENV_KEY`. Setting this to
/// `managed_external` makes the worker skip self-publishing the runtime
/// session row — the daemon writes it after WorkerHello.
pub const RUNTIME_KIND_ENV: &str = "CC_SWITCH_PROXY_RUNTIME_KIND";
pub const RUNTIME_KIND_MANAGED_EXTERNAL: &str = "managed_external";

const WORKER_HELLO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
struct WorkerInfo {
    app_type: AppType,
    pid: u32,
    address: String,
    port: u16,
    session_token: String,
}

#[derive(Default)]
struct SupervisorInner {
    workers: HashMap<AppType, WorkerInfo>,
    pending_hellos: HashMap<String, oneshot::Sender<WorkerInfo>>,
    pending_tokens: HashMap<String, String>,
    stopping_workers: HashSet<AppType>,
    restart: RestartPolicy,
    last_restart_at: Option<chrono::DateTime<chrono::Utc>>,
    restart_count: u32,
    shutdown_requested: bool,
}

#[derive(Clone)]
pub struct Supervisor {
    db: Arc<Database>,
    proxy: ProxyService,
    inner: Arc<Mutex<SupervisorInner>>,
    /// Serializes worker spawn so concurrent EnsureWorker IPCs share the same
    /// pending hello rather than racing — a second caller used to overwrite
    /// `pending_hello` and `pending_token`, leaving the first caller waiting
    /// the full 10 s `WORKER_HELLO_TIMEOUT` and then surfacing as
    /// "Resource temporarily unavailable (os error 35)" once the client's 15 s
    /// IPC read timeout expired.
    spawn_lock: Arc<Mutex<()>>,
    socket_path: PathBuf,
    binary_path: PathBuf,
    shutdown_notify: Arc<Notify>,
}

impl Supervisor {
    pub fn new(db: Arc<Database>, socket_path: PathBuf, binary_path: PathBuf) -> Self {
        let proxy = ProxyService::new(db.clone());
        Self {
            db,
            proxy,
            inner: Arc::new(Mutex::new(SupervisorInner::default())),
            spawn_lock: Arc::new(Mutex::new(())),
            socket_path,
            binary_path,
            shutdown_notify: Arc::new(Notify::new()),
        }
    }

    pub fn shutdown_signal(&self) -> Arc<Notify> {
        self.shutdown_notify.clone()
    }

    pub async fn recover_on_startup(&self) -> Result<(), String> {
        self.proxy.recover_takeovers_on_startup().await
    }

    /// Bring up a worker if none is running, then return its bound address.
    ///
    /// Concurrent callers serialize through `spawn_lock` so we never spawn two
    /// workers in parallel (which would fight for the listen port and corrupt
    /// `pending_hello`). After acquiring the lock we re-check `inner.worker` so
    /// later callers reuse the worker the first one brought up.
    async fn ensure_worker(&self, app: AppType) -> Result<WorkerInfo, String> {
        let _spawn_guard = self.spawn_lock.lock().await;
        let app_key = app.as_str().to_string();

        let (session_token, hello_rx) = {
            let mut inner = self.inner.lock().await;
            if let Some(info) = inner.workers.get(&app).cloned() {
                return Ok(info);
            }
            let (tx, rx) = oneshot::channel();
            inner.pending_hellos.insert(app_key.clone(), tx);
            let token = uuid::Uuid::new_v4().to_string();
            inner.pending_tokens.insert(app_key.clone(), token.clone());
            (token, rx)
        };

        let app_config = self
            .db
            .get_proxy_config_for_app(&app_key)
            .await
            .map_err(|err| format!("load proxy config for {app_key} failed: {err}"))?;
        let global_config = self
            .db
            .get_global_proxy_config()
            .await
            .map_err(|err| format!("load global proxy config failed: {err}"))?;

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("proxy")
            .arg("serve")
            .arg("--listen-address")
            .arg(global_config.listen_address)
            .arg("--listen-port")
            .arg(app_config.listen_port.to_string())
            .env(DAEMON_SOCKET_ENV, &self.socket_path)
            .env(SESSION_TOKEN_ENV, &session_token)
            .env(RESTORE_GUARD_BYPASS_ENV, "1")
            .env(RUNTIME_KIND_ENV, RUNTIME_KIND_MANAGED_EXTERNAL)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let spawned = cmd
            .spawn()
            .map_err(|err| format!("spawn {app_key} proxy worker failed: {err}"))?;
        let pid = spawned
            .id()
            .ok_or_else(|| format!("spawned {app_key} worker has no pid"))?;
        log::info!("[daemon] spawned {app_key} worker pid={pid}");

        let supervisor = self.clone();
        let watch_app = app.clone();
        tokio::spawn(async move {
            supervisor.watch_worker(spawned, watch_app, pid).await;
        });

        let info = match tokio::time::timeout(WORKER_HELLO_TIMEOUT, hello_rx).await {
            Ok(Ok(info)) => info,
            Ok(Err(_)) => return Err(format!("{app_key} worker exited before hello")),
            Err(_) => return Err(format!("{app_key} worker hello timed out")),
        };

        {
            let mut inner = self.inner.lock().await;
            inner.workers.insert(app.clone(), info.clone());
            inner.last_restart_at = Some(chrono::Utc::now());
            inner.restart.on_worker_started(Instant::now());
            inner.pending_tokens.remove(&app_key);
        }
        self.persist_runtime_session().await?;
        Ok(info)
    }

    async fn handle_ensure_worker(&self, app_type: &str) -> Response {
        let app = match parse_app_type(app_type) {
            Some(a) => a,
            None => {
                return Response::Error {
                    message: format!("proxy takeover not supported for app: {app_type}"),
                };
            }
        };

        let info = match self.ensure_worker(app.clone()).await {
            Ok(info) => info,
            Err(err) => {
                return Response::Error { message: err };
            }
        };

        if let Err(err) = self.proxy.set_global_enabled(true).await {
            return Response::Error {
                message: err.to_string(),
            };
        }

        if let Err(err) = self.proxy.set_takeover_for_app(app.as_str(), true).await {
            return Response::Error { message: err };
        }

        Response::Worker {
            address: info.address,
            port: info.port,
            session_token: info.session_token,
            pid: info.pid,
        }
    }

    async fn handle_drop_takeover(&self, app_type: &str) -> Response {
        let app = match parse_app_type(app_type) {
            Some(a) => a,
            None => {
                return Response::Error {
                    message: format!("proxy takeover not supported for app: {app_type}"),
                };
            }
        };

        if let Err(err) = self.proxy.set_takeover_for_app(app.as_str(), false).await {
            return Response::Error { message: err };
        }

        let (stop_pid, had_worker, should_shutdown) = {
            let mut inner = self.inner.lock().await;
            let pid = inner.workers.get(&app).map(|w| w.pid);
            if pid.is_some() {
                inner.stopping_workers.insert(app.clone());
                if inner.workers.len() <= 1 {
                    inner.shutdown_requested = true;
                }
            }
            (pid, pid.is_some(), inner.workers.len() <= 1)
        };
        let _ = send_sigterm(stop_pid);
        if had_worker {
            tokio::time::sleep(Duration::from_millis(100)).await;
        } else if should_shutdown {
            self.shutdown_notify.notify_waiters();
        }
        Response::Ok
    }

    async fn handle_worker_hello(
        &self,
        pid: u32,
        address: String,
        port: u16,
        session_token: String,
    ) -> Response {
        let mut inner = self.inner.lock().await;
        let app_key = inner
            .pending_tokens
            .iter()
            .find_map(|(app_type, token)| (token == &session_token).then(|| app_type.clone()));
        let Some(app_key) = app_key else {
            log::warn!("[daemon] worker hello with mismatched token (pid={pid})");
            return Response::Error {
                message: "session token mismatch".to_string(),
            };
        };
        let Some(tx) = inner.pending_hellos.remove(&app_key) else {
            log::warn!("[daemon] worker hello received but no pending ensure (pid={pid})");
            return Response::Error {
                message: "no pending worker registration".to_string(),
            };
        };
        let Some(app_type) = parse_app_type(&app_key) else {
            return Response::Error {
                message: format!("proxy takeover not supported for app: {app_key}"),
            };
        };
        let info = WorkerInfo {
            app_type,
            pid,
            address,
            port,
            session_token,
        };
        if tx.send(info).is_err() {
            log::warn!("[daemon] worker hello dropped: ensure waiter cancelled");
        }
        Response::Ok
    }

    async fn handle_set_global_enabled(&self, enabled: bool) -> Response {
        if enabled {
            match self.proxy.set_global_enabled(true).await {
                Ok(_) => return Response::Ok,
                Err(err) => {
                    return Response::Error {
                        message: err.to_string(),
                    };
                }
            }
        }

        // Disabling: drop every active takeover so each app's live config is
        // restored, then stop the worker. We snapshot the active list under
        // the inner lock so we don't hold it while running per-app restores
        // (which acquire the file-level state mutation guard).
        let mut active = Vec::new();
        for app in [AppType::Claude, AppType::Codex, AppType::Gemini] {
            match self.db.get_proxy_config_for_app(app.as_str()).await {
                Ok(config) if config.enabled => active.push(app),
                Ok(_) => {}
                Err(err) => log::warn!(
                    "[daemon] set_global_enabled(false): read {} proxy config failed: {err}",
                    app.as_str()
                ),
            }
        }
        for app in &active {
            if let Err(err) = self.proxy.set_takeover_for_app(app.as_str(), false).await {
                log::warn!(
                    "[daemon] set_global_enabled(false): drop takeover for {} failed: {err}",
                    app.as_str()
                );
            }
        }

        let stop_pids = {
            let mut inner = self.inner.lock().await;
            inner.shutdown_requested = true;
            inner.workers.values().map(|w| w.pid).collect::<Vec<_>>()
        };
        for pid in &stop_pids {
            let _ = send_sigterm(Some(*pid));
        }
        if !stop_pids.is_empty() {
            // Brief pause so the worker has a chance to exit and the watcher
            // task can clear the runtime session row before we ack. The
            // watcher then sees `active_takeovers.is_empty()` and signals
            // daemon shutdown.
            tokio::time::sleep(Duration::from_millis(100)).await;
        } else {
            // No worker to drain — signal shutdown directly so the daemon
            // doesn't stay idle after a "disable everything" with nothing
            // currently running.
            self.shutdown_notify.notify_waiters();
        }
        Response::Ok
    }

    async fn handle_status(&self) -> Response {
        let inner = self.inner.lock().await;
        let takeovers = self.read_takeover_flags().await;
        let mut workers = inner
            .workers
            .values()
            .map(|info| WorkerState {
                app_type: info.app_type.as_str().to_string(),
                running: true,
                address: info.address.clone(),
                port: info.port,
                pid: Some(info.pid),
            })
            .collect::<Vec<_>>();
        workers.sort_by(|left, right| left.app_type.cmp(&right.app_type));
        let primary = workers.first();
        Response::Status {
            running: !workers.is_empty(),
            address: primary.map(|w| w.address.clone()).unwrap_or_default(),
            port: primary.map(|w| w.port).unwrap_or_default(),
            worker_pid: primary.and_then(|w| w.pid),
            takeovers,
            restart_count: inner.restart_count,
            last_restart_at: inner.last_restart_at.map(|d| d.to_rfc3339()),
            workers,
        }
    }

    pub async fn shutdown(&self) {
        let stop_pids = {
            let mut inner = self.inner.lock().await;
            inner.shutdown_requested = true;
            inner.workers.values().map(|w| w.pid).collect::<Vec<_>>()
        };
        for pid in stop_pids {
            let _ = send_sigterm(Some(pid));
        }
        if let Err(err) = self.proxy.stop_with_restore().await {
            log::warn!("[daemon] shutdown: stop_with_restore failed: {err}");
        }
        let _ = self.clear_runtime_session();
        self.shutdown_notify.notify_waiters();
    }

    async fn handle_shutdown(&self) -> Response {
        self.shutdown().await;
        Response::Ok
    }

    async fn read_takeover_flags(&self) -> TakeoverFlags {
        let status = self.proxy.get_takeover_status().await.unwrap_or_default();
        TakeoverFlags {
            claude: status.claude,
            codex: status.codex,
            gemini: status.gemini,
        }
    }

    async fn watch_worker(&self, mut child: Child, app: AppType, pid: u32) {
        let app_key = app.as_str().to_string();
        let exit_status = match child.wait().await {
            Ok(status) => status,
            Err(err) => {
                log::warn!("[daemon] waitpid {app_key} worker={pid} failed: {err}");
                return;
            }
        };
        log::info!("[daemon] {app_key} worker pid={pid} exited: {exit_status}");

        let (intentional, has_remaining_workers) = {
            let mut inner = self.inner.lock().await;
            inner.workers.remove(&app);
            inner.pending_tokens.remove(&app_key);
            if let Some(tx) = inner.pending_hellos.remove(&app_key) {
                drop(tx);
            }
            let intentional = inner.shutdown_requested || inner.stopping_workers.remove(&app);
            (intentional, !inner.workers.is_empty())
        };

        let _ = self.persist_runtime_session().await;

        if intentional {
            log::info!("[daemon] {app_key} worker exit was expected, not restarting");
            if !has_remaining_workers {
                log::info!("[daemon] no remaining workers, exiting");
                self.shutdown_notify.notify_waiters();
            }
            return;
        }

        if let Err(err) = self.proxy.set_takeover_for_app(app.as_str(), false).await {
            log::warn!("[daemon] restore takeover for {app_key} failed: {err}");
        }

        let decision = {
            let mut inner = self.inner.lock().await;
            inner.restart.on_worker_exited(Instant::now())
        };

        match decision {
            Decision::Restart { delay, attempt } => {
                log::warn!(
                    "[daemon] {app_key} worker pid={pid} crashed; restarting in {:?} (attempt {})",
                    delay,
                    attempt + 1
                );
                tokio::time::sleep(delay).await;
                {
                    let mut inner = self.inner.lock().await;
                    inner.restart_count = inner.restart_count.saturating_add(1);
                }
                if let Err(err) = self.respawn_after_crash(app).await {
                    log::error!("[daemon] respawn {app_key} after crash failed: {err}");
                }
            }
            Decision::GiveUp => {
                log::error!(
                    "[daemon] {app_key} worker pid={pid} circuit-broke after repeated crashes"
                );
                if !has_remaining_workers {
                    self.shutdown_notify.notify_waiters();
                }
            }
        }
    }

    fn respawn_after_crash<'a>(
        &'a self,
        app: AppType,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            let _info = self.ensure_worker(app.clone()).await?;
            if let Err(err) = self.proxy.set_takeover_for_app(app.as_str(), true).await {
                log::warn!(
                    "[daemon] re-applying takeover for {} after restart failed: {err}",
                    app.as_str()
                );
            }
            Ok(())
        })
    }

    async fn persist_runtime_session(&self) -> Result<(), String> {
        let workers = {
            let inner = self.inner.lock().await;
            inner
                .workers
                .iter()
                .map(|(app, info)| {
                    (
                        app.as_str().to_string(),
                        json!({
                            "pid": info.pid,
                            "address": info.address,
                            "port": info.port,
                            "started_at": chrono::Utc::now().to_rfc3339(),
                            "kind": "managed_external",
                            "session_token": info.session_token,
                            "app_type": app.as_str(),
                        }),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
        };
        if workers.is_empty() {
            return self.clear_runtime_session();
        }
        let payload = json!({ "workers": workers });
        let serialized = serde_json::to_string(&payload)
            .map_err(|err| format!("serialize runtime session failed: {err}"))?;
        self.db
            .set_setting(PROXY_RUNTIME_SESSION_KEY, &serialized)
            .map_err(|err| format!("persist runtime session failed: {err}"))
    }

    fn clear_runtime_session(&self) -> Result<(), String> {
        self.db
            .delete_setting(PROXY_RUNTIME_SESSION_KEY)
            .map_err(|err| format!("clear runtime session failed: {err}"))
    }
}

impl Handler for Supervisor {
    async fn handle(&self, request: Request) -> Response {
        match request {
            Request::EnsureWorker { app_type } => self.handle_ensure_worker(&app_type).await,
            Request::DropTakeover { app_type } => self.handle_drop_takeover(&app_type).await,
            Request::Status => self.handle_status().await,
            Request::WorkerHello {
                pid,
                address,
                port,
                session_token,
            } => {
                self.handle_worker_hello(pid, address, port, session_token)
                    .await
            }
            Request::SetGlobalEnabled { enabled } => self.handle_set_global_enabled(enabled).await,
            Request::Shutdown => self.handle_shutdown().await,
        }
    }
}

fn parse_app_type(s: &str) -> Option<AppType> {
    match s {
        "claude" => Some(AppType::Claude),
        "codex" => Some(AppType::Codex),
        "gemini" => Some(AppType::Gemini),
        _ => None,
    }
}

fn send_sigterm(pid: Option<u32>) -> Result<(), String> {
    let Some(pid) = pid else {
        return Ok(());
    };
    if pid == 0 {
        return Ok(());
    }
    unsafe {
        let rc = libc::kill(pid as i32, libc::SIGTERM);
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ESRCH) {
                return Err(format!("SIGTERM worker {pid}: {err}"));
            }
        }
    }
    Ok(())
}
