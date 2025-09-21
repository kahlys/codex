use crate::model;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type Db = Arc<Mutex<HashMap<u32, model::Monster>>>;

pub fn new_db() -> Db {
    let mut data = HashMap::new();
    data.insert(
        1,
        model::Monster {
            id: 1,
            name: "Dragon".to_string(),
            age: 500,
            alive: true,
        },
    );
    data.insert(
        2,
        model::Monster {
            id: 2,
            name: "Zombie".to_string(),
            age: 200,
            alive: false,
        },
    );
    Arc::new(Mutex::new(data))
}
