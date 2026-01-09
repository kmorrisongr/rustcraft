//! High-performance water physics calculations.
//!
//! Optimized for batch queries needed by boats and multiple entities.

use super::config::WaveConfig;
use bevy::math::{Vec2, Vec3};
use std::f32::consts::PI;

/// Result of a water surface query at a single point.
#[derive(Debug, Clone, Copy, Default)]
pub struct WaterSample {
    /// World-space position on the water surface
    pub position: Vec3,
    /// Surface normal (for boat tilting, reflections)
    pub normal: Vec3,
    /// Surface height (Y coordinate)
    pub height: f32,
    /// Horizontal flow velocity (for pushing objects)
    pub flow_velocity: Vec2,
}

/// High-performance water physics calculator.
///
/// Designed for efficient batch queries - sample many points at once
/// for boats, large creatures, or particle systems.
pub struct WaterPhysicsWorld {
    config: WaveConfig,
    /// Precomputed wave constants to avoid redundant calculations
    wave_constants: [WaveConstants; 4],
}

/// Precomputed constants for a single wave.
#[derive(Clone, Copy, Default)]
struct WaveConstants {
    /// Wave number k = 2π / wavelength
    k: f32,
    /// Angular frequency ω = k * speed
    omega: f32,
    /// Amplitude = steepness / k
    amplitude: f32,
    /// Direction (cached for SIMD)
    dir_x: f32,
    dir_y: f32,
}

impl WaterPhysicsWorld {
    /// Create a new water physics world from configuration.
    pub fn new(config: WaveConfig) -> Self {
        let mut wave_constants = [WaveConstants::default(); 4];

        for (i, wave) in config.active_waves().enumerate() {
            let k = 2.0 * PI / wave.wavelength;
            wave_constants[i] = WaveConstants {
                k,
                omega: k * wave.speed,
                amplitude: (wave.steepness / k) * config.amplitude_scale,
                dir_x: wave.direction.x,
                dir_y: wave.direction.y,
            };
        }

        Self {
            config,
            wave_constants,
        }
    }

    /// Update the wave configuration (e.g., when weather changes).
    pub fn set_config(&mut self, config: WaveConfig) {
        *self = Self::new(config);
    }

    /// Get the current wave configuration.
    pub fn config(&self) -> &WaveConfig {
        &self.config
    }

    /// Sample the water surface at a single point.
    #[inline]
    pub fn sample(&self, x: f32, z: f32, time: f32) -> WaterSample {
        let mut height = self.config.base_level;
        let mut displacement_x = 0.0f32;
        let mut displacement_z = 0.0f32;
        let mut normal = Vec3::new(0.0, 1.0, 0.0);
        let mut flow_x = 0.0f32;
        let mut flow_z = 0.0f32;

        for i in 0..self.config.num_waves as usize {
            let wc = &self.wave_constants[i];
            let wave = &self.config.waves[i];

            // Phase = k * (dir · pos) - ω * t
            let dot = wc.dir_x * x + wc.dir_y * z;
            let phase = wc.k * dot - wc.omega * time;

            let (sin_phase, cos_phase) = phase.sin_cos();

            // Vertical displacement
            height += wc.amplitude * cos_phase;

            // Horizontal displacement (Gerstner circular motion)
            displacement_x += wc.dir_x * wc.amplitude * sin_phase;
            displacement_z += wc.dir_y * wc.amplitude * sin_phase;

            // Normal contribution
            let wa = wave.steepness * self.config.amplitude_scale;
            normal.x -= wc.dir_x * wa * cos_phase;
            normal.y -= wa * sin_phase; // Accumulates, will subtract from 1.0 later
            normal.z -= wc.dir_y * wa * cos_phase;

            // Flow velocity (derivative of displacement)
            let flow_mag = wc.amplitude * wc.omega * cos_phase;
            flow_x += wc.dir_x * flow_mag;
            flow_z += wc.dir_y * flow_mag;
        }

        // Finalize normal (Y component needs adjustment)
        normal.y = 1.0 + normal.y; // Convert accumulated negative to positive offset
        let normal = normal.normalize_or(Vec3::Y);

        WaterSample {
            position: Vec3::new(x + displacement_x, height, z + displacement_z),
            normal,
            height,
            flow_velocity: Vec2::new(flow_x, flow_z),
        }
    }

    /// Sample water height only (faster than full sample).
    #[inline]
    pub fn sample_height(&self, x: f32, z: f32, time: f32) -> f32 {
        let mut height = self.config.base_level;

        for i in 0..self.config.num_waves as usize {
            let wc = &self.wave_constants[i];
            let dot = wc.dir_x * x + wc.dir_y * z;
            let phase = wc.k * dot - wc.omega * time;
            height += wc.amplitude * phase.cos();
        }

        height
    }

    /// Batch sample multiple points efficiently.
    /// Useful for boats (sample at 4 corners) or large creatures.
    pub fn sample_batch(&self, points: &[(f32, f32)], time: f32, results: &mut [WaterSample]) {
        debug_assert_eq!(points.len(), results.len());

        for (i, &(x, z)) in points.iter().enumerate() {
            results[i] = self.sample(x, z, time);
        }
    }

    /// Batch sample heights only (fastest batch operation).
    pub fn sample_heights_batch(&self, points: &[(f32, f32)], time: f32, heights: &mut [f32]) {
        debug_assert_eq!(points.len(), heights.len());

        for (i, &(x, z)) in points.iter().enumerate() {
            heights[i] = self.sample_height(x, z, time);
        }
    }

