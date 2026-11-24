use std::time::Duration;

use advisor_core::HealthCheck;
use advisor_embed::{EmbedError, Embedder, HttpEmbedder, HttpEmbedderConfig};
use serde_json::json;
use url::Url;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn embedder(server: &MockServer, dimensions: usize) -> HttpEmbedder {
    HttpEmbedder::new(HttpEmbedderConfig {
        url: Url::parse(&format!("{}/embed", server.uri())).unwrap(),
        timeout: Duration::from_secs(5),
        dimensions,
    })
    .unwrap()
}

#[tokio::test]
async fn sends_texts_and_returns_vectors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/embed"))
        .and(body_json(json!({ "text": ["apple earnings"] })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "embedding": [[0.1, 0.2, 0.3]] })),
        )
        .mount(&server)
        .await;

    let vectors = embedder(&server, 3)
        .embed(&["apple earnings".to_string()])
        .await
        .unwrap();
    assert_eq!(vectors, vec![vec![0.1, 0.2, 0.3]]);
}

#[tokio::test]
async fn wrong_vector_size_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "embedding": [[0.1, 0.2]] })),
        )
        .mount(&server)
        .await;

    let err = embedder(&server, 384)
        .embed(&["x".to_string()])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            EmbedError::Shape {
                dimensions: 384,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[tokio::test]
async fn http_error_is_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503).set_body_string("loading model"))
        .mount(&server)
        .await;

    let err = embedder(&server, 3)
        .embed(&["x".to_string()])
        .await
        .unwrap_err();
    assert!(
        matches!(err, EmbedError::Api { status: 503, .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn health_check_reports_dimensions() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "embedding": [[0.0, 0.0]] })),
        )
        .mount(&server)
        .await;

    assert_eq!(embedder(&server, 2).check().await.unwrap(), "2 dimensions");
}
