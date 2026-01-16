use serde::Deserialize;
use strum_macros::EnumString;


#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize)]
#[derive(EnumString)]
pub enum Tree {
    Oak,
    Spruce,
    Sequoia,
    Palm,
    Birch,
    Chestnut,
    Cypress,
    Ironwood,
    Baobab,
    Cactus,
    Acacia,
    Bamboo
}
