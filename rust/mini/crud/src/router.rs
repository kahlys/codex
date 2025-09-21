use crate::{
    db, middlewares,
    model::{self, Monster},
};
use axum::{
    Router,
    extract::{Json, Path, State},
    http::StatusCode,
    middleware, response,
    routing::{get, put},
};
use serde_json::{Value, json};
use tower::ServiceBuilder;

pub fn new_app() -> Router {
    Router::new()
        .route("/", get(hello))
        .route("/monsters", get(list_monsters).post(create_monster))
        .route("/monsters/{id}", put(update_monster).delete(delete_monster))
        .with_state(db::new_db())
        .layer(ServiceBuilder::new().layer(middleware::from_fn(middlewares::print_request_info)))
}

pub async fn hello() -> &'static str {
    "Hello Monsters!"
}

pub async fn list_monsters(State(state): State<db::Db>) -> response::Json<Value> {
    let db = state.lock().await;
    let monsters: Vec<_> = db.values().collect();
    response::Json(json!(monsters))
}

pub async fn create_monster(State(state): State<db::Db>, Json(payload): Json<model::Monster>) {
    let mut db = state.lock().await;
    let new_id = (db.len() as u32) + 1;
    db.insert(
        new_id,
        Monster {
            id: payload.id,
            name: payload.name,
            age: payload.age,
            alive: payload.alive,
        },
    );
}

pub async fn update_monster(
    State(state): State<db::Db>,
    Path(id): Path<u32>,
    Json(payload): Json<model::Monster>,
) -> Result<(), StatusCode> {
    let mut db = state.lock().await;
    match db.get_mut(&id) {
        Some(monster) => {
            monster.name = payload.name;
            monster.age = payload.age;
            monster.alive = payload.alive;
            Ok(())
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn delete_monster(
    State(state): State<db::Db>,
    Path(id): Path<u32>,
) -> Result<(), StatusCode> {
    let mut db = state.lock().await;
    match db.remove(&id) {
        Some(_) => Ok(()),
        None => Err(StatusCode::NOT_FOUND),
    }
}
