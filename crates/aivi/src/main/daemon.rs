use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

use aivi::cg_type::CgType;
use aivi::{
    AiviError, AssemblyStats, DiagnosticSeverity, FrontendAssemblyMode, HirProgram,
    WorkspaceSession,
};

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

const DAEMON_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DaemonIdentity {
    exe_path: PathBuf,
    exe_len: u64,
    exe_modified_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PreparedCompileArtifacts {
    pub(crate) program: HirProgram,
    pub(crate) cg_types: HashMap<String, HashMap<String, CgType>>,
    pub(crate) monomorph_plan: HashMap<String, Vec<CgType>>,
    pub(crate) source_schemas: HashMap<String, Vec<CgType>>,
    pub(crate) constructor_ordinals: HashMap<String, Option<usize>>,
    pub(crate) crate_natives: Vec<aivi::native_bridge::CrateNativeBinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CompileSummary {
    pub(crate) compiled_modules: Vec<String>,
    pub(crate) reused_modules: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum PrepareCompileFailure {
    Diagnostics {
        rendered: String,
        summary: CompileSummary,
    },
    Error(AiviError),
}

impl From<AiviError> for PrepareCompileFailure {
    fn from(value: AiviError) -> Self {
        Self::Error(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Envelope<T> {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    identity: Option<DaemonIdentity>,
    payload: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonIdentityState {
    Compatible,
    Missing,
    Mismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum DaemonRequest {
    Ping,
    Shutdown,
    CompileProject {
        source_target: String,
        root_modules: Vec<String>,
        use_color: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum DaemonResponse {
    Pong,
    Ack,
    CompileOk {
        artifacts: Box<PreparedCompileArtifacts>,
        summary: CompileSummary,
    },
    Diagnostics {
        rendered: String,
        summary: CompileSummary,
    },
    Error {
        message: String,
    },
}

#[cfg(unix)]
#[derive(Debug, Clone)]
struct ProjectDaemonPaths {
    root: PathBuf,
    socket: PathBuf,
    pid: PathBuf,
}

#[cfg(unix)]
#[derive(Debug, Clone)]
enum ProjectDaemonState {
    Compatible,
    Incompatible(Option<DaemonIdentity>),
    Stopped,
}

pub(crate) fn compile_target_in_fresh_session(
    target: &str,
    root_modules: &[String],
    use_color: bool,
) -> Result<(PreparedCompileArtifacts, CompileSummary), PrepareCompileFailure> {
    let mut session = WorkspaceSession::new();
    compile_target_with_session(&mut session, target, root_modules, use_color)
}

pub(crate) fn compile_project_with_optional_daemon(
    project_root: &Path,
    target: &str,
    root_modules: &[String],
    use_color: bool,
) -> Result<(PreparedCompileArtifacts, CompileSummary), PrepareCompileFailure> {
    if daemon_disabled_for_current_process() {
        return compile_target_in_fresh_session(target, root_modules, use_color);
    }

    #[cfg(unix)]
    {
        match ensure_project_daemon(project_root).and_then(|paths| {
            send_daemon_request(
                &paths,
                &DaemonRequest::CompileProject {
                    source_target: target.to_string(),
                    root_modules: root_modules.to_vec(),
                    use_color,
                },
            )
        }) {
            Ok(DaemonResponse::CompileOk { artifacts, summary }) => Ok((*artifacts, summary)),
            Ok(DaemonResponse::Diagnostics { rendered, summary }) => {
                Err(PrepareCompileFailure::Diagnostics { rendered, summary })
            }
            Ok(DaemonResponse::Error { message }) => {
                eprintln!("[daemon] compile failed, falling back to local frontend: {message}");
                compile_target_in_fresh_session(target, root_modules, use_color)
            }
            Ok(other) => {
                eprintln!("[daemon] unexpected response {other:?}, falling back to local frontend");
                compile_target_in_fresh_session(target, root_modules, use_color)
            }
            Err(err) => {
                eprintln!("[daemon] unavailable, falling back to local frontend: {err}");
                compile_target_in_fresh_session(target, root_modules, use_color)
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = project_root;
        compile_target_in_fresh_session(target, root_modules, use_color)
    }
}

pub(crate) fn print_compile_summary(summary: &CompileSummary) {
    if !summary.compiled_modules.is_empty() {
        eprintln!("  Compiling {} module(s):", summary.compiled_modules.len());
        for module in &summary.compiled_modules {
            eprintln!("    - {module}");
        }
        return;
    }

    if !summary.reused_modules.is_empty() {
        eprintln!(
            "  Reusing cached frontend modules ({})",
            summary.reused_modules.len()
        );
    }
}

pub(crate) fn cmd_daemon(args: &[String]) -> Result<(), AiviError> {
    let Some(subcommand) = args.first() else {
        return Err(AiviError::InvalidCommand(
            "daemon expects start|status|stop".to_string(),
        ));
    };

    match subcommand.as_str() {
        "start" => cmd_daemon_start(),
        "status" => cmd_daemon_status(),
        "stop" => cmd_daemon_stop(),
        #[cfg(unix)]
        "serve" => cmd_daemon_serve(&args[1..]),
        other => Err(AiviError::InvalidCommand(format!("daemon {other}"))),
    }
}

fn trace_timing() -> bool {
    env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1")
}

fn daemon_disabled_for_current_process() -> bool {
    env::var("AIVI_NO_DAEMON").is_ok_and(|v| v == "1") || trace_timing()
}

fn compile_target_with_session(
    session: &mut WorkspaceSession,
    target: &str,
    root_modules: &[String],
    use_color: bool,
) -> Result<(PreparedCompileArtifacts, CompileSummary), PrepareCompileFailure> {
    let trace = trace_timing();
    let total_started = trace.then(Instant::now);
    let assembly_started = trace.then(Instant::now);
    let assembly = session.assemble_target_with_roots(
        target,
        root_modules,
        FrontendAssemblyMode::InferFast,
    )?;
    if let Some(started) = assembly_started {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms",
            "frontend assembly",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }

    let summary = compile_summary_from_stats(&assembly.stats);
    if diagnostics_have_errors(&assembly.parse_diagnostics) {
        return Err(PrepareCompileFailure::Diagnostics {
            rendered: render_diagnostics_text(&assembly.parse_diagnostics, use_color),
            summary,
        });
    }

    let mut diagnostics = assembly.resolver_diagnostics.clone();
    diagnostics.extend(assembly.typecheck_diagnostics.clone());
    if let Some(inference) = &assembly.inference {
        diagnostics.extend(inference.diagnostics.clone());
    }
    if diagnostics_have_errors(&diagnostics) {
        return Err(PrepareCompileFailure::Diagnostics {
            rendered: render_diagnostics_text(&diagnostics, use_color),
            summary,
        });
    }

    let desugar_started = trace.then(Instant::now);
    let program = assembly.desugar();
    if let Some(started) = desugar_started {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms",
            "desugar_modules (HIR)",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }

    let infer = assembly
        .inference
        .expect("infer-fast assembly should always include inference");
    if let Some(started) = total_started {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms  ← TOTAL frontend",
            "frontend pipeline",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }

    let artifacts = PreparedCompileArtifacts {
        program,
        cg_types: infer.cg_types,
        monomorph_plan: infer.monomorph_plan,
        source_schemas: infer.source_schemas,
        constructor_ordinals: collect_surface_constructor_ordinals(&assembly.modules),
        crate_natives: aivi::native_bridge::collect_crate_natives(&assembly.modules),
    };
    Ok((artifacts, summary))
}

fn compile_summary_from_stats(stats: &AssemblyStats) -> CompileSummary {
    let mut compiled_modules = if !stats.reinferred_modules.is_empty() {
        stats.reinferred_modules.clone()
    } else if !stats.reelaborated_modules.is_empty() {
        stats.reelaborated_modules.clone()
    } else if !stats.rechecked_modules.is_empty() {
        stats.rechecked_modules.clone()
    } else {
        stats.invalidated_modules.clone()
    };
    compiled_modules.sort();
    compiled_modules.dedup();

    let mut reused_modules = stats.reused_modules.clone();
    reused_modules.sort();
    reused_modules.dedup();

    CompileSummary {
        compiled_modules,
        reused_modules,
    }
}

fn current_daemon_identity() -> Option<DaemonIdentity> {
    let exe_path = env::current_exe().ok()?;
    let exe_path = fs::canonicalize(&exe_path).unwrap_or(exe_path);
    let metadata = fs::metadata(&exe_path).ok()?;
    let exe_modified_ms = metadata
        .modified()
        .ok()
        .and_then(|timestamp| timestamp.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0);
    Some(DaemonIdentity {
        exe_path,
        exe_len: metadata.len(),
        exe_modified_ms,
    })
}

fn daemon_envelope<T>(payload: T) -> Envelope<T> {
    Envelope {
        version: DAEMON_PROTOCOL_VERSION,
        identity: current_daemon_identity(),
        payload,
    }
}

fn daemon_identity_state(
    expected: Option<&DaemonIdentity>,
    actual: Option<&DaemonIdentity>,
) -> DaemonIdentityState {
    match (expected, actual) {
        (Some(expected), Some(actual)) if expected == actual => DaemonIdentityState::Compatible,
        (Some(_), Some(_)) => DaemonIdentityState::Mismatch,
        (Some(_), None) => DaemonIdentityState::Missing,
        (None, _) => DaemonIdentityState::Compatible,
    }
}

fn current_daemon_identity_state(actual: Option<&DaemonIdentity>) -> DaemonIdentityState {
    let expected = current_daemon_identity();
    daemon_identity_state(expected.as_ref(), actual)
}

fn format_daemon_identity(identity: Option<&DaemonIdentity>) -> String {
    match identity {
        Some(identity) => format!(
            "{} (len={}, mtime_ms={})",
            identity.exe_path.display(),
            identity.exe_len,
            identity.exe_modified_ms
        ),
        None => "<missing identity>".to_string(),
    }
}

fn daemon_identity_mismatch_message(actual: Option<&DaemonIdentity>) -> Option<String> {
    let expected = current_daemon_identity();
    match daemon_identity_state(expected.as_ref(), actual) {
        DaemonIdentityState::Compatible => None,
        DaemonIdentityState::Missing => Some(format!(
            "project daemon did not report its executable identity; current binary is {}",
            format_daemon_identity(expected.as_ref())
        )),
        DaemonIdentityState::Mismatch => Some(format!(
            "project daemon is using {}, but current binary is {}",
            format_daemon_identity(actual),
            format_daemon_identity(expected.as_ref())
        )),
    }
}

fn diagnostics_have_errors(diagnostics: &[aivi::FileDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diag| diag.diagnostic.severity == DiagnosticSeverity::Error)
}

fn render_diagnostics_text(diagnostics: &[aivi::FileDiagnostic], use_color: bool) -> String {
    let mut rendered = String::new();
    for diag in diagnostics {
        let text = aivi::render_diagnostics(
            &diag.path,
            std::slice::from_ref(&diag.diagnostic),
            use_color,
        );
        if !text.is_empty() {
            rendered.push_str(&text);
        }
    }
    rendered
}

fn collect_surface_constructor_ordinals(
    surface_modules: &[aivi::Module],
) -> HashMap<String, Option<usize>> {
    let mut ordinals = HashMap::new();
    for module in surface_modules {
        for item in &module.items {
            match item {
                aivi::ModuleItem::TypeDecl(decl) => {
                    for (ordinal, ctor) in decl.constructors.iter().enumerate() {
                        ordinals
                            .entry(ctor.name.name.clone())
                            .or_insert(Some(ordinal));
                    }
                }
                aivi::ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        let aivi::DomainItem::TypeAlias(decl) = domain_item else {
                            continue;
                        };
                        for (ordinal, ctor) in decl.constructors.iter().enumerate() {
                            ordinals
                                .entry(ctor.name.name.clone())
                                .or_insert(Some(ordinal));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    ordinals
}

fn current_project_root() -> Result<PathBuf, AiviError> {
    let cwd = env::current_dir()?;
    for ancestor in cwd.ancestors() {
        if ancestor.join("aivi.toml").is_file() {
            return Ok(ancestor.to_path_buf());
        }
    }
    Err(AiviError::Config(
        "daemon expects to run inside an AIVI project".to_string(),
    ))
}

fn cmd_daemon_start() -> Result<(), AiviError> {
    #[cfg(unix)]
    {
        let root = current_project_root()?;
        let paths = ensure_project_daemon(&root)?;
        println!("running {}", paths.socket.display());
        Ok(())
    }

    #[cfg(not(unix))]
    {
        Err(AiviError::InvalidCommand(
            "daemon is only available on unix platforms in v0.1".to_string(),
        ))
    }
}

fn cmd_daemon_status() -> Result<(), AiviError> {
    #[cfg(unix)]
    {
        let root = current_project_root()?;
        let paths = daemon_paths_for_root(&root)?;
        match project_daemon_state(&paths) {
            ProjectDaemonState::Compatible => println!("running {}", paths.socket.display()),
            ProjectDaemonState::Incompatible(identity) => {
                println!("running {} (stale)", paths.socket.display());
                if let Some(message) = daemon_identity_mismatch_message(identity.as_ref()) {
                    println!("  {message}");
                }
            }
            ProjectDaemonState::Stopped => println!("stopped {}", paths.socket.display()),
        }
        Ok(())
    }

    #[cfg(not(unix))]
    {
        Err(AiviError::InvalidCommand(
            "daemon is only available on unix platforms in v0.1".to_string(),
        ))
    }
}

fn cmd_daemon_stop() -> Result<(), AiviError> {
    #[cfg(unix)]
    {
        let root = current_project_root()?;
        let paths = daemon_paths_for_root(&root)?;
        match project_daemon_state(&paths) {
            ProjectDaemonState::Stopped => {
                println!("stopped {}", paths.socket.display());
                Ok(())
            }
            ProjectDaemonState::Compatible | ProjectDaemonState::Incompatible(_) => {
                shutdown_project_daemon(&paths);
                println!("stopped {}", paths.socket.display());
                Ok(())
            }
        }
    }

    #[cfg(not(unix))]
    {
        Err(AiviError::InvalidCommand(
            "daemon is only available on unix platforms in v0.1".to_string(),
        ))
    }
}

#[cfg(unix)]
fn cmd_daemon_serve(args: &[String]) -> Result<(), AiviError> {
    let mut root = None;
    let mut socket = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => root = iter.next().map(PathBuf::from),
            "--socket" => socket = iter.next().map(PathBuf::from),
            other => {
                return Err(AiviError::InvalidCommand(format!(
                    "unexpected daemon serve argument {other}"
                )))
            }
        }
    }
    let Some(root) = root else {
        return Err(AiviError::InvalidCommand(
            "daemon serve requires --root <path>".to_string(),
        ));
    };
    let Some(socket) = socket else {
        return Err(AiviError::InvalidCommand(
            "daemon serve requires --socket <path>".to_string(),
        ));
    };

    run_project_daemon(&root, &socket)
}

#[cfg(unix)]
fn run_project_daemon(root: &Path, socket: &Path) -> Result<(), AiviError> {
    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent)?;
    }
    let pid_path = socket.with_extension("pid");
    let _ = fs::remove_file(socket);
    let listener = UnixListener::bind(socket)?;
    fs::write(&pid_path, std::process::id().to_string())?;
    let _cleanup = DaemonCleanup {
        socket: socket.to_path_buf(),
        pid: pid_path,
    };
    env::set_current_dir(root)?;

    let mut session = WorkspaceSession::new();
    loop {
        let (stream, _addr) = listener.accept()?;
        if !handle_connection(&mut session, stream)? {
            break;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn handle_connection(
    session: &mut WorkspaceSession,
    mut stream: UnixStream,
) -> Result<bool, AiviError> {
    let request: Envelope<DaemonRequest> = read_json_line(&mut stream)?;
    if request.version != DAEMON_PROTOCOL_VERSION {
        write_json_line(
            &mut stream,
            &daemon_envelope(DaemonResponse::Error {
                message: format!(
                    "protocol mismatch: client {}, daemon {}",
                    request.version, DAEMON_PROTOCOL_VERSION
                ),
            }),
        )?;
        return Ok(true);
    }

    match request.payload {
        DaemonRequest::Ping => {
            write_json_line(&mut stream, &daemon_envelope(DaemonResponse::Pong))?;
            Ok(true)
        }
        DaemonRequest::Shutdown => {
            write_json_line(&mut stream, &daemon_envelope(DaemonResponse::Ack))?;
            Ok(false)
        }
        DaemonRequest::CompileProject {
            source_target,
            root_modules,
            use_color,
        } => {
            let payload = match compile_target_with_session(
                session,
                &source_target,
                &root_modules,
                use_color,
            ) {
                Ok((artifacts, summary)) => DaemonResponse::CompileOk {
                    artifacts: Box::new(artifacts),
                    summary,
                },
                Err(PrepareCompileFailure::Diagnostics { rendered, summary }) => {
                    DaemonResponse::Diagnostics { rendered, summary }
                }
                Err(PrepareCompileFailure::Error(err)) => DaemonResponse::Error {
                    message: err.render(false),
                },
            };
            write_json_line(&mut stream, &daemon_envelope(payload))?;
            Ok(true)
        }
    }
}

#[cfg(unix)]
fn read_json_line<T: for<'de> Deserialize<'de>>(stream: &mut UnixStream) -> Result<T, AiviError> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.trim().is_empty() {
        return Err(AiviError::runtime_message(
            "daemon sent an empty response".to_string(),
        ));
    }
    serde_json::from_str(&line)
        .map_err(|err| AiviError::runtime_message(format!("daemon json decode failed: {err}")))
}

#[cfg(unix)]
fn write_json_line<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<(), AiviError> {
    serde_json::to_writer(&mut *stream, value)
        .map_err(|err| AiviError::runtime_message(format!("daemon json encode failed: {err}")))?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

#[cfg(unix)]
fn ensure_project_daemon(project_root: &Path) -> Result<ProjectDaemonPaths, AiviError> {
    let paths = daemon_paths_for_root(project_root)?;
    match project_daemon_state(&paths) {
        ProjectDaemonState::Compatible => return Ok(paths),
        ProjectDaemonState::Incompatible(identity) => {
            if let Some(message) = daemon_identity_mismatch_message(identity.as_ref()) {
                eprintln!("[daemon] restarting stale project daemon: {message}");
            }
            shutdown_project_daemon(&paths);
        }
        ProjectDaemonState::Stopped => cleanup_stale_daemon_files(&paths),
    }
    spawn_project_daemon(&paths)?;
    for _ in 0..40 {
        if matches!(project_daemon_state(&paths), ProjectDaemonState::Compatible) {
            return Ok(paths);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(AiviError::runtime_message(format!(
        "timed out waiting for daemon {}",
        paths.socket.display()
    )))
}

#[cfg(unix)]
fn send_daemon_request(
    paths: &ProjectDaemonPaths,
    payload: &DaemonRequest,
) -> Result<DaemonResponse, AiviError> {
    let response = send_daemon_request_raw(paths, payload)?;
    if response.version != DAEMON_PROTOCOL_VERSION {
        return Err(AiviError::runtime_message(format!(
            "protocol mismatch: daemon {}, client {}",
            response.version, DAEMON_PROTOCOL_VERSION
        )));
    }
    if let Some(message) = daemon_identity_mismatch_message(response.identity.as_ref()) {
        return Err(AiviError::runtime_message(message));
    }
    Ok(response.payload)
}

#[cfg(unix)]
fn send_daemon_request_raw(
    paths: &ProjectDaemonPaths,
    payload: &DaemonRequest,
) -> Result<Envelope<DaemonResponse>, AiviError> {
    let mut stream = UnixStream::connect(&paths.socket)?;
    write_json_line(&mut stream, &daemon_envelope(payload.clone()))?;
    read_json_line(&mut stream)
}

#[cfg(unix)]
fn project_daemon_state(paths: &ProjectDaemonPaths) -> ProjectDaemonState {
    let response = match send_daemon_request_raw(paths, &DaemonRequest::Ping) {
        Ok(response) => response,
        Err(_) => return ProjectDaemonState::Stopped,
    };
    if response.version != DAEMON_PROTOCOL_VERSION {
        return ProjectDaemonState::Incompatible(response.identity);
    }
    if !matches!(response.payload, DaemonResponse::Pong) {
        return ProjectDaemonState::Stopped;
    }
    match current_daemon_identity_state(response.identity.as_ref()) {
        DaemonIdentityState::Compatible => ProjectDaemonState::Compatible,
        DaemonIdentityState::Missing | DaemonIdentityState::Mismatch => {
            ProjectDaemonState::Incompatible(response.identity)
        }
    }
}

#[cfg(unix)]
fn shutdown_project_daemon(paths: &ProjectDaemonPaths) {
    let _ = send_daemon_request_raw(paths, &DaemonRequest::Shutdown);
    for _ in 0..20 {
        if !paths.socket.exists() && !paths.pid.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    cleanup_stale_daemon_files(paths);
}

#[cfg(unix)]
fn spawn_project_daemon(paths: &ProjectDaemonPaths) -> Result<(), AiviError> {
    let exe = env::current_exe()?;
    let _child = Command::new(exe)
        .arg("daemon")
        .arg("serve")
        .arg("--root")
        .arg(&paths.root)
        .arg("--socket")
        .arg(&paths.socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(unix)]
fn cleanup_stale_daemon_files(paths: &ProjectDaemonPaths) {
    let _ = fs::remove_file(&paths.socket);
    let _ = fs::remove_file(&paths.pid);
}

#[cfg(unix)]
fn daemon_paths_for_root(project_root: &Path) -> Result<ProjectDaemonPaths, AiviError> {
    let root = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    let hash = hasher.finish();

    let runtime_dir = daemon_runtime_dir()?;
    fs::create_dir_all(&runtime_dir)?;
    let socket = runtime_dir.join(format!("{hash:016x}.sock"));
    let pid = runtime_dir.join(format!("{hash:016x}.pid"));
    Ok(ProjectDaemonPaths { root, socket, pid })
}

#[cfg(unix)]
fn daemon_runtime_dir() -> Result<PathBuf, AiviError> {
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(dir).join("aivi-daemon"));
    }
    let uid = unsafe { libc::geteuid() };
    Ok(env::temp_dir().join(format!("aivi-daemon-{uid}")))
}

#[cfg(unix)]
struct DaemonCleanup {
    socket: PathBuf,
    pid: PathBuf,
}

#[cfg(unix)]
impl Drop for DaemonCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
        let _ = fs::remove_file(&self.pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity(path: &str, len: u64, modified_ms: u64) -> DaemonIdentity {
        DaemonIdentity {
            exe_path: PathBuf::from(path),
            exe_len: len,
            exe_modified_ms: modified_ms,
        }
    }

    #[test]
    fn compile_summary_prefers_reinferred_modules() {
        let summary = compile_summary_from_stats(&AssemblyStats {
            reinferred_modules: vec!["app.main".to_string(), "app.helper".to_string()],
            reused_modules: vec!["aivi.list".to_string()],
            ..AssemblyStats::default()
        });
        assert_eq!(summary.compiled_modules, vec!["app.helper", "app.main"]);
        assert_eq!(summary.reused_modules, vec!["aivi.list"]);
    }

    #[test]
    fn compile_target_in_fresh_session_reuses_modules_on_second_run() {
        let temp = tempfile::tempdir().expect("tempdir");
        let main_path = temp.path().join("main.aivi");
        let helper_path = temp.path().join("helper.aivi");
        fs::write(
            &main_path,
            "module daemonTest.main\n\nuse daemonTest.helper (value)\n\nexport main\n\nmain = value\n",
        )
        .expect("write main");
        fs::write(
            &helper_path,
            "module daemonTest.helper\n\nexport value\n\nvalue = 1\n",
        )
        .expect("write helper");

        let target = temp.path().display().to_string() + "/...";
        let roots = vec!["daemonTest.main".to_string()];
        let mut session = WorkspaceSession::new();

        let (_first, first_summary) =
            compile_target_with_session(&mut session, &target, &roots, false)
                .expect("first compile");
        assert!(first_summary
            .compiled_modules
            .contains(&"daemonTest.main".to_string()));

        let (_second, second_summary) =
            compile_target_with_session(&mut session, &target, &roots, false)
                .expect("second compile");
        assert!(second_summary.compiled_modules.is_empty());
        assert!(second_summary
            .reused_modules
            .contains(&"daemonTest.main".to_string()));
    }

    #[test]
    fn daemon_identity_state_accepts_matching_identity() {
        let identity = test_identity("/tmp/aivi", 42, 1234);
        assert_eq!(
            daemon_identity_state(Some(&identity), Some(&identity)),
            DaemonIdentityState::Compatible
        );
    }

    #[test]
    fn daemon_identity_state_rejects_missing_identity() {
        let identity = test_identity("/tmp/aivi", 42, 1234);
        assert_eq!(
            daemon_identity_state(Some(&identity), None),
            DaemonIdentityState::Missing
        );
    }

    #[test]
    fn daemon_identity_state_rejects_mismatched_identity() {
        let expected = test_identity("/tmp/aivi-debug", 42, 1234);
        let actual = test_identity("/home/me/.cargo/bin/aivi", 24, 4321);
        assert_eq!(
            daemon_identity_state(Some(&expected), Some(&actual)),
            DaemonIdentityState::Mismatch
        );
    }
}
