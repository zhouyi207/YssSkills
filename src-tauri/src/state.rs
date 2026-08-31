use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use skill_index::{IndexError, SkillIndex};
use skill_local::{WatchManager, WatchTarget, WatchTargetKind};
use skill_registry::SkillsShClient;
use thiserror::Error;

use yss_api::{Application, ApplicationError, CatalogIndexWorkerConfig};

type ApplicationJob = Box<dyn FnOnce(&mut Application) + Send + 'static>;

enum ApplicationMessage {
    Execute(ApplicationJob),
    Shutdown,
}

enum IndexWorkerMessage {
    Reconfigure(CatalogIndexWorkerConfig),
    Shutdown,
}

#[derive(Clone)]
pub struct ApplicationHandle {
    runtime: Arc<ApplicationRuntime>,
}

struct ApplicationRuntime {
    application_sender: Sender<ApplicationMessage>,
    index_sender: Sender<IndexWorkerMessage>,
    cancellation: Arc<AtomicBool>,
    application_thread: Mutex<Option<JoinHandle<()>>>,
    index_thread: Mutex<Option<JoinHandle<()>>>,
}

pub struct AppState {
    pub application: ApplicationHandle,
    pub registry: SkillsShClient,
}

#[derive(Debug, Error)]
pub enum ApplicationWorkerError {
    #[error("failed to start the application worker")]
    Start(#[source] io::Error),
    #[error("failed to initialize application state")]
    Initialization(#[source] ApplicationError),
    #[error("application operation failed")]
    Operation(#[source] ApplicationError),
    #[error("the application worker is unavailable")]
    Unavailable,
    #[error("the application worker stopped before returning a response")]
    ResponseDropped,
}

impl ApplicationHandle {
    pub fn start(
        database_path: PathBuf,
        default_catalog_root: PathBuf,
    ) -> Result<Self, ApplicationWorkerError> {
        let (application_sender, application_receiver) = mpsc::channel::<ApplicationMessage>();
        let (initialization_sender, initialization_receiver) = mpsc::sync_channel(1);

        let application_thread = thread::Builder::new()
            .name("yssskills-application".to_owned())
            .spawn(move || {
                let mut application = match Application::open(database_path, default_catalog_root) {
                    Ok(application) => {
                        let index_config = application.catalog_index_worker_config();
                        // Startup waits synchronously for this result; a dropped receiver means
                        // the process is already abandoning initialization.
                        drop(initialization_sender.send(Ok(index_config)));
                        application
                    }
                    Err(error) => {
                        drop(initialization_sender.send(Err(error)));
                        return;
                    }
                };

                while let Ok(message) = application_receiver.recv() {
                    match message {
                        ApplicationMessage::Execute(job) => job(&mut application),
                        ApplicationMessage::Shutdown => break,
                    }
                }
            })
            .map_err(ApplicationWorkerError::Start)?;

        let initialization = match initialization_receiver.recv() {
            Ok(initialization) => initialization,
            Err(_) => {
                join_startup_application_thread(application_thread);
                return Err(ApplicationWorkerError::Unavailable);
            }
        };
        let index_config = match initialization {
            Ok(index_config) => index_config,
            Err(error) => {
                join_startup_application_thread(application_thread);
                return Err(ApplicationWorkerError::Initialization(error));
            }
        };
        let (index_sender, index_receiver) = mpsc::channel();
        let cancellation = Arc::new(AtomicBool::new(false));
        let index_cancellation = Arc::clone(&cancellation);
        let index_thread = match thread::Builder::new()
            .name("yssskills-skill-index".to_owned())
            .spawn(move || run_index_worker(index_config, index_receiver, index_cancellation))
        {
            Ok(thread) => thread,
            Err(source) => {
                drop(application_sender.send(ApplicationMessage::Shutdown));
                if application_thread.join().is_err() {
                    eprintln!("the application worker panicked while startup was being cancelled");
                }
                return Err(ApplicationWorkerError::Start(source));
            }
        };
        Ok(Self {
            runtime: Arc::new(ApplicationRuntime {
                application_sender,
                index_sender,
                cancellation,
                application_thread: Mutex::new(Some(application_thread)),
                index_thread: Mutex::new(Some(index_thread)),
            }),
        })
    }

    pub fn execute<T, F>(&self, operation: F) -> Result<T, ApplicationWorkerError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Application) -> Result<T, ApplicationError> + Send + 'static,
    {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        let index_sender = self.runtime.index_sender.clone();
        self.runtime
            .application_sender
            .send(ApplicationMessage::Execute(Box::new(move |application| {
                let previous_index_config = application.catalog_index_worker_config();
                let result = operation(application).map_err(ApplicationWorkerError::Operation);
                let next_index_config = application.catalog_index_worker_config();
                if previous_index_config != next_index_config
                    && index_sender
                        .send(IndexWorkerMessage::Reconfigure(next_index_config))
                        .is_err()
                {
                    eprintln!(
                        "the Skill index watcher stopped before accepting a catalog reconfiguration"
                    );
                }
                // A Tauri command can be cancelled while blocking work finishes. Dropping the
                // completed result is safe because application state was already finalized.
                drop(response_sender.send(result));
            })))
            .map_err(|_| ApplicationWorkerError::Unavailable)?;
        response_receiver
            .recv()
            .map_err(|_| ApplicationWorkerError::ResponseDropped)?
    }
}

impl Drop for ApplicationRuntime {
    fn drop(&mut self) {
        self.cancellation.store(true, Ordering::Release);
        if self
            .index_sender
            .send(IndexWorkerMessage::Shutdown)
            .is_err()
        {
            eprintln!("the Skill index worker had already stopped during shutdown");
        }
        if self
            .application_sender
            .send(ApplicationMessage::Shutdown)
            .is_err()
        {
            eprintln!("the application worker had already stopped during shutdown");
        }

        if let Some(thread) = take_thread(&self.index_thread) {
            if thread.join().is_err() {
                eprintln!("the Skill index worker panicked during shutdown");
            }
        }
        if let Some(thread) = take_thread(&self.application_thread) {
            if thread.join().is_err() {
                eprintln!("the application worker panicked during shutdown");
            }
        }
    }
}

fn take_thread(slot: &Mutex<Option<JoinHandle<()>>>) -> Option<JoinHandle<()>> {
    match slot.lock() {
        Ok(mut slot) => slot.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    }
}

fn run_index_worker(
    mut config: CatalogIndexWorkerConfig,
    receiver: mpsc::Receiver<IndexWorkerMessage>,
    cancellation: Arc<AtomicBool>,
) {
    let (mut index, status) = match SkillIndex::open(&config.database_path) {
        Ok(opened) => opened,
        Err(error) => {
            report_index_worker_error("open", &error);
            return;
        }
    };
    if let Some(backup) = status.recovered_from {
        eprintln!(
            "the background worker recovered the derived Skill index from {}",
            backup.display()
        );
    }
    if status.needs_rebuild {
        if let Err(error) = index.rebuild(&config.skills_root, &cancellation) {
            report_index_worker_error("initial rebuild", &error);
        }
    }

    let mut watcher = create_index_watcher(&config);
    reconcile_in_background(&mut index, &config, &cancellation);
    mark_unwatched_index_stale(&index, watcher.is_none());

    loop {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(IndexWorkerMessage::Reconfigure(next)) => {
                if next.database_path != config.database_path {
                    match SkillIndex::open(&next.database_path) {
                        Ok((next_index, status)) => {
                            index = next_index;
                            if let Some(backup) = status.recovered_from {
                                eprintln!(
                                    "the reconfigured Skill index was recovered from {}",
                                    backup.display()
                                );
                            }
                            if status.needs_rebuild {
                                if let Err(error) = index.rebuild(&next.skills_root, &cancellation)
                                {
                                    report_index_worker_error("reconfigured rebuild", &error);
                                }
                            }
                        }
                        Err(error) => {
                            report_index_worker_error("reconfigure", &error);
                            continue;
                        }
                    }
                }
                config = next;
                replace_index_watcher(&mut watcher, &config);
                reconcile_in_background(&mut index, &config, &cancellation);
                mark_unwatched_index_stale(&index, watcher.is_none());
            }
            Ok(IndexWorkerMessage::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                if drain_watcher_changes(watcher.as_ref()) {
                    reconcile_in_background(&mut index, &config, &cancellation);
                }
            }
        }
        if cancellation.load(Ordering::Acquire) {
            break;
        }
    }

    if let Some(mut watcher) = watcher {
        if let Err(error) = watcher.shutdown() {
            eprintln!("failed to shut down the Skill index watcher: {error}");
        }
    }
}

fn create_index_watcher(config: &CatalogIndexWorkerConfig) -> Option<WatchManager> {
    let mut watcher = match WatchManager::new(Duration::from_millis(350)) {
        Ok(watcher) => watcher,
        Err(error) => {
            eprintln!("failed to create the Skill index watcher: {error}");
            return None;
        }
    };
    let target = WatchTarget {
        id: "central-skill-index".to_owned(),
        path: config.skills_root.clone(),
        kind: WatchTargetKind::Skills,
    };
    if let Err(error) = watcher.replace_targets([target]) {
        eprintln!("failed to watch the central Skills directory: {error}");
        if let Err(shutdown_error) = watcher.shutdown() {
            eprintln!("failed to stop the unusable Skill index watcher: {shutdown_error}");
        }
        return None;
    }
    Some(watcher)
}

fn replace_index_watcher(watcher: &mut Option<WatchManager>, config: &CatalogIndexWorkerConfig) {
    if let Some(mut previous) = watcher.take() {
        if let Err(error) = previous.shutdown() {
            eprintln!("failed to stop the previous Skill index watcher: {error}");
        }
    }
    *watcher = create_index_watcher(config);
}

fn drain_watcher_changes(watcher: Option<&WatchManager>) -> bool {
    let Some(watcher) = watcher else {
        return false;
    };
    let mut changed = false;
    loop {
        match watcher.try_recv() {
            Ok(Some(_)) => changed = true,
            Ok(None) => return changed,
            Err(error) => {
                eprintln!("the Skill index watcher reported an error: {error}");
                return true;
            }
        }
    }
}

fn reconcile_in_background(
    index: &mut SkillIndex,
    config: &CatalogIndexWorkerConfig,
    cancellation: &AtomicBool,
) {
    match index.reconcile(&config.skills_root, cancellation) {
        Ok(_) | Err(IndexError::Cancelled) => {}
        Err(error) => report_index_worker_error("reconcile", &error),
    }
}

fn report_index_worker_error(operation: &str, error: &IndexError) {
    eprintln!("background Skill index {operation} failed: {error}");
}

fn mark_unwatched_index_stale(index: &SkillIndex, watcher_unavailable: bool) {
    if watcher_unavailable {
        if let Err(error) = index.mark_stale() {
            report_index_worker_error("mark unwatched snapshot stale", &error);
        }
    }
}

fn join_startup_application_thread(thread: JoinHandle<()>) {
    if thread.join().is_err() {
        eprintln!("the application worker panicked during startup failure handling");
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, time::Instant};

    use tempfile::tempdir;

    use super::*;

    fn write_skill(path: &Path, name: &str, body: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill\n---\n{body}\n"),
        )
        .unwrap();
    }

