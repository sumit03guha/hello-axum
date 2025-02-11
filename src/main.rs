use std::collections::HashMap;

use axum::{
    body::Body,
    extract::{Path, Query, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string_pretty, Value};

#[derive(Debug, Serialize, Deserialize)]
struct Identity {
    name: String,
    age: u32,
}

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
        .route("/identity", post(parse_json))
        .route("/headers", post(parse_headers))
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

async fn parse_json(Json(identity): Json<Identity>) -> Response {
    println!(
        "The name is {} and the age is {}",
        identity.name, identity.age
    );
    // Json(json!({
    //    "name": identity.name,
    //    "age":identity.age
    // }))

    let json_data = to_string_pretty(&identity).unwrap();

    Response::new(Body::new(json_data))
}

async fn parse_headers(req: Request) -> impl IntoResponse {
    let headers = req.headers();
    let method = req.method();
    let uri = req.uri();
    let version = req.version();

    println!(
        "The header details are : {:#?}, {:#?}, {:#?}, {:#?}",
        headers, method, uri, version
    );
}
