use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Monster {
    #[serde(skip_deserializing)]
    pub id: u32,

    pub name: String,
    pub age: u32,
    pub alive: bool,
}
