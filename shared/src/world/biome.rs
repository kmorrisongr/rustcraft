use strum_macros::EnumString;

#[derive(Debug, Clone, Copy, EnumString, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Biome {
    PolarOcean,
    Canyon,
    Ocean,
    Plain,
    Mountain,
    Desert,
    Tundra,
    Savannah,
    Jungle,
}
