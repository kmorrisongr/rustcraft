//! Gerstner wave physics for water surfaces.
//!
//! This module provides calculations for Gerstner waves, which create realistic
//! ocean wave motion. Gerstner waves move particles in circular patterns,
//! creating the characteristic sharp crests and gentle troughs of real water.

use bevy::math::{Vec2, Vec3};
use std::f32::consts::PI;

/// Parameters for a single Gerstner wave.
#[derive(Debug, Clone, Copy)]
pub struct GerstnerWave {
    /// Wave direction (normalized 2D vector)
    pub direction: Vec2,
    /// Wave steepness (0.0 = sine wave, 1.0 = sharp crest)
    /// Recommended: 0.0 - 0.8 for realistic waves
    pub steepness: f32,
    /// Wavelength in world units
    pub wavelength: f32,
    /// Wave speed multiplier
    pub speed: f32,
}

impl GerstnerWave {
    /// Create a new Gerstner wave with the given parameters.
    pub fn new(direction: Vec2, steepness: f32, wavelength: f32, speed: f32) -> Self {
        Self {
            direction: direction.normalize_or_zero(),
            steepness: steepness.clamp(0.0, 1.0),
            wavelength: wavelength.max(0.1),
            speed,
        }
    }

    /// Calculate wave number (k = 2π / wavelength)
    #[inline]
    fn wave_number(&self) -> f32 {
        2.0 * PI / self.wavelength
    }

    /// Calculate angular frequency based on wave number and speed
    #[inline]
    fn frequency(&self) -> f32 {
        self.wave_number() * self.speed
    }

    /// Calculate the displacement of a point due to this wave.
    ///
    /// Returns (horizontal_displacement, vertical_displacement)
    pub fn calculate_displacement(&self, position: Vec2, time: f32) -> (Vec2, f32) {
        let k = self.wave_number();
        let omega = self.frequency();
        let phase = k * self.direction.dot(position) - omega * time;
        
        let cos_phase = phase.cos();
        let sin_phase = phase.sin();
        
        // Amplitude calculation (affects steepness)
        let amplitude = self.steepness / k;
        
        // Horizontal displacement (creates the circular motion)
        let horizontal = self.direction * amplitude * sin_phase;
        
        // Vertical displacement (wave height)
        let vertical = amplitude * cos_phase;
        
        (horizontal, vertical)
    }

    /// Calculate the normal vector at a point due to this wave.
    pub fn calculate_normal(&self, position: Vec2, time: f32) -> Vec3 {
        let k = self.wave_number();
        let omega = self.frequency();
        let phase = k * self.direction.dot(position) - omega * time;
        
        let cos_phase = phase.cos();
        let sin_phase = phase.sin();
        
        let wa = self.steepness;
        
        // Normal calculation for Gerstner waves
        let normal_x = -self.direction.x * wa * cos_phase;
        let normal_y = 1.0 - wa * sin_phase;
        let normal_z = -self.direction.y * wa * cos_phase;
        
        Vec3::new(normal_x, normal_y, normal_z).normalize_or(Vec3::Y)
    }
}

/// Collection of Gerstner waves that combine to create complex water motion.
#[derive(Debug, Clone)]
pub struct GerstnerWaveSystem {
    /// List of waves
    pub waves: Vec<GerstnerWave>,
    /// Base water level (y coordinate)
    pub base_level: f32,
}

impl Default for GerstnerWaveSystem {
    fn default() -> Self {
        Self::ocean_waves(0.0)
    }
}

impl GerstnerWaveSystem {
    /// Create a new empty wave system.
    pub fn new(base_level: f32) -> Self {
        Self {
            waves: Vec::new(),
            base_level,
        }
    }

    /// Create a realistic ocean wave system with multiple wave frequencies.
    pub fn ocean_waves(base_level: f32) -> Self {
        let mut waves = Vec::new();
        
        // Primary wave - largest, slowest
        waves.push(GerstnerWave::new(
            Vec2::new(1.0, 0.3),
            0.6,
            8.0,
            1.5,
        ));
        
        // Secondary wave - medium size, different direction
        waves.push(GerstnerWave::new(
            Vec2::new(-0.7, 1.0),
            0.5,
            5.0,
            1.8,
        ));
        
        // Tertiary wave - smaller, faster
        waves.push(GerstnerWave::new(
            Vec2::new(0.5, -1.0),
            0.4,
            3.0,
            2.2,
        ));
        
        // Detail wave - smallest ripples
        waves.push(GerstnerWave::new(
            Vec2::new(-1.0, -0.5),
            0.3,
            1.5,
            2.8,
        ));
        
        Self { waves, base_level }
    }

