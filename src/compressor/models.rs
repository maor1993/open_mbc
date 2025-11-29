use std::fmt::Debug;

use super::process::run_alpha_beta;

#[derive(Debug)]
pub enum CompressionEmulationEnum {
    Ideal(IdealCompressor), //peak
    Optical(OpticalCompressor),
    VCA(VCACompressor), //rms
}

impl CompressionEmulationEnum {
    pub fn new_ideal() -> Self {
        Self::Ideal(IdealCompressor)
    }
    pub fn new_optical(sample_rate: f32, steps: usize, coeffs_per_step: usize) -> Self {
        Self::Optical(OpticalCompressor::new(sample_rate, steps, coeffs_per_step))
    }
    pub fn new_vca(window_size_msec: f32) -> Self {
        Self::VCA(VCACompressor {
            current_reduction_sq: 0.0,
            window_size_msec,
        })
    }

    pub fn get_gain_reduction(&mut self, new_reduction: f32, ideal_reduction: f32) -> f32 {
        match self {
            CompressionEmulationEnum::Ideal(x) => {
                x.get_gain_reduction(new_reduction, ideal_reduction)
            }
            CompressionEmulationEnum::Optical(x) => {
                x.get_gain_reduction(new_reduction, ideal_reduction)
            }
            CompressionEmulationEnum::VCA(x) => {
                x.get_gain_reduction(new_reduction, ideal_reduction)
            }
        }
    }
}

pub trait CompressionModel: Debug {
    fn get_gain_reduction(&mut self, new_reduction: f32, ideal_reduction: f32) -> f32;
}

#[derive(Debug, Default)]
pub struct IdealCompressor;

impl CompressionModel for IdealCompressor {
    fn get_gain_reduction(&mut self, new_reduction: f32, _ideal_reduction: f32) -> f32 {
        new_reduction
    }
}

#[derive(Debug)]
pub struct OpticalCompressor {
    coeffs_per_step: usize,
    total_coeffs: usize,
    current_reduction: f32,
    attack_coeffs: Vec<f32>,
    release_coeffs: Vec<f32>,
    limit: f32,
}

impl OpticalCompressor {
    pub fn new(sample_rate: f32, steps: usize, coeffs_per_step: usize) -> Self {
        let total_coeffs = steps * coeffs_per_step;
        let mut attack_coeffs = vec![0.0_f32; total_coeffs];
        let mut release_coeffs = vec![0.0_f32; total_coeffs];

        for idx in 0..total_coeffs {
            let step_db = idx as f32 / coeffs_per_step as f32;
            let resistance = 480.0 / (3.0 + step_db);
            let attack_rate = resistance / 10.0;
            let release_rate = resistance; 

            attack_coeffs[idx] = (0.27_f32.ln() / (sample_rate * attack_rate / 1000.0)).exp();
            release_coeffs[idx] = (0.27_f32.ln() / (sample_rate * release_rate / 1000.0)).exp();
        }

        Self {
            attack_coeffs,
            release_coeffs,
            coeffs_per_step,
            total_coeffs,
            limit: 24.0,
            current_reduction: 0.0,
        }
    }
}

impl CompressionModel for OpticalCompressor {
    //we're modeling two behiviours 
    // old LDRs had slow rise and fall times, we model that by delaying the output of the compressor factor 
    // the LDR has an inverse log response curve, as such, at a certain limit the effect of the change in input power is saturated.

    fn get_gain_reduction(&mut self, new_reduction: f32, ideal_reduction: f32) -> f32 {
        let old_reduction = self.current_reduction;
        let ncoeff = (new_reduction * (self.coeffs_per_step as f32)) as isize;
        let ncoeff = ncoeff.clamp(0, (self.total_coeffs - 1) as isize) as usize;

        self.current_reduction = if new_reduction > self.current_reduction {
            run_alpha_beta(self.attack_coeffs[ncoeff], old_reduction, new_reduction)
        } else {
            run_alpha_beta(self.release_coeffs[ncoeff], old_reduction, new_reduction)
        };

        if self.current_reduction < ideal_reduction {
            let diff = ideal_reduction - self.current_reduction;

            ideal_reduction - (self.limit - (self.limit / (1.0 + (diff / self.limit))))
        } else {
            self.current_reduction
        }
    }
}

#[derive(Debug)]
pub struct VCACompressor {
    current_reduction_sq: f32,
    window_size_msec: f32,
}

impl VCACompressor {
    fn apply_rms_filter(&self, input_level: f32) -> (f32, f32) {
        if self.window_size_msec == 0.0 {
            return (self.current_reduction_sq, input_level);
        }

        let input_sq = input_level * input_level;

        let filtered_sq =
            run_alpha_beta(self.window_size_msec, self.current_reduction_sq, input_sq);

        (filtered_sq, filtered_sq.sqrt())
    }
}

impl CompressionModel for VCACompressor {
    //here, we model that compression is done on the RMS value of the signal and not the peak

    fn get_gain_reduction(&mut self, _new_reduction: f32, ideal_reduction: f32) -> f32 {
        let (filtered_sq, filtered) = self.apply_rms_filter(ideal_reduction);

        self.current_reduction_sq = filtered_sq;

        return filtered;
    }
}
