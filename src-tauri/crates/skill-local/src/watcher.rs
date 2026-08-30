use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, RwLock,
    },
    time::Duration,
};

use notify::RecursiveMode;
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache,
};

use super::{ensure_directory, map_io_error, LocalError, WatchFailure};

type LocalDebouncer = Debouncer<notify::RecommendedWatcher, RecommendedCache>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchTargetKind {
    Skills,
    Config,
    Discovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchTarget {
    pub id: String,
    pub path: PathBuf,
    pub kind: WatchTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchChange {
    pub target_id: String,
    pub kind: WatchTargetKind,
    pub paths: Vec<PathBuf>,
}

#[derive(Clone)]
struct RegisteredTarget {
    target: WatchTarget,
    compare_path: PathBuf,
    watch_path: PathBuf,
    mode: RecursiveMode,
}

struct WatchMessage {
    generation: u64,
    result: Result<WatchChange, LocalError>,
}

pub struct WatchManager {
    debouncer: Option<LocalDebouncer>,
    debounce: Duration,
    registrations: Arc<RwLock<Vec<RegisteredTarget>>>,
    generation: Arc<AtomicU64>,
    generation_allocator: AtomicU64,
    sender: mpsc::Sender<WatchMessage>,
    receiver: mpsc::Receiver<WatchMessage>,
    closed: bool,
}

impl WatchManager {
    pub fn new(debounce: Duration) -> Result<Self, LocalError> {
        if debounce.is_zero() {
            return Err(LocalError::InvalidDebounce);
        }

        let registrations = Arc::new(RwLock::new(Vec::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = mpsc::channel();
        let debouncer = create_debouncer(debounce, Arc::clone(&registrations), 0, sender.clone())?;

        Ok(Self {
            debouncer: Some(debouncer),
            debounce,
            registrations,
            generation,
            generation_allocator: AtomicU64::new(0),
            sender,
            receiver,
            closed: false,
        })
    }

    pub fn replace_targets(
        &mut self,
        targets: impl IntoIterator<Item = WatchTarget>,
    ) -> Result<(), LocalError> {
        if self.closed {
            return Err(LocalError::WatcherClosed);
        }

        let next_generation = allocate_generation(&self.generation_allocator);
        let prepared_targets = prepare_targets(targets)?;
        let physical_watches = collect_physical_watches(&prepared_targets);
        let new_registrations = Arc::new(RwLock::new(prepared_targets));

        let mut new_debouncer = create_debouncer(
            self.debounce,
            Arc::clone(&new_registrations),
            next_generation,
            self.sender.clone(),
        )?;
        for (watch_path, mode) in physical_watches {
            let error_path = watch_path.clone();
            if let Err(source) = new_debouncer.watch(&watch_path, mode) {
                new_debouncer.stop();
                return Err(LocalError::Watch {
                    operation: "watch",
                    path: error_path,
                    source: WatchFailure::from_notify(source),
                });
            }
        }

        let old_registrations = std::mem::replace(&mut self.registrations, new_registrations);
        let old_debouncer = self.debouncer.replace(new_debouncer);
        self.generation.store(next_generation, Ordering::Release);
        if let Some(old_debouncer) = old_debouncer {
            old_debouncer.stop();
        }
        drop(old_registrations);

        Ok(())
    }

    pub fn try_recv(&self) -> Result<Option<WatchChange>, LocalError> {
        if self.closed {
            return Err(LocalError::WatcherClosed);
        }

        try_recv_current_generation(&self.receiver, &self.generation)
    }

    pub fn shutdown(&mut self) -> Result<(), LocalError> {
        if self.closed {
            return Ok(());
        }

        self.closed = true;
        let state_result = self
            .registrations
            .write()
            .map(|mut registrations| registrations.clear())
            .map_err(|_| LocalError::WatcherStatePoisoned);

        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop();
        }

        state_result
    }
}

fn create_debouncer(
    debounce: Duration,
    registrations: Arc<RwLock<Vec<RegisteredTarget>>>,
    generation: u64,
    sender: mpsc::Sender<WatchMessage>,
) -> Result<LocalDebouncer, LocalError> {
    let callback = move |result: DebounceEventResult| {
        handle_debounce_result(result, &registrations, &sender, generation);
    };

    new_debouncer(debounce, None, callback).map_err(|source| LocalError::Watch {
        operation: "create",
        path: PathBuf::new(),
        source: WatchFailure::from_notify(source),
    })
}

fn handle_debounce_result(
    result: DebounceEventResult,
    registrations: &Arc<RwLock<Vec<RegisteredTarget>>>,
    sender: &mpsc::Sender<WatchMessage>,
    generation: u64,
) {
    match result {
        Ok(events) => handle_debounced_events(events, registrations, sender, generation),
        Err(errors) => {
            send_callback_result(
                sender,
                generation,
                Err(LocalError::WatcherCallback {
                    errors: errors.into_iter().map(WatchFailure::from_notify).collect(),
                }),
            );
        }
    }
}

fn handle_debounced_events(
    events: Vec<DebouncedEvent>,
    registrations: &Arc<RwLock<Vec<RegisteredTarget>>>,
    sender: &mpsc::Sender<WatchMessage>,
    generation: u64,
) {
    let registrations = match registrations.read() {
        Ok(registrations) => registrations.clone(),
        Err(_) => {
            send_callback_result(sender, generation, Err(LocalError::WatcherStatePoisoned));
            return;
        }
    };

    let needs_rescan = events.iter().any(|event| event.need_rescan());
    let mut grouped = BTreeMap::<String, (WatchTargetKind, PathBuf, BTreeSet<PathBuf>)>::new();

    for event in events {
        for event_path in &event.paths {
            for registered in &registrations {
                if target_matches(registered, event_path) {
                    let Some(normalized_path) = normalize_event_path(registered, event_path) else {
                        continue;
                    };
                    grouped
                        .entry(registered.target.id.clone())
                        .or_insert_with(|| {
                            (
                                registered.target.kind,
                                registered.target.path.clone(),
                                BTreeSet::new(),
                            )
                        })
                        .2
                        .insert(normalized_path);
                }
            }
        }
    }

    if needs_rescan && grouped.is_empty() {
        for registered in registrations {
            grouped
                .entry(registered.target.id.clone())
                .or_insert_with(|| {
                    (
                        registered.target.kind,
                        registered.compare_path.clone(),
                        BTreeSet::new(),
                    )
                })
                .2
                .insert(registered.compare_path);
        }
    }

    for (target_id, (kind, compare_path, paths)) in grouped {
        let change = WatchChange {
            target_id,
            kind,
            paths: normalize_paths(paths, &compare_path),
        };
        if !send_callback_result(sender, generation, Ok(change)) {
            return;
        }
    }
}

fn target_matches(registered: &RegisteredTarget, event_path: &Path) -> bool {
    match registered.target.kind {
        WatchTargetKind::Skills => event_path.starts_with(&registered.compare_path),
        WatchTargetKind::Config => event_path == registered.compare_path,
        WatchTargetKind::Discovery => {
            event_path.parent() == Some(registered.compare_path.as_path())
        }
    }
}

fn normalize_event_path(registered: &RegisteredTarget, event_path: &Path) -> Option<PathBuf> {
    match registered.target.kind {
        WatchTargetKind::Config => {
            (event_path == registered.compare_path).then(|| registered.target.path.clone())
        }
        WatchTargetKind::Skills | WatchTargetKind::Discovery => event_path
            .strip_prefix(&registered.compare_path)
            .ok()
            .map(|relative| registered.target.path.join(relative)),
    }
}

fn normalize_paths(paths: impl IntoIterator<Item = PathBuf>, compare_path: &Path) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| path.starts_with(compare_path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn send_callback_result(
    sender: &mpsc::Sender<WatchMessage>,
    generation: u64,
    result: Result<WatchChange, LocalError>,
) -> bool {
    sender.send(WatchMessage { generation, result }).is_ok()
}

fn allocate_generation(allocator: &AtomicU64) -> u64 {
    allocator.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
}

fn try_recv_current_generation(
    receiver: &mpsc::Receiver<WatchMessage>,
    generation: &AtomicU64,
) -> Result<Option<WatchChange>, LocalError> {
    loop {
        match receiver.try_recv() {
            Ok(message) => {
                if message.generation != generation.load(Ordering::Acquire) {
                    continue;
                }
                return match message.result {
                    Ok(change) => Ok(Some(change)),
                    Err(error) => Err(error),
                };
            }
            Err(mpsc::TryRecvError::Empty) => return Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => return Err(LocalError::WatcherClosed),
        }
    }
}

fn prepare_targets(
    targets: impl IntoIterator<Item = WatchTarget>,
) -> Result<Vec<RegisteredTarget>, LocalError> {
    let targets: Vec<_> = targets.into_iter().collect();
    let mut ids = HashSet::with_capacity(targets.len());

    for target in &targets {
        if target.id.trim().is_empty() {
            return Err(LocalError::InvalidWatchTarget {
                id: target.id.clone(),
                path: target.path.clone(),
                reason: "id must not be empty",
            });
        }
        if !ids.insert(target.id.clone()) {
            return Err(LocalError::DuplicateWatchTarget {
                id: target.id.clone(),
            });
        }
    }

    targets.into_iter().map(prepare_target).collect()
}

fn prepare_target(target: WatchTarget) -> Result<RegisteredTarget, LocalError> {
    match target.kind {
        WatchTargetKind::Skills => prepare_directory_target(target, RecursiveMode::Recursive),
        WatchTargetKind::Discovery => prepare_directory_target(target, RecursiveMode::NonRecursive),
        WatchTargetKind::Config => prepare_config_target(target),
    }
}

fn prepare_directory_target(
    target: WatchTarget,
    mode: RecursiveMode,
) -> Result<RegisteredTarget, LocalError> {
    ensure_directory(&target.path)?;
    let canonical =
        fs::canonicalize(&target.path).map_err(|source| map_io_error(&target.path, source))?;

    Ok(RegisteredTarget {
        target,
        compare_path: canonical.clone(),
        watch_path: canonical,
        mode,
    })
}

fn prepare_config_target(target: WatchTarget) -> Result<RegisteredTarget, LocalError> {
    match fs::metadata(&target.path) {
        Ok(metadata) if metadata.is_dir() => {
            return Err(LocalError::InvalidWatchTarget {
                id: target.id.clone(),
                path: target.path.clone(),
                reason: "config target must not be a directory",
            });
        }
        Ok(_) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => return Err(map_io_error(&target.path, source)),
    }

    let file_name = target.path.file_name().map(PathBuf::from).ok_or_else(|| {
        LocalError::InvalidWatchTarget {
            id: target.id.clone(),
            path: target.path.clone(),
            reason: "config target must name a file",
        }
    })?;
    let parent = target
        .path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure_directory(parent)?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|source| map_io_error(parent, source))?;

    Ok(RegisteredTarget {
        target,
        compare_path: canonical_parent.join(file_name),
        watch_path: canonical_parent,
        mode: RecursiveMode::NonRecursive,
    })
}

fn collect_physical_watches(registrations: &[RegisteredTarget]) -> Vec<(PathBuf, RecursiveMode)> {
    let mut watches = Vec::new();

    for registration in registrations {
        if let Some((_, mode)) = watches
            .iter_mut()
            .find(|(path, _)| path == &registration.watch_path)
        {
            if matches!(registration.mode, RecursiveMode::Recursive) {
                *mode = RecursiveMode::Recursive;
            }
            continue;
        }

        watches.push((registration.watch_path.clone(), registration.mode));
    }

    watches
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        sync::{atomic::AtomicU64, mpsc, Arc},
        thread,
        time::Instant,
    };

    use notify::{
        event::{EventAttributes, Flag},
        Event, EventKind,
    };
    use notify_debouncer_full::DebouncedEvent;
    use tempfile::tempdir;

    use super::*;

    fn debounced_event(paths: Vec<PathBuf>, rescan: bool) -> DebouncedEvent {
        let mut event = Event {
            kind: EventKind::Any,
            paths,
            attrs: EventAttributes::default(),
        };
        if rescan {
            event.attrs.set_flag(Flag::Rescan);
        }
        DebouncedEvent::new(event, Instant::now())
    }

    #[test]
    fn normalized_paths_are_unique_sorted_and_component_bounded() {
        let root = PathBuf::from("C:/home/.codex/skills");
        let child = root.join("alpha/SKILL.md");
        let sibling = PathBuf::from("C:/home/.codex/skills-other/file");

        let paths = normalize_paths([child.clone(), sibling, child.clone()], &root);

        assert_eq!(paths, vec![child]);
    }

    #[test]
    fn callback_batches_matching_paths_per_target() {
        let root = PathBuf::from("C:/home/.codex/skills");
        let first = root.join("alpha");
        let second = root.join("zeta");
        let sibling = PathBuf::from("C:/home/.codex/skills-other/file");
        let registrations = Arc::new(RwLock::new(vec![
            RegisteredTarget {
                target: WatchTarget {
                    id: "skills".to_owned(),
                    path: root.clone(),
                    kind: WatchTargetKind::Skills,
                },
                compare_path: root.clone(),
                watch_path: root.clone(),
                mode: RecursiveMode::Recursive,
            },
            RegisteredTarget {
                target: WatchTarget {
                    id: "discovery".to_owned(),
                    path: root.clone(),
                    kind: WatchTargetKind::Discovery,
                },
                compare_path: root.clone(),
                watch_path: root.clone(),
                mode: RecursiveMode::NonRecursive,
            },
        ]));
        let active_generation = AtomicU64::new(1);
        let (sender, receiver) = mpsc::channel();

        handle_debounced_events(
            vec![debounced_event(
                vec![second.clone(), sibling, first.clone(), second.clone()],
                false,
            )],
            &registrations,
            &sender,
            1,
        );

        let mut changes = vec![
            try_recv_current_generation(&receiver, &active_generation)
                .unwrap()
                .unwrap(),
            try_recv_current_generation(&receiver, &active_generation)
                .unwrap()
                .unwrap(),
        ];
        changes.sort_by(|left, right| left.target_id.cmp(&right.target_id));
        assert_eq!(
            changes,
            vec![
                WatchChange {
                    target_id: "discovery".to_owned(),
                    kind: WatchTargetKind::Discovery,
                    paths: vec![first.clone(), second.clone()],
                },
                WatchChange {
                    target_id: "skills".to_owned(),
                    kind: WatchTargetKind::Skills,
                    paths: vec![first, second],
                },
            ]
        );
        assert!(matches!(
            try_recv_current_generation(&receiver, &active_generation),
            Ok(None)
        ));
    }

    #[test]
    fn rescan_without_paths_notifies_only_registered_targets() {
        let skills_path = PathBuf::from("C:/home/.codex/skills");
        let config_path = PathBuf::from("C:/home/.codex/config.toml");
        let registrations = Arc::new(RwLock::new(vec![
            RegisteredTarget {
                target: WatchTarget {
                    id: "skills".to_owned(),
                    path: skills_path.clone(),
                    kind: WatchTargetKind::Skills,
                },
                compare_path: skills_path.clone(),
                watch_path: skills_path.clone(),
                mode: RecursiveMode::Recursive,
            },
            RegisteredTarget {
                target: WatchTarget {
                    id: "config".to_owned(),
                    path: config_path.clone(),
                    kind: WatchTargetKind::Config,
                },
                compare_path: config_path.clone(),
                watch_path: PathBuf::from("C:/home/.codex"),
                mode: RecursiveMode::NonRecursive,
            },
        ]));
        let active_generation = AtomicU64::new(1);
        let (sender, receiver) = mpsc::channel();

        handle_debounced_events(
            vec![debounced_event(Vec::new(), true)],
            &registrations,
            &sender,
            1,
        );

        let mut changes = vec![
            try_recv_current_generation(&receiver, &active_generation)
                .unwrap()
                .unwrap(),
            try_recv_current_generation(&receiver, &active_generation)
                .unwrap()
                .unwrap(),
        ];
        changes.sort_by(|left, right| left.target_id.cmp(&right.target_id));
        assert_eq!(
            changes,
            vec![
                WatchChange {
                    target_id: "config".to_owned(),
                    kind: WatchTargetKind::Config,
                    paths: vec![config_path],
                },
                WatchChange {
                    target_id: "skills".to_owned(),
                    kind: WatchTargetKind::Skills,
                    paths: vec![skills_path],
                },
            ]
        );
        assert!(matches!(
            try_recv_current_generation(&receiver, &active_generation),
            Ok(None)
        ));
    }

    #[test]
    fn in_flight_old_generation_is_filtered_before_delivery() {
        let active_generation = AtomicU64::new(1);
        let (sender, receiver) = mpsc::channel();
        let old_change = WatchChange {
            target_id: "old".to_owned(),
            kind: WatchTargetKind::Skills,
            paths: vec![PathBuf::from("C:/old")],
        };
        let new_change = WatchChange {
            target_id: "new".to_owned(),
            kind: WatchTargetKind::Discovery,
            paths: vec![PathBuf::from("C:/new")],
        };

        assert!(send_callback_result(&sender, 0, Ok(old_change)));
        assert!(send_callback_result(
            &sender,
            0,
            Err(LocalError::WatcherCallback {
                errors: vec![WatchFailure::from_notify(notify::Error::generic(
                    "old callback",
                ))],
            })
        ));
        assert!(send_callback_result(&sender, 1, Ok(new_change.clone())));

        let received = try_recv_current_generation(&receiver, &active_generation)
            .unwrap()
            .unwrap();
        assert_eq!(received, new_change);
        assert!(matches!(
            try_recv_current_generation(&receiver, &active_generation),
            Ok(None)
        ));
    }

    #[test]
    fn failed_generation_candidate_is_not_reused_by_the_next_replacement() {
        let allocator = AtomicU64::new(0);
        let failed_generation = allocate_generation(&allocator);
        let next_generation = allocate_generation(&allocator);
        let active_generation = AtomicU64::new(next_generation);
        let (sender, receiver) = mpsc::channel();

        assert_ne!(failed_generation, next_generation);
        assert!(send_callback_result(
            &sender,
            failed_generation,
            Ok(WatchChange {
                target_id: "failed-candidate".to_owned(),
                kind: WatchTargetKind::Skills,
                paths: vec![PathBuf::from("C:/failed-candidate")],
            })
        ));
        assert!(matches!(
            try_recv_current_generation(&receiver, &active_generation),
            Ok(None)
        ));
    }

    #[test]
    fn callback_errors_are_delivered_through_the_change_channel() {
        let registrations = Arc::new(RwLock::new(Vec::new()));
        let active_generation = AtomicU64::new(1);
        let (sender, receiver) = mpsc::channel();

        handle_debounce_result(
            Err(vec![notify::Error::generic("callback failed")]),
            &registrations,
            &sender,
            1,
        );

        match try_recv_current_generation(&receiver, &active_generation) {
            Err(error) => {
                match &error {
                    LocalError::WatcherCallback { errors } => assert_eq!(errors.len(), 1),
                    other => panic!("unexpected callback result: {other:?}"),
                }
                assert!(error.source().is_some());
            }
            result => panic!("unexpected callback result: {result:?}"),
        }
    }

    #[test]
    fn replacement_does_not_depend_on_the_old_registration_lock() {
        let root = tempdir().unwrap();
        let existing = root.path().join("existing");
        fs::create_dir_all(&existing).unwrap();
        let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();
        manager
            .replace_targets([WatchTarget {
                id: "initial".to_owned(),
                path: existing.clone(),
                kind: WatchTargetKind::Skills,
            }])
            .unwrap();

        let registrations = Arc::clone(&manager.registrations);
        let poison = thread::spawn(move || {
            let _guard = registrations.write().unwrap();
            panic!("poison registration state for the replacement test");
        });
        assert!(poison.join().is_err());

        manager
            .replace_targets([WatchTarget {
                id: "replacement".to_owned(),
                path: existing,
                kind: WatchTargetKind::Skills,
            }])
            .unwrap();
        manager.shutdown().unwrap();
    }
}
