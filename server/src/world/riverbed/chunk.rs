use itertools::Itertools;
use packed_uints::PackedUints;
use shared::world::{CHUNK_S1, CHUNK_S1I, CHUNKP_S1, CHUNKP_S2, CHUNKP_S3, ChunkedPos, ColedPos, blocks::blocks::BlockId, face::Face};

use crate::world::riverbed::utils::Palette;


#[derive(Debug)]
pub struct Chunk {
    pub data: PackedUints,
    pub palette: Palette<BlockId>,
}

pub fn linearize(x: usize, y: usize, z: usize) -> usize {
    z + x * CHUNKP_S1 + y * CHUNKP_S2
}

pub fn pad_linearize(x: usize, y: usize, z: usize) -> usize {
    z + 1 + (x+1) * CHUNKP_S1 + (y+1) * CHUNKP_S2
}

impl Chunk {
    pub fn get(&self, (x, y, z): ChunkedPos) -> &BlockId {
        &self.palette[self.data.get(pad_linearize(x, y, z))]
    }

    pub fn set(&mut self, (x, y, z): ChunkedPos, block: BlockId) {
        let idx = pad_linearize(x, y, z);
        self.data.set(idx, self.palette.index(block));
    }

    pub fn set_unpadded(&mut self, (x, y, z): ChunkedPos, block: BlockId) {
        let idx = linearize(x, y, z);
        self.data.set(idx, self.palette.index(block));
    }

    pub fn set_yrange(&mut self, (x, top, z): ChunkedPos, height: usize, block: BlockId) {
        let value = self.palette.index(block);
        // Note: we do end+1 because set_range(_step) is not inclusive
        self.data.set_range_step(
            pad_linearize(x, top - height, z), 
            pad_linearize(x, top, z)+1, 
            CHUNKP_S2,
            value
        );
    }

    pub fn top(&self, (x, z): ColedPos) -> (&BlockId, usize) {
        for y in (0..CHUNK_S1).rev() {
            let b_idx = self.data.get(pad_linearize(x, y, z));
            if b_idx > 0 {
                return (&self.palette[b_idx], y);
            }
        }
        (&self.palette[0], 0)
    }

    pub fn set_if_empty(&mut self, (x, y, z): ChunkedPos, block: BlockId) -> bool {
        pad_linearize(x, y, z);
        false
    }

    pub fn copy_side_from(&mut self, other: &Chunk, face: Face) {
        let row_step = match face {
            Face::Left | Face::Right => 1,
            Face::Down | Face::Up => 1,
            Face::Back | Face::Front => CHUNKP_S1,
        };
        let col_step = match face {
            Face::Left | Face::Right => CHUNKP_S2 - CHUNK_S1,
            Face::Down | Face::Up => CHUNKP_S1 - CHUNK_S1,
            Face::Back | Face::Front => CHUNKP_S1 + CHUNKP_S1,
        };
        let [nx, ny, nz] = face.n();
        let mut self_i = linearize(
            ((nx * CHUNK_S1I).max(1) + nx) as usize,
            ((ny * CHUNK_S1I).max(1) + ny) as usize, 
            ((nz * CHUNK_S1I).max(1) + nz) as usize,
        );
        let [nx, ny, nz] = face.opposite().n();
        let mut other_i= linearize(
            (nx * CHUNK_S1I).max(1) as usize,
            (ny * CHUNK_S1I).max(1) as usize, 
            (nz * CHUNK_S1I).max(1) as usize,
        );
        let translation = other.palette.map_to(&self.palette);
        for _ in 0..CHUNK_S1 {
            for _ in 0..CHUNK_S1 {
                let other_value = other.data.get(other_i);
                let value = if let Some(val) = translation[other_value] {
                    val
                } else {
                    self.palette.index(other.palette[other_value].clone())
                };
                self.data.set(self_i, value);
                self_i += row_step;
                other_i += row_step;
            }
            self_i += col_step;
            other_i += col_step;
        }
    }
}

impl From<&[BlockId]> for Chunk {
    fn from(values: &[BlockId]) -> Self {
        let mut palette = Palette::new();
        let values = values.iter().map(|v| palette.index(v.clone())).collect_vec();
        let data = PackedUints::from(values.as_slice());
        Chunk {data, palette}
    }
}

impl Chunk {
    pub fn new() -> Self {
        let palette = Palette::new();
        Chunk {
            data: PackedUints::new(CHUNKP_S3),
            palette: palette, 
        }
    }
}

#[cfg(test)]
mod tests {
    use shared::world::{CHUNK_S1, CHUNK_S1I, CHUNKP_S1, CHUNKP_S2, face::Face};

    use crate::world::riverbed::linearize;


    fn plane(face: Face)  -> [usize; 3] {
        match face {
            Face::Left => [0, 1, 1],
            Face::Down => [1, 0, 1],
            Face::Back => [1, 1, 0], 
            Face::Right => [0, 1, 1],
            Face::Up => [1, 0, 1],
            Face::Front => [1, 1, 0],
        }
    }

    fn chunk_face_indices_safe(face: Face) -> Vec<usize>{
        let [nx, ny, nz] = face.n();
        let x = ((nx * CHUNK_S1I).max(1) + nx) as usize;
        let y = ((ny * CHUNK_S1I).max(1) + ny) as usize;
        let z = ((nz * CHUNK_S1I).max(1) + nz) as usize;
        let [tx, ty, tz] = plane(face);
        let mut res = vec![];
        for dy in 0..(CHUNK_S1*ty).max(1) {
            for dx in 0..(CHUNK_S1*tx).max(1) {
                for dz in 0..(CHUNK_S1*tz).max(1) {
                    let idx = super::linearize(x + dx, y + dy, z + dz);
                    res.push(idx);
                }
            }
        }
        res
    }

    fn chunk_face_indices_fast(face: Face) -> Vec<usize> {
        let row_step = match face {
            Face::Left | Face::Right => 1,
            Face::Down | Face::Up => 1,
            Face::Back | Face::Front => CHUNKP_S1,
        };
        let col_step = match face {
            Face::Left | Face::Right => CHUNKP_S2 - CHUNK_S1,
            Face::Down | Face::Up => CHUNKP_S1 - CHUNK_S1,
            Face::Back | Face::Front => CHUNKP_S1 + CHUNKP_S1,
        };
        let [nx, ny, nz] = face.n();
        let mut self_i = linearize(
            ((nx * CHUNK_S1I).max(1) + nx) as usize,
            ((ny * CHUNK_S1I).max(1) + ny) as usize, 
            ((nz * CHUNK_S1I).max(1) + nz) as usize,
        );
        let mut res = vec![];
        for _ in 0..CHUNK_S1 {
            for _ in 0..CHUNK_S1 {
                res.push(self_i);
                self_i += row_step;
            }
            self_i += col_step;
        }
        res
    }

    fn assert_chunk_face_indices(face: Face) {
        let safe = chunk_face_indices_safe(face);
        let fast = chunk_face_indices_fast(face);
        assert_eq!(safe, fast);
    }

    #[test]
    fn test_face_idx() {
        assert_chunk_face_indices(Face::Left);
        assert_chunk_face_indices(Face::Down);
        assert_chunk_face_indices(Face::Back);
        assert_chunk_face_indices(Face::Right);
        assert_chunk_face_indices(Face::Up);
        assert_chunk_face_indices(Face::Front);
    }
}