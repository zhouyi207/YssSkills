use std::{
    io,
    path::PathBuf,
    sync::mpsc::{self, Sender},
    thread,
};

use skill_registry::SkillsShClient;
use thiserror::Error;

use crate::application::{Application, ApplicationError};

type ApplicationJob = Box<dyn FnOnce(&mut Application) + Send + 'static>;

#[derive(Clone)]
pub struct ApplicationHandle {
    sender: Sender<ApplicationJob>,
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
        let (sender, receiver) = mpsc::channel::<ApplicationJob>();
        let (initialization_sender, initialization_receiver) = mpsc::sync_channel(1);

        thread::Builder::new()
            .name("yssskills-application".to_owned())
            .spawn(move || {
                let mut application = match Application::open(database_path, default_catalog_root) {
                    Ok(application) => {
                        // Startup waits synchronously for this result; a dropped receiver means
                        // the process is already abandoning initialization.
                        drop(initialization_sender.send(Ok(())));
                        application
                    }
                    Err(error) => {
                        drop(initialization_sender.send(Err(error)));
                        return;
                    }
                };

                while let Ok(job) = receiver.recv() {
                    job(&mut application);
                }
            })
            .map_err(ApplicationWorkerError::Start)?;

        initialization_receiver
            .recv()
            .map_err(|_| ApplicationWorkerError::Unavailable)?
            .map_err(ApplicationWorkerError::Initialization)?;
        Ok(Self { sender })
    }

    pub fn execute<T, F>(&self, operation: F) -> Result<T, ApplicationWorkerError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Application) -> Result<T, ApplicationError> + Send + 'static,
    {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.sender
            .send(Box::new(move |application| {
                let result = operation(application).map_err(ApplicationWorkerError::Operation);
                // A Tauri command can be cancelled while blocking work finishes. Dropping the
                // completed result is safe because application state was already finalized.
                drop(response_sender.send(result));
            }))
            .map_err(|_| ApplicationWorkerError::Unavailable)?;
        response_receiver
            .recv()
            .map_err(|_| ApplicationWorkerError::ResponseDropped)?
    }
}