    fn wait_for_catalog(
        handle: &ApplicationHandle,
        predicate: impl Fn(&yss_api::CatalogSkillList) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let view = handle
                .execute(|application| application.list_catalog_skills_view())
                .unwrap();
            if predicate(&view) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the derived Skill index did not converge before the test deadline"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn startup_reconcile_and_watcher_recover_changes_missed_while_offline() {
        let root = tempdir().unwrap();
        let database = root.path().join("app/yssskills.sqlite3");
        let catalog_root = root.path().join("catalog");
        let alpha = catalog_root.join("skills/alpha");
        write_skill(&alpha, "Alpha", "first body");

        let first = ApplicationHandle::start(database.clone(), catalog_root.clone()).unwrap();
        wait_for_catalog(&first, |view| {
            view.skills
                .iter()
                .any(|skill| skill.snapshot.installed.metadata.name() == "Alpha")
        });
        drop(first);

        write_skill(&alpha, "Alpha Updated", "changed while the app was closed");
        let second = ApplicationHandle::start(database, catalog_root.clone()).unwrap();
        wait_for_catalog(&second, |view| {
            view.skills
                .iter()
                .any(|skill| skill.snapshot.installed.metadata.name() == "Alpha Updated")
        });

        write_skill(
            &catalog_root.join("skills/beta"),
            "Beta",
            "created while watcher is active",
        );
        wait_for_catalog(&second, |view| view.skills.len() == 2);
        drop(second);
    }
}
