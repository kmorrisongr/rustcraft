use crate::messages::PlayerId;
use crate::world::utils::Palette;
use crate::world::blocks::blocks::{BlockId};
use packed_uints::PackedUints;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet};
use std::fmt::Debug;

/// Represents a type of flora that can be requested for generation in the chunk above.
#[derive(Clone, Debug)]
pub struct SerdablePackedUints(pub PackedUints);

impl Default for SerdablePackedUints {
    fn default() -> Self {
        Self(PackedUints::new(0))
    }
}

impl Serialize for SerdablePackedUints {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as u32 values (portable, sufficient for block IDs)
        // Format: [length as u32, then all values as u32]
        let length = self.0.length as u32;
        let values: Vec<u32> = self.0.unpack_u32();
        
        // Build the byte buffer: 4 bytes for length + 4 bytes per value
        let mut bytes: Vec<u8> = Vec::with_capacity(4 + values.len() * 4);
        bytes.extend_from_slice(&length.to_le_bytes());
        for val in values.iter().take(self.0.length) {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> Deserialize<'de> for SerdablePackedUints {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PackedUintsVisitor;

        impl<'de> serde::de::Visitor<'de> for PackedUintsVisitor {
            type Value = SerdablePackedUints;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a byte array representing PackedUints")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() < 4 {
                    return Err(E::custom("PackedUints data too short"));
                }
                
                // Read length (first 4 bytes)
                let length = u32::from_le_bytes(v[0..4].try_into().unwrap()) as usize;
                let data_bytes = &v[4..];
                
                if data_bytes.len() != length * 4 {
                    return Err(E::custom(format!(
                        "expected {} bytes for {} values, got {}",
                        length * 4,
                        length,
                        data_bytes.len()
                    )));
                }
                
                let values: Vec<usize> = data_bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()) as usize)
                    .collect();
                
                Ok(SerdablePackedUints(PackedUints::from(values.as_slice())))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                // Handle formats that deserialize bytes as a sequence
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    bytes.push(byte);
                }
                self.visit_bytes(&bytes)
            }
        }

        deserializer.deserialize_bytes(PackedUintsVisitor)
    }
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ServerChunk {
    pub data: SerdablePackedUints,
    pub palette: Palette<BlockId>,
    /// Timestamp marking the last update this chunk has received
    pub ts: u64,
    pub sent_to_clients: HashSet<PlayerId>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sent_to_clients_deduplicates_players() {
        let mut chunk = ServerChunk::default();
        chunk.sent_to_clients.insert(1);
        chunk.sent_to_clients.insert(1);

        assert_eq!(chunk.sent_to_clients.len(), 1);
    }
}