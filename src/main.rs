use std::collections::HashMap;

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};

#[tokio::main]
async fn main() {
    let app = app();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Running on : {:?}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    Router::new()
        .route("/", get(hello_world))
        .route("/{id}", get(call_with_id))
        .route("/id", get(call_with_query_params))
}

async fn hello_world() -> &'static str {
    "Hello World!"
}

async fn call_with_id(Path(id): Path<u32>) -> impl IntoResponse {
    println!("Id : {id}");
    (StatusCode::OK, format!("Hello from {id}")).into_response()
}

async fn call_with_query_params(Query(params): Query<HashMap<String, String>>) -> &'static str {
    for (name, age) in &params {
        println!("The name is {} and the age is {}", name, age);
    }

    "Hello"
}
