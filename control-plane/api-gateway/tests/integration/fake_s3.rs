use axum::{
    body::{self, Body},
    extract::{Request, State},
    http::{header::ETAG, Method, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Default)]
struct Store {
    objects: HashMap<String, Vec<u8>>,
    uploads: HashMap<String, BTreeMap<u32, Vec<u8>>>,
    parts: usize,
}

#[derive(Clone, Default)]
pub struct FakeS3 {
    store: Arc<Mutex<Store>>,
}

fn query(request: &Request) -> HashMap<String, String> {
    request
        .uri()
        .query()
        .unwrap_or_default()
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) => (name.to_string(), value.to_string()),
            None => (pair.to_string(), String::new()),
        })
        .collect()
}

fn reply(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn handle(State(fake): State<FakeS3>, request: Request) -> Response {
    let path = request.uri().path().to_string();
    let query = query(&request);
    let method = request.method().clone();
    let bytes = body::to_bytes(request.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    let mut store = fake.store.lock().unwrap();
    match (method, query.get("uploadId"), query.get("partNumber")) {
        (Method::POST, None, _) if query.contains_key("uploads") => {
            let id = Uuid::new_v4().to_string();
            store.uploads.insert(id.clone(), BTreeMap::new());
            let xml = format!(
                "<InitiateMultipartUploadResult><UploadId>{id}</UploadId></InitiateMultipartUploadResult>"
            );
            reply(StatusCode::OK, &xml)
        }
        (Method::POST, Some(id), _) => {
            let Some(parts) = store.uploads.remove(id) else {
                return reply(StatusCode::NOT_FOUND, "NoSuchUpload");
            };
            let assembled = parts.into_values().flatten().collect();
            store.objects.insert(path, assembled);
            reply(StatusCode::OK, "<CompleteMultipartUploadResult/>")
        }
        (Method::PUT, Some(id), Some(number)) => {
            let number: u32 = number.parse().unwrap();
            let Some(parts) = store.uploads.get_mut(id) else {
                return reply(StatusCode::NOT_FOUND, "NoSuchUpload");
            };
            parts.insert(number, bytes);
            store.parts += 1;
            Response::builder()
                .status(StatusCode::OK)
                .header(ETAG, format!("\"part-{number}\""))
                .body(Body::empty())
                .unwrap()
        }
        (Method::PUT, _, _) => {
            store.objects.insert(path, bytes);
            reply(StatusCode::OK, "")
        }
        (Method::GET, _, _) => match store.objects.get(&path) {
            Some(object) => Response::new(Body::from(object.clone())),
            None => reply(
                StatusCode::NOT_FOUND,
                "<Error><Code>NoSuchKey</Code></Error>",
            ),
        },
        (Method::DELETE, Some(id), _) => {
            store.uploads.remove(id);
            reply(StatusCode::NO_CONTENT, "")
        }
        (Method::DELETE, None, _) => {
            store.objects.remove(&path);
            reply(StatusCode::NO_CONTENT, "")
        }
        _ => reply(StatusCode::BAD_REQUEST, "unsupported"),
    }
}

impl FakeS3 {
    pub async fn start() -> (String, FakeS3) {
        let fake = FakeS3::default();
        let router = Router::new()
            .route("/{*path}", any(handle))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        (format!("http://{address}"), fake)
    }

    pub fn keys(&self) -> Vec<String> {
        self.store.lock().unwrap().objects.keys().cloned().collect()
    }

    pub fn object(&self, key: &str) -> Option<Vec<u8>> {
        self.store.lock().unwrap().objects.get(key).cloned()
    }

    pub fn uploaded_parts(&self) -> usize {
        self.store.lock().unwrap().parts
    }
}