    /// Create a calmer lake wave system.
    pub fn lake_waves(base_level: f32) -> Self {
        let mut waves = Vec::new();
        
        // Gentle primary wave
        waves.push(GerstnerWave::new(
            Vec2::new(1.0, 0.0),
            0.3,
            4.0,
            0.8,
        ));
        
        // Light secondary wave
        waves.push(GerstnerWave::new(
            Vec2::new(0.3, 1.0),
            0.2,
            2.5,
            1.0,
        ));
        
        Self { waves, base_level }
    }

    /// Add a wave to the system.
    pub fn add_wave(&mut self, wave: GerstnerWave) {
        self.waves.push(wave);
    }

    /// Calculate the total water surface height at a given position and time.
    pub fn get_surface_height(&self, position: Vec2, time: f32) -> f32 {
        let mut height = self.base_level;
        
        for wave in &self.waves {
            let (_, vertical) = wave.calculate_displacement(position, time);
            height += vertical;
        }
        
        height
    }

    /// Calculate the 3D position of a water surface point.
    ///
    /// This includes both horizontal displacement and vertical height,
    /// creating the characteristic circular motion of Gerstner waves.
    pub fn get_surface_position(&self, position: Vec2, time: f32) -> Vec3 {
        let mut horizontal_displacement = Vec2::ZERO;
        let mut vertical_displacement = 0.0;
        
        for wave in &self.waves {
            let (h, v) = wave.calculate_displacement(position, time);
            horizontal_displacement += h;
            vertical_displacement += v;
        }
        
        Vec3::new(
            position.x + horizontal_displacement.x,
            self.base_level + vertical_displacement,
            position.y + horizontal_displacement.y,
        )
    }

    /// Calculate the normal vector at a water surface point.
    pub fn get_surface_normal(&self, position: Vec2, time: f32) -> Vec3 {
        let mut combined_normal = Vec3::ZERO;
        
        for wave in &self.waves {
            combined_normal += wave.calculate_normal(position, time);
        }
        
        combined_normal.normalize_or(Vec3::Y)
    }

    /// Check if a point is below the water surface.
    ///
    /// # Arguments
    /// * `position` - 3D world position to check
    /// * `time` - Current time for wave animation
    ///
    /// # Returns
    /// `true` if the point is underwater (below the wave surface)
    pub fn is_underwater(&self, position: Vec3, time: f32) -> bool {
        let surface_height = self.get_surface_height(Vec2::new(position.x, position.z), time);
        position.y < surface_height
    }

    /// Calculate buoyancy force for an object partially or fully submerged.
    ///
    /// # Arguments
    /// * `position` - Center position of the object
    /// * `height` - Height of the object
    /// * `time` - Current time for wave animation
    ///
    /// # Returns
    /// Upward buoyancy force (positive = upward)
    pub fn calculate_buoyancy_force(&self, position: Vec3, height: f32, time: f32) -> f32 {
        let surface_height = self.get_surface_height(Vec2::new(position.x, position.z), time);
        let object_bottom = position.y - height / 2.0;
        let object_top = position.y + height / 2.0;
        
        if object_bottom >= surface_height {
            // Completely above water
            0.0
        } else if object_top <= surface_height {
            // Completely submerged
            1.0
        } else {
            // Partially submerged
            let submerged_fraction = (surface_height - object_bottom) / height;
            submerged_fraction.clamp(0.0, 1.0)
        }
    }

    /// Calculate horizontal water flow velocity at a position.
    ///
    /// This creates the effect of waves pushing objects horizontally.
    pub fn get_flow_velocity(&self, position: Vec2, time: f32) -> Vec2 {
        let mut velocity = Vec2::ZERO;
        
        for wave in &self.waves {
            let k = wave.wave_number();
            let omega = wave.frequency();
            let phase = k * wave.direction.dot(position) - omega * time;
            
            // Horizontal velocity component of Gerstner wave
            let amplitude = wave.steepness / k;
            velocity += wave.direction * amplitude * omega * phase.cos();
        }
        
        velocity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_creation() {
        let wave = GerstnerWave::new(Vec2::new(1.0, 0.0), 0.5, 4.0, 1.0);
        assert_eq!(wave.direction, Vec2::new(1.0, 0.0));
        assert_eq!(wave.steepness, 0.5);
    }

    #[test]
    fn test_surface_height() {
        let system = GerstnerWaveSystem::ocean_waves(10.0);
        let height = system.get_surface_height(Vec2::ZERO, 0.0);
        // Height should be near base level at t=0
        assert!((height - 10.0).abs() < 3.0);
    }

    #[test]
    fn test_underwater_detection() {
        let system = GerstnerWaveSystem::new(10.0);
        assert!(system.is_underwater(Vec3::new(0.0, 5.0, 0.0), 0.0));
        assert!(!system.is_underwater(Vec3::new(0.0, 15.0, 0.0), 0.0));
    }
}
