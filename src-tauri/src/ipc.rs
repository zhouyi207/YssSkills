pub use yss_api::*;

use crate::state::ApplicationWorkerError;

pub(crate) fn map_application_worker_error(error: ApplicationWorkerError) -> IpcError {
    match error {
        ApplicationWorkerError::Initialization(error)
        | ApplicationWorkerError::Operation(error) => error.into(),
        ApplicationWorkerError::Start(source) => IpcError::application_worker_start_failed(source),
        ApplicationWorkerError::Unavailable | ApplicationWorkerError::ResponseDropped => {
            IpcError::application_worker_unavailable()
        }
    }
}
