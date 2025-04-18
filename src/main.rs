use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, StatusCode, Uri,
    },
    middleware::{from_fn, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Form, Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use mongodb::{bson::doc, Client, Collection, Database};

use jsonwebtoken::{
    decode, encode, get_current_timestamp, DecodingKey, EncodingKey, Header, Validation,
};
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Serialize, Deserialize)]
struct Identity {
    name: String,
    age: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Counter {
    value: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Auth {
    user_name: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
}

#[derive(Debug, Serialize)]
struct ResponseData<T> {
    status: u16,
    message: String,
    data: T,
}

impl<T: Serialize> IntoResponse for ResponseData<T> {
    fn into_response(self) -> Response {
        let Ok(response) = serde_json::to_string(&self) else {
            return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
        };
        Response::new(Body::from(response))
    }
}

#[tokio::main]
async fn main() {
    let client = db().await;
    let app = app(client);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Running on : {:?}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn db() -> Database {
    let uri = "mongodb://localhost:27017/";
    // Create a new client and connect to the server
    let client = Client::with_uri_str(uri).await.unwrap();
    let database = client.database("hello_axum");

    database
}

fn app(database: Database) -> Router {
    let cors_layer = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin("0.0.0.4000".parse::<HeaderValue>().unwrap());

    let shared_state = Arc::new(Mutex::new(Counter { value: 1 }));
    let user_router = Router::new().route("/profile", get(profile));
    let about_router = Router::new().route("/about", get(about));
    let another_nested_shared_router: Router<Arc<Mutex<Counter>>> =
        Router::new().route("/new", get(nested_shared_route));

    let auth_router: Router<(Arc<Mutex<Counter>>, Arc<Database>)> = Router::new()
        .route("/signup", post(signup))
        .route("/signin", post(signin))
        .route(
            "/protected",
            get(protected).route_layer(from_fn(login_required)),
        );

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
        .nest("/auth", auth_router)
        .with_state((Arc::clone(&shared_state), Arc::new(database)))
        .layer(cors_layer)
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

async fn signup(
    State((_, database)): State<(Arc<Mutex<Counter>>, Arc<Database>)>,
    Json(input): Json<Auth>,
) -> impl IntoResponse {
    let salt: SaltString = SaltString::generate(&mut OsRng);

    // Argon2 with default params (Argon2id v19)
    let argon2: Argon2<'_> = Argon2::default();

    // Hash password to PHC string ($argon2id$v=19$...)
    let password_hash = argon2
        .hash_password(input.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    let users_collection: Collection<Auth> = database.collection("users");
    let result = users_collection
        .insert_one(Auth {
            user_name: input.user_name,
            password: password_hash,
        })
        .await
        .unwrap();

    println!("Inserted a document with _id: {}", result.inserted_id);
    // (StatusCode::OK, "User signed up")
    ResponseData {
        status: StatusCode::OK.as_u16(),
        message: "User signed up".to_string(),
        data: result.inserted_id,
    }
}

async fn signin(
    State((_, database)): State<(Arc<Mutex<Counter>>, Arc<Database>)>,
    Json(input): Json<Auth>,
) -> impl IntoResponse {
    let users_collection: Collection<Auth> = database.collection("users");

    if let Some(result) = users_collection
        .find_one(doc! {
            "user_name": &input.user_name
        })
        .await
        .unwrap()
    {
        let parsed_hash = PasswordHash::new(&result.password).unwrap();
        if Argon2::default()
            .verify_password(input.password.as_bytes(), &parsed_hash)
            .is_ok()
        {
            let token = generate_token(&input.user_name);
            match token {
                Ok(token) => ResponseData {
                    status: StatusCode::OK.as_u16(),
                    message: "Signed in".to_string(),
                    data: token,
                },
                Err(e) => {
                    eprintln!("Error generating token : {}", e);
                    ResponseData {
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        message: "Error generating token".to_string(),
                        data: "".to_string(),
                    }
                }
            }
        } else {
            ResponseData {
                status: StatusCode::UNAUTHORIZED.as_u16(),
                message: "Invalid password".to_string(),
                data: "".to_string(),
            }
        }
    } else {
        ResponseData {
            status: StatusCode::NOT_FOUND.as_u16(),
            message: "User does not exist".to_string(),
            data: "".to_string(),
        }
    }
}

fn generate_token(username: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let key = b"secret";
    let my_claims = Claims {
        sub: username.to_string(),
        exp: get_current_timestamp() + Duration::new(60, 0).as_secs(),
    };
    encode(
        &Header::default(),
        &my_claims,
        &EncodingKey::from_secret(key),
    )
}

async fn protected(Extension(username): Extension<String>) -> impl IntoResponse {
    let response = format!("Hello {}", username);
    (StatusCode::OK, response)
}

async fn login_required(mut req: Request, next: Next) -> impl IntoResponse {
    let headers = req.headers().get("Authorization");

    match headers {
        Some(value) => {
            let token = value.to_str().unwrap();
            match decode::<Claims>(
                &token,
                &DecodingKey::from_secret(b"secret"),
                &Validation::default(),
            ) {
                Ok(token_data) => {
                    let claims = token_data.claims;
                    let username = claims.sub;
                    req.extensions_mut().insert(username);
                    next.run(req).await
                }
                Err(err) => (StatusCode::UNAUTHORIZED, err.to_string()).into_response(),
            }
        }
        None => (StatusCode::UNAUTHORIZED, "Missing auth token").into_response(),
    }
}
