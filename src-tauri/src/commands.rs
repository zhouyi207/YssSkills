pub(crate) mod dashboard;
pub(crate) mod registry;
pub(crate) mod settings;
pub(crate) mod skills;
pub(crate) mod workspaces;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::ipc::IpcError;
use yss_api::{Application, ApplicationError, ApplicationHandle};

fn parse_request<T>(request: Option<Value>) -> Result<T, IpcError>
where
    T: DeserializeOwned,
{
    let request =
        request.ok_or_else(|| IpcError::invalid_request_payload("request payload is required"))?;
    serde_json::from_value(request).map_err(IpcError::invalid_request_payload)
}

async fn run_application<T, F>(handle: ApplicationHandle, operation: F) -> Result<T, IpcError>
where
    T: Send + 'static,
    F: FnOnce(&mut Application) -> Result<T, ApplicationError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || handle.execute(operation))
        .await
        .map_err(IpcError::blocking_task_failed)?
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TestRequest {
        value: String,
    }

    #[test]
    fn malformed_request_uses_structured_ipc_error() {
        let error = parse_request::<TestRequest>(Some(serde_json::json!({
            "value": "ok",
            "unexpected": true
        })))
        .unwrap_err();

        assert_eq!(error.code, "request.invalid");
        assert!(!error.retryable);
        assert!(error
            .context
            .get("reason")
            .is_some_and(|reason| reason.contains("unknown field")));
    }

    #[test]
    fn missing_request_uses_structured_ipc_error() {
        let error = parse_request::<TestRequest>(None).unwrap_err();

        assert_eq!(error.code, "request.invalid");
        assert_eq!(
            error.context.get("reason").map(String::as_str),
            Some("request payload is required")
        );
    }

    #[test]
    fn valid_request_is_deserialized() {
        let request =
            parse_request::<TestRequest>(Some(serde_json::json!({ "value": "ok" }))).unwrap();

        assert_eq!(request.value, "ok");
    }
}
