use thiserror::Error;
use tracing::error;
use crate::Node;
use std::path::Path;
use tokio::{fs::{self, File}, io::AsyncReadExt};
use http::{Response, StatusCode, Request};

pub async fn handle_custom_uri(node: &Node, req: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, HandleCustomUriError> {
    let path = req.uri().path().strip_prefix("/").unwrap_or(req.uri().path()).split('/').collect::<Vec<_>>();
    match path.first().copied() {
        Some("thumbnail") => {
            let file_cas_id = path.get(1).ok_or_else(|| HandleCustomUriError::BadRequest("Invalid number of parameters!"))?;
            let filename = Path::new(&node.config.data_directory())
                .join("thumbnails")
                .join(file_cas_id)
                .with_extension("webp");

            let mut file = File::open(&filename).await.map_err(|_| HandleCustomUriError::NotFound)?;

            let mut buf = match fs::metadata(&filename).await {
                Ok(metadata) => Vec::with_capacity(metadata.len() as usize),
                Err(_) => Vec::new(),
            };

            file.read_to_end(&mut buf).await.unwrap();
            Ok(Response::builder()
                .header("Content-Type", "image/webp")
                .status(StatusCode::OK)
                .body(buf)?)
        }
        _ => Err(HandleCustomUriError::BadRequest("Invalid operation!")),
    }
}

#[derive(Error, Debug)]
pub enum HandleCustomUriError {
    #[error("error creating http request/response: {0}")]
    Http(#[from] http::Error),
    #[error("{0}")]
    BadRequest(&'static str),
    #[error("resource not found")]
    NotFound
}

impl HandleCustomUriError {
    pub fn into_response(self) -> http::Result<Response<Vec<u8>>> {
        match self {
            HandleCustomUriError::Http(err) => {
                error!("Error creating http request/response: {}", err);
                Response::builder()
                    .header("Content-Type", "text/plain")
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(b"Internal Server Error".to_vec())
            },
            HandleCustomUriError::BadRequest(msg) => {
                error!("Bad request: {}", msg);
                Response::builder()
                    .header("Content-Type", "text/plain")
                    .status(StatusCode::BAD_REQUEST)
                    .body(msg.as_bytes().to_vec())
            }
            HandleCustomUriError::NotFound => 
                Response::builder()
                    .header("Content-Type", "text/plain")
                    .status(StatusCode::NOT_FOUND)
                    .body(b"Resource not found".to_vec()),
        }
    }
}