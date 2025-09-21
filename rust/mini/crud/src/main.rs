mod db;
mod middlewares;
mod model;
mod router;

#[tokio::main]
async fn main() {
    println!("Server running on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, router::new_app()).await.unwrap();
}
