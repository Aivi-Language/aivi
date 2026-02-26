use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use aivi::{AiviError, CancelHandle};
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};

/// Run an AIVI program in watch mode: compile + execute, then re-run on file
/// changes. Compile errors are printed but do not exit the loop.
pub(crate) fn run_watch(target: &str, watch_dir: &Path) -> Result<(), AiviError> {
    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(300), tx)
        .map_err(|e| AiviError::Io(std::io::Error::other(format!("watcher init: {e}"))))?;

    debouncer
        .watcher()
        .watch(watch_dir, RecursiveMode::Recursive)
        .map_err(|e| AiviError::Io(std::io::Error::other(format!("watch: {e}"))))?;

    let ctrl_c = Arc::new(AtomicBool::new(false));
    install_ctrlc_handler(ctrl_c.clone());

    eprintln!(
        "\x1b[1;36m[watch]\x1b[0m watching {} for changes…",
        watch_dir.display()
    );

    loop {
        let since = SystemTime::now();
        let cancel = CancelHandle::new();
        let target_owned = target.to_string();
        let cancel_for_thread = cancel.clone();

        let runner = thread::spawn(move || -> Result<(), AiviError> {
            let (program, cg_types, monomorph_plan) =
                aivi::desugar_target_with_cg_types(&target_owned)?;
            aivi::run_cranelift_jit_with_handle(
                program,
                cg_types,
                monomorph_plan,
                &cancel_for_thread,
            )
        });

        // Wait for file change, program exit, or Ctrl-C
        let restart = loop {
            if ctrl_c.load(Ordering::Relaxed) {
                cancel.cancel();
                let _ = runner.join();
                eprintln!("\n\x1b[1;36m[watch]\x1b[0m interrupted — exiting.");
                return Ok(());
            }

            if runner.is_finished() {
                break false;
            }

            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(events)) if has_aivi_change(&events, since) => break true,
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    eprintln!("\x1b[1;33m[watch]\x1b[0m watcher error: {e:?}");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    cancel.cancel();
                    let _ = runner.join();
                    return Err(AiviError::Io(std::io::Error::other(
                        "file watcher disconnected",
                    )));
                }
            }
        };

        if restart {
            cancel.cancel();
            let _ = runner.join();
            eprintln!(
                "\n\x1b[1;36m[watch]\x1b[0m file changed — restarting…\n\
                 ─────────────────────────────────────────────"
            );
        } else {
            match runner.join() {
                Ok(Ok(())) => {
                    eprintln!("\x1b[1;32m[watch]\x1b[0m program exited successfully.");
                }
                Ok(Err(AiviError::Diagnostics)) => {
                    eprintln!("\x1b[1;31m[watch]\x1b[0m compile errors — waiting for changes…");
                }
                Ok(Err(err)) => {
                    eprintln!("\x1b[1;31m[watch]\x1b[0m error: {err}");
                }
                Err(_panic) => {
                    eprintln!("\x1b[1;31m[watch]\x1b[0m program panicked — waiting for changes…");
                }
            }

            // Wait for next .aivi file change
            let wait_since = SystemTime::now();
            loop {
                if ctrl_c.load(Ordering::Relaxed) {
                    eprintln!("\n\x1b[1;36m[watch]\x1b[0m interrupted — exiting.");
                    return Ok(());
                }
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(Ok(events)) if has_aivi_change(&events, wait_since) => {
                        eprintln!(
                            "\n\x1b[1;36m[watch]\x1b[0m file changed — restarting…\n\
                             ─────────────────────────────────────────────"
                        );
                        break;
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        eprintln!("\x1b[1;33m[watch]\x1b[0m watcher error: {e:?}");
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(AiviError::Io(std::io::Error::other(
                            "file watcher disconnected",
                        )));
                    }
                }
            }
        }
    }
}

/// Check whether any debounced event represents a real `.aivi` file modification.
///
/// The `notify` crate on Linux watches for `IN_OPEN` inotify events, so simply
/// reading a file during compilation produces debounced events indistinguishable
/// from writes. We verify each candidate path's mtime to filter out read-only
/// access events and avoid an infinite restart loop.
fn has_aivi_change(events: &[notify_debouncer_mini::DebouncedEvent], since: SystemTime) -> bool {
    events.iter().any(|ev| {
        ev.kind == DebouncedEventKind::Any
            && ev.path.extension().is_some_and(|ext| ext == "aivi")
            && was_modified_since(&ev.path, since)
    })
}

fn was_modified_since(path: &Path, since: SystemTime) -> bool {
    match path.metadata().and_then(|m| m.modified()) {
        Ok(mtime) => mtime > since,
        // File removed or inaccessible — treat as a real change.
        Err(_) => true,
    }
}

/// Install a SIGINT handler that sets an atomic flag instead of terminating.
fn install_ctrlc_handler(flag: Arc<AtomicBool>) {
    // Leak the Arc so the pointer is valid for the process lifetime.
    let ptr = Arc::into_raw(flag);
    CTRL_C_PTR.store(ptr as *mut AtomicBool, Ordering::Release);
    #[cfg(unix)]
    unsafe {
        use std::mem::MaybeUninit;
        let mut sa: MaybeUninit<libc::sigaction> = MaybeUninit::zeroed();
        let sa_ref = sa.assume_init_mut();
        sa_ref.sa_sigaction = sigint_handler as *const () as usize;
        sa_ref.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sa_ref.sa_mask);
        libc::sigaction(libc::SIGINT, sa.as_ptr(), std::ptr::null_mut());
    }
}

static CTRL_C_PTR: std::sync::atomic::AtomicPtr<AtomicBool> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(unix)]
extern "C" fn sigint_handler(_sig: i32) {
    let ptr = CTRL_C_PTR.load(Ordering::Acquire);
    if !ptr.is_null() {
        unsafe { &*ptr }.store(true, Ordering::Release);
    }
}
