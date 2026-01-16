use noise::{Fbm, MultiFractal, NoiseFn, Perlin, RidgedMulti};

/// 2D FBM with 5 octaves in [0;1]
pub fn fbm(x: f32, width: usize, z: f32, height: usize, seed: u32, freq: f32) -> Vec<f32> {
    let noise: Fbm<Perlin> = Fbm::new(seed)
        .set_octaves(5)
        .set_frequency(freq as f64);
    
    let mut res = Vec::with_capacity(width * height);
    for zi in 0..height {
        for xi in 0..width {
            let sample = noise.get([(x as f64 + xi as f64), (z as f64 + zi as f64)]);
            // Fbm output is approximately [-1, 1], scale to [0, 1]
            res.push((sample * 0.5 + 0.5) as f32);
        }
    }
    res
}

/// 2D FBM with 5 octaves in [min;max]
pub fn fbm_scaled(
    x: f32, width: usize, z: f32, height: usize, seed: u32, freq: f32, min: f32, max: f32
) -> Vec<f32> {
    let noise: Fbm<Perlin> = Fbm::new(seed)
        .set_octaves(5)
        .set_frequency(freq as f64);
    
    let delta = (max - min) as f64;
    let mut res = Vec::with_capacity(width * height);
    for zi in 0..height {
        for xi in 0..width {
            let sample = noise.get([(x as f64 + xi as f64), (z as f64 + zi as f64)]);
            // Fbm output is approximately [-1, 1], scale to [min, max]
            res.push((sample * 0.5 * delta + 0.5 * delta + min as f64) as f32);
        }
    }
    res
}

/// 2D Ridge noise in [0;1]
pub fn ridge(x: f32, width: usize, z: f32, height: usize, seed: u32, freq: f32) -> Vec<f32> {
    let noise: RidgedMulti<Perlin> = RidgedMulti::new(seed)
        .set_frequency(freq as f64);
    
    let mut res = Vec::with_capacity(width * height);
    for zi in 0..height {
        for xi in 0..width {
            let sample = noise.get([(x as f64 + xi as f64), (z as f64 + zi as f64)]);
            // RidgedMulti output is approximately [-1, 1], scale to [0, 1]
            res.push(((sample + 1.0) * 0.5).clamp(0.0, 1.0) as f32);
        }
    }
    res
}

/// 2D Ridge noise in [min;max]
pub fn ridge_scaled(
    x: f32, width: usize, z: f32, height: usize, seed: u32, freq: f32, min: f32, max: f32
) -> Vec<f32> {
    let noise: RidgedMulti<Perlin> = RidgedMulti::new(seed)
        .set_frequency(freq as f64);
    
    let delta = (max - min) as f64;
    let mut res = Vec::with_capacity(width * height);
    for zi in 0..height {
        for xi in 0..width {
            let sample = noise.get([(x as f64 + xi as f64), (z as f64 + zi as f64)]);
            // RidgedMulti output is approximately [-1, 1], scale to [min, max]
            let normalized = ((sample + 1.0) * 0.5).clamp(0.0, 1.0);
            res.push((normalized * delta + min as f64) as f32);
        }
    }
    res
}

pub fn quantize(sample: &mut Vec<f32>, step: f32) {
    sample.iter_mut().for_each(|v| *v = (*v/step).round()*step);   
}

pub fn mul(a: &mut Vec<f32>, b: &Vec<f32>) {
    a.iter_mut().zip(b).for_each(|(a, b)| *a *= b);
}

pub fn add(a: &mut Vec<f32>, b: &Vec<f32>) {
    a.iter_mut().zip(b).for_each(|(a, b)| *a += b);
}

pub fn add_const(a: &mut Vec<f32>, c: f32) {
    a.iter_mut().for_each(|a| *a += c);
}

pub fn mul_const(a: &mut Vec<f32>, c: f32) {
    a.iter_mut().for_each(|a| *a *= c);
}

pub fn powi(a: &mut Vec<f32>, n: i32) {
    a.iter_mut().for_each(|a| *a = a.powi(n));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_bounds(sample: Vec<f32>, min: f32, max: f32) {
        let delta = max - min;
        let error = delta * 0.05;
        let smin = sample.iter().cloned().fold(f32::INFINITY, f32::min);
        let smax = sample.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        println!("min: {}, max: {}", smin, smax);
        assert!(smin >= min);
        assert!(smin < min+error);
        assert!(smax <= max);
        assert!(smax > max-error);
    }

    #[test]
    fn fbm_len() {
        let len = 1024;
        let res = fbm(0.0, len, 0.0, len, 42, 0.01);
        assert_eq!(res.len(), len*len);
    }

    #[test]
    fn ridge_len() {
        let len = 1024;
        let res = ridge(0.0, len, 0.0, len, 42, 0.01);
        assert_eq!(res.len(), len*len);
    }

    #[test]
    fn fbm_domain() {
        let len = 4096;
        let res = fbm(0.0, len, 0.0, len, 11111, 0.333333);
        assert_bounds(res, 0., 1.);
    }

    #[test]
    fn ridge_domain() {
        let len = 4096;
        let res = ridge(0.0, len, 0.0, len, 42, 0.333333);
        assert_bounds(res, 0., 1.);
    }

    #[test]
    fn fbm_scaled_domain() {
        let len = 4096;
        let res = fbm_scaled(0.0, len, 0.0, len, 42, 0.333333, 50., 100.);
        assert_bounds(res, 50., 100.);
    }

    #[test]
    fn ridge_scaled_domain() {
        let len = 4096;
        let res = ridge_scaled(0.0, len, 0.0, len, 42, 0.333333, 50., 100.);
        assert_bounds(res, 50., 100.);
    }
}