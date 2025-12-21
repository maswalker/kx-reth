use jsonrpsee_types::error::{ErrorCode, ErrorObjectOwned};

/// Errors that can occur when interacting with the `kasplex_` namespace
#[derive(Debug, thiserror::Error)]
pub enum KasplexApiError {
    #[error("not found")]
    GethNotFound,
}

impl From<KasplexApiError> for ErrorObjectOwned {
    /// Converts the KasplexApiError into the jsonrpsee ErrorObject.
    fn from(error: KasplexApiError) -> Self {
        match error {
            KasplexApiError::GethNotFound => ErrorObjectOwned::owned(
                ErrorCode::ServerError(-32004).code(),
                "not found",
                None::<()>,
            ),
        }
    }
}
