use kurrentdb::Endpoint;
use neon::{object::Object, prelude::Context, prelude::JsError, result::JsResult};
use eyre::Report;

#[derive(Debug)]
pub enum ErrorKind {
    UnavailableError,
    StreamNotFoundError,
    StreamDeletedError,
    AccessDeniedError,
    ParseError(String),
    NotLeaderError(Endpoint),
    UnknownError(String),
}

impl From<kurrentdb::Error> for ErrorKind {
    fn from(err: kurrentdb::Error) -> Self {
        match err {
            kurrentdb::Error::GrpcConnectionError(_) => ErrorKind::UnavailableError,
            kurrentdb::Error::ResourceNotFound => ErrorKind::StreamNotFoundError,
            kurrentdb::Error::ResourceDeleted => ErrorKind::StreamDeletedError,
            kurrentdb::Error::AccessDenied => ErrorKind::AccessDeniedError,
            kurrentdb::Error::NotLeaderException(endpoint) => ErrorKind::NotLeaderError(endpoint),
            _ => ErrorKind::UnknownError(err.to_string()),
        }
    }
}

impl From<kurrentdb::ClientSettingsParseError> for ErrorKind {
    fn from(err: kurrentdb::ClientSettingsParseError) -> Self {
        ErrorKind::ParseError(err.message().to_string())
    }
}

impl From<Report> for ErrorKind {
    fn from(err: Report) -> Self {
        if let Some(kdb_err) = err.downcast_ref::<kurrentdb::Error>() {
            return ErrorKind::from(kdb_err.clone());
        }

        if let Some(parse_err) = err.downcast_ref::<kurrentdb::ClientSettingsParseError>() {
            return ErrorKind::from(parse_err.clone());
        }

        ErrorKind::UnknownError(err.to_string())
    }
}

pub fn create_js_error<'a, C, E>(cx: &mut C, error: E) -> JsResult<'a, JsError>
where
    C: Context<'a>,
    E: Into<ErrorKind> + std::fmt::Display,
{
    let kind = ErrorKind::from(error.into());

    let (type_name, error_message) = match &kind {
        ErrorKind::UnavailableError => ("UnavailableError", format!("{:?}", kind)),
        ErrorKind::StreamNotFoundError => ("StreamNotFoundError", format!("{:?}", kind)),
        ErrorKind::StreamDeletedError => ("StreamDeletedError", format!("{:?}", kind)),
        ErrorKind::ParseError(msg) => ("ParseError", msg.clone()),
        ErrorKind::AccessDeniedError => ("AccessDeniedError", format!("{:?}", kind)),
        ErrorKind::NotLeaderError(_) => ("NotLeaderError", format!("{:?}", kind)),
        ErrorKind::UnknownError(msg) => ("UnknownError", msg.clone()),
    };

    let error = JsError::error(cx, &error_message)?;
    let name = cx.string(type_name);
    error.set(cx, "name", name)?;

    let metadata = cx.empty_object();

    match &kind {
        ErrorKind::NotLeaderError(endpoint) => {
            let host = cx.string(endpoint.host.to_string());
            let port = cx.number(endpoint.port);

            metadata.set(cx, "leader-endpoint-host", host)?;
            metadata.set(cx, "leader-endpoint-port", port)?;
        }
        ErrorKind::UnknownError(msg) => {
            let detail = cx.string(msg);
            metadata.set(cx, "detail", detail)?;
        }
        _ => {}
    }

    error.set(cx, "metadata", metadata)?;

    Ok(error)
}
