//! Unified wave configuration shared between CPU physics and GPU rendering.
//!
//! This ensures physics calculations exactly match visual wave motion.

use bevy::math::Vec2;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Maximum number of waves supported (matches GPU shader array size).
pub const MAX_WAVES: usize = 4;

/// Configuration for a single Gerstner wave.
/// This struct is designed to be directly uploadable to GPU uniform buffers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C)]
pub struct WaveParams {
    /// Wave direction (normalized 2D vector)
    pub direction: Vec2,
    /// Wave steepness (0.0 = sine wave, 1.0 = sharp crest)
    pub steepness: f32,
    /// Wavelength in world units
    pub wavelength: f32,
    /// Wave speed multiplier
    pub speed: f32,
    /// Padding for GPU alignment (std140 layout)
    pub _padding: f32,
}

impl WaveParams {
    pub fn new(direction: Vec2, steepness: f32, wavelength: f32, speed: f32) -> Self {
        Self {
            direction: direction.normalize_or_zero(),
            steepness: steepness.clamp(0.0, 1.0),
            wavelength: wavelength.max(0.1),
            speed,
            _padding: 0.0,
        }
    }

    /// Calculate wave number (k = 2π / wavelength)
    #[inline(always)]
    pub fn wave_number(&self) -> f32 {
        2.0 * PI / self.wavelength
    }

    /// Calculate angular frequency (ω = k * speed)
    #[inline(always)]
    pub fn frequency(&self) -> f32 {
        self.wave_number() * self.speed
    }
}

impl Default for WaveParams {
    fn default() -> Self {
        Self::new(Vec2::X, 0.3, 4.0, 1.0)
    }
}

/// Complete wave system configuration.
/// Can be serialized for save files and network sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveConfig {
    /// Array of wave parameters (fixed size for GPU compatibility)
    pub waves: [WaveParams; MAX_WAVES],
    /// Number of active waves (0-4)
    pub num_waves: u32,
    /// Base water level (Y coordinate)
    pub base_level: f32,
    /// Global amplitude multiplier
    pub amplitude_scale: f32,
}

impl WaveConfig {
    /// Create an empty wave config with no active waves.
    pub fn new(base_level: f32) -> Self {
        Self {
            waves: [WaveParams::default(); MAX_WAVES],
            num_waves: 0,
            base_level,
            amplitude_scale: 1.0,
        }
    }

    /// Add a wave to the configuration. Returns false if max waves reached.
    pub fn add_wave(&mut self, params: WaveParams) -> bool {
        if (self.num_waves as usize) < MAX_WAVES {
            self.waves[self.num_waves as usize] = params;
            self.num_waves += 1;
            true
        } else {
            false
        }
    }

    /// Get iterator over active waves only.
    pub fn active_waves(&self) -> impl Iterator<Item = &WaveParams> {
        self.waves.iter().take(self.num_waves as usize)
    }
}

/// Preset wave configurations for different water types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum WavePreset {
    /// Calm water with minimal waves
    Calm,
    /// Lake with gentle waves
    Lake,
    /// Standard ocean waves
    #[default]
    Ocean,
    /// Stormy ocean with large waves
    Storm,
    /// Completely still water (no waves)
    Still,
}

impl WavePreset {
    /// Create a WaveConfig from this preset.
    pub fn to_config(self, base_level: f32) -> WaveConfig {
        let mut config = WaveConfig::new(base_level);

        match self {
            WavePreset::Still => {
                // No waves
            }
            WavePreset::Calm => {
                config.amplitude_scale = 0.3;
                config.add_wave(WaveParams::new(Vec2::new(1.0, 0.2), 0.2, 6.0, 0.5));
            }
            WavePreset::Lake => {
                config.amplitude_scale = 0.5;
                config.add_wave(WaveParams::new(Vec2::new(1.0, 0.0), 0.3, 4.0, 0.8));
                config.add_wave(WaveParams::new(Vec2::new(0.3, 1.0), 0.2, 2.5, 1.0));
            }
            WavePreset::Ocean => {
                config.amplitude_scale = 1.0;
                config.add_wave(WaveParams::new(Vec2::new(1.0, 0.3), 0.6, 8.0, 1.5));
                config.add_wave(WaveParams::new(Vec2::new(-0.7, 1.0), 0.5, 5.0, 1.8));
                config.add_wave(WaveParams::new(Vec2::new(0.5, -1.0), 0.4, 3.0, 2.2));
                config.add_wave(WaveParams::new(Vec2::new(-1.0, -0.5), 0.3, 1.5, 2.8));
            }
            WavePreset::Storm => {
                config.amplitude_scale = 2.0;
                config.add_wave(WaveParams::new(Vec2::new(1.0, 0.2), 0.8, 12.0, 2.0));
                config.add_wave(WaveParams::new(Vec2::new(-0.5, 1.0), 0.7, 8.0, 2.5));
                config.add_wave(WaveParams::new(Vec2::new(0.7, -0.7), 0.6, 5.0, 3.0));
                config.add_wave(WaveParams::new(Vec2::new(-1.0, -0.3), 0.5, 3.0, 3.5));
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_params_alignment() {
        // Ensure struct is GPU-compatible size (multiple of 16 bytes for std140)
        assert_eq!(std::mem::size_of::<WaveParams>(), 24);
    }

    #[test]
    fn test_preset_wave_counts() {
        assert_eq!(WavePreset::Still.to_config(0.0).num_waves, 0);
        assert_eq!(WavePreset::Calm.to_config(0.0).num_waves, 1);
        assert_eq!(WavePreset::Lake.to_config(0.0).num_waves, 2);
        assert_eq!(WavePreset::Ocean.to_config(0.0).num_waves, 4);
        assert_eq!(WavePreset::Storm.to_config(0.0).num_waves, 4);
    }
}