    /// Calculate buoyancy data for a box-shaped object (e.g., boat hull).
    ///
    /// Samples water at the 4 corners of the object's base to determine:
    /// - Total buoyancy force (how much to push up)
    /// - Torque for tilting (pitch and roll)
    pub fn calculate_buoyancy_box(
        &self,
        center: Vec3,
        half_extents: Vec2, // (half_width, half_length)
        time: f32,
    ) -> BuoyancyResult {
        // Sample at 4 corners of the object's base
        let corners = [
            (center.x - half_extents.x, center.z - half_extents.y), // Back-left
            (center.x + half_extents.x, center.z - half_extents.y), // Back-right
            (center.x - half_extents.x, center.z + half_extents.y), // Front-left
            (center.x + half_extents.x, center.z + half_extents.y), // Front-right
        ];

        let mut samples = [WaterSample::default(); 4];
        self.sample_batch(&corners, time, &mut samples);

        // Calculate average submersion
        let object_bottom = center.y;
        let mut total_submersion = 0.0f32;
        let mut avg_flow = Vec2::ZERO;

        for sample in &samples {
            let submersion = (sample.height - object_bottom).max(0.0);
            total_submersion += submersion;
            avg_flow += sample.flow_velocity;
        }

        total_submersion /= 4.0;
        avg_flow /= 4.0;

        // Calculate tilt based on height differences
        let left_avg = (samples[0].height + samples[2].height) / 2.0;
        let right_avg = (samples[1].height + samples[3].height) / 2.0;
        let back_avg = (samples[0].height + samples[1].height) / 2.0;
        let front_avg = (samples[2].height + samples[3].height) / 2.0;

        // Roll = difference between left and right sides
        let roll = (right_avg - left_avg).atan2(half_extents.x * 2.0);
        // Pitch = difference between front and back
        let pitch = (front_avg - back_avg).atan2(half_extents.y * 2.0);

        // Average normal for surface alignment
        let avg_normal =
            (samples[0].normal + samples[1].normal + samples[2].normal + samples[3].normal)
                .normalize_or(Vec3::Y);

        BuoyancyResult {
            submersion_depth: total_submersion,
            buoyancy_force: total_submersion, // Multiply by object density/mass elsewhere
            pitch_angle: pitch,
            roll_angle: roll,
            flow_velocity: avg_flow,
            surface_normal: avg_normal,
        }
    }

    /// Check if a point is underwater.
    #[inline]
    pub fn is_underwater(&self, position: Vec3, time: f32) -> bool {
        position.y < self.sample_height(position.x, position.z, time)
    }

    /// Calculate how deep underwater a point is.
    /// Returns positive value if underwater, negative if above water.
    #[inline]
    pub fn depth_at(&self, position: Vec3, time: f32) -> f32 {
        self.sample_height(position.x, position.z, time) - position.y
    }
}

/// Result of buoyancy calculation for a floating object.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuoyancyResult {
    /// How deep the object is submerged (in world units)
    pub submersion_depth: f32,
    /// Normalized buoyancy force (0.0 = floating, 1.0 = fully submerged)
    pub buoyancy_force: f32,
    /// Pitch angle (forward/backward tilt) in radians
    pub pitch_angle: f32,
    /// Roll angle (left/right tilt) in radians
    pub roll_angle: f32,
    /// Average horizontal flow velocity at object position
    pub flow_velocity: Vec2,
    /// Average surface normal (for aligning object to waves)
    pub surface_normal: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water::config::WavePreset;

    #[test]
    fn test_still_water_height() {
        let config = WavePreset::Still.to_config(10.0);
        let physics = WaterPhysicsWorld::new(config);

        // Still water should always return base level
        assert_eq!(physics.sample_height(0.0, 0.0, 0.0), 10.0);
        assert_eq!(physics.sample_height(100.0, 100.0, 5.0), 10.0);
    }

    #[test]
    fn test_ocean_wave_variation() {
        let config = WavePreset::Ocean.to_config(10.0);
        let physics = WaterPhysicsWorld::new(config);

        let h1 = physics.sample_height(0.0, 0.0, 0.0);
        let h2 = physics.sample_height(0.0, 0.0, 1.0);
        let h3 = physics.sample_height(5.0, 5.0, 0.0);

        // Heights should vary over time and space
        assert_ne!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_underwater_detection() {
        let config = WavePreset::Still.to_config(10.0);
        let physics = WaterPhysicsWorld::new(config);

        assert!(physics.is_underwater(Vec3::new(0.0, 5.0, 0.0), 0.0));
        assert!(!physics.is_underwater(Vec3::new(0.0, 15.0, 0.0), 0.0));
    }

    #[test]
    fn test_buoyancy_calculation() {
        let config = WavePreset::Still.to_config(10.0);
        let physics = WaterPhysicsWorld::new(config);

        // Object half-submerged
        let result = physics.calculate_buoyancy_box(
            Vec3::new(0.0, 10.0, 0.0), // Center at water level
            Vec2::new(1.0, 2.0),       // 2x4 base
            0.0,
        );

        // Should have some buoyancy, no tilt on still water
        assert!(result.pitch_angle.abs() < 0.001);
        assert!(result.roll_angle.abs() < 0.001);
    }
}
