use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::{StatusCode, Uri},
    middleware::{self, from_fn, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post, put},
    Extension, Form, Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string_pretty, Value};

#[derive(Debug, Serialize, Deserialize)]
struct Identity {
    name: String,
    age: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Counter {
    value: u32,
}

#[tokio::main]
async fn main() {
    let app = app();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Running on : {:?}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    let shared_state = Arc::new(Mutex::new(Counter { value: 1 }));
    let user_router = Router::new().route("/profile", get(profile));
    let about_router = Router::new().route("/about", get(about));
    let another_nested_shared_router: Router<Arc<Mutex<Counter>>> =
        Router::new().route("/new", get(nested_shared_route));

    Router::new()
        .route("/", get(hello_world))
        .nest("/user", user_router)
        .merge(about_router)
        .route(
            "/hello",
            get(hello).route_layer(from_fn(middleware_to_request)),
        )
        .route("/wildcard/{*rest}", get(wildcard_route))
        .route(
            "/{id}",
            get(call_with_id).route_layer(from_fn(call_with_id_middleware)),
        )
        .route("/id", get(call_with_query_params))
        .route("/identity", post(parse_json))
        .route("/headers", post(parse_headers))
        .route("/status-code", post(returns_with_status_code))
        .with_state(Arc::clone(&shared_state))
        .route(
            "/counter",
            post(increase_counter)
                .get(get_counter)
                .put(put_counter)
                .delete(delete_counter),
        )
        .with_state(Arc::clone(&shared_state))
        .fallback(not_found)
        .layer(from_fn(global_middleware))
        .route("/redirect-to-hello", get(redirect))
        .route("/a/big/uri", get(get_uri))
        .route("/submit-form", post(submit_form))
        .nest("/nested", another_nested_shared_router)
        .with_state(Arc::clone(&shared_state))
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

async fn returns_with_status_code() -> impl IntoResponse {
    (StatusCode::OK, "Okay!")
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

async fn get_counter(State(counter): State<Arc<Mutex<Counter>>>) -> impl IntoResponse {
    let count = counter;
    (StatusCode::OK, format!("The count is : {:?}", count)).into_response()
}

async fn put_counter(
    State(counter): State<Arc<Mutex<Counter>>>,
    Json(c): Json<Counter>,
) -> impl IntoResponse {
    let put_value = c.value;
    let mut counter = counter.lock().unwrap();
    counter.value = put_value;

    let json_data = to_string_pretty(&*counter).unwrap();

    Response::new(Body::new(json_data))
}

async fn delete_counter(State(counter): State<Arc<Mutex<Counter>>>) -> impl IntoResponse {
    let mut counter = counter.lock().unwrap();
    counter.value = 0;

    (StatusCode::OK, "The counter has been deleted.").into_response()
}

async fn increase_counter(State(counter): State<Arc<Mutex<Counter>>>) -> impl IntoResponse {
    let mut counter = counter.lock().unwrap();
    counter.value += 1;

    (StatusCode::OK, "The count has been increased.").into_response()
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 | Not Found").into_response()
}

async fn global_middleware(request: Request, next: Next) -> impl IntoResponse {
    println!("Hello from global middleware");
    let response = next.run(request).await;
    response
}

async fn call_with_id_middleware(request: Request, next: Next) -> impl IntoResponse {
    let req_body = request.uri().path().trim_matches('/');
    let result = req_body.parse::<u32>();
    if result.is_ok() {
        next.run(request).await
    } else {
        (StatusCode::OK, "Wrong input").into_response()
    }
}

async fn hello(Extension(identity): Extension<Arc<Identity>>) -> &'static str {
    println!("Identity : {:?}", identity);
    "Hello"
}

async fn middleware_to_request(mut request: Request, next: Next) -> impl IntoResponse {
    let identity = Identity {
        name: String::from("John Doe"),
        age: 29,
    };

    request.extensions_mut().insert(Arc::new(identity));
    next.run(request).await
}

async fn redirect() -> impl IntoResponse {
    Redirect::to("/hello")
}

async fn profile() -> impl IntoResponse {
    (StatusCode::OK, "Profile")
}

async fn about() -> impl IntoResponse {
    (StatusCode::OK, "About")
}

async fn wildcard_route(Path(wildcard): Path<String>) -> impl IntoResponse {
    println!("Wildcard route : {}", wildcard);

    (StatusCode::OK, wildcard)
}

async fn get_uri(uri: Uri) -> impl IntoResponse {
    println!("The uri is : {:#?}", uri);
    (StatusCode::OK, uri.to_string())
}

async fn submit_form(Form(identity): Form<Identity>) -> impl IntoResponse {
    println!("The form is : {:#?}", identity);
    StatusCode::OK
}

async fn nested_shared_route(State(state): State<Arc<Mutex<Counter>>>) -> impl IntoResponse {
    println!("The shared state is : {:?}", state);
    (StatusCode::OK, "Okay")
}
