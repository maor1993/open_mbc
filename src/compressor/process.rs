use std::cmp::Ordering;

#[inline]
pub fn run_alpha_beta(coeff: f32, prev_val: f32, new_val: f32) -> f32 {
    return coeff * prev_val + (1.0 - coeff) * new_val;
}

#[derive(Default,Debug)]
pub enum CurveType {
    #[default]
    LogLin,
    LogSmoothDecoupled,
    LogSmoothBranching,
}



#[derive(Default, Debug)]
pub struct CompressorSolver {
    sample_rate: f32,
    pub threshold: f32,
    ratio: f32,
    knee_width: f32,
    knee_width_x0_5: f32,
    knee_width_x2: f32,
    pub attack_msec: f32,
    pub release_msec: f32,
    attack_coeff: f32,
    release_coeff_lin:f32,
    release_coeff: f32,
}

impl CompressorSolver {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ..Default::default()
        }
    }
    pub fn update_ratio(&mut self, ratio:f32){
        self.ratio = 1.0 - (1.0/ratio)
    }

    pub fn update_knee_width(&mut self, knee_width_db: f32) {
        self.knee_width = knee_width_db;
        self.knee_width_x0_5 = knee_width_db / 2.0;
        self.knee_width_x2 = knee_width_db * 2.0;
    }

    pub fn update_attack(&mut self, attack_msec: f32) {
        let attack_msec = attack_msec.max(0.0);
        self.attack_msec = attack_msec;

        if attack_msec == 0.0 {
            self.attack_coeff = 0.0;
        } else {
            self.attack_coeff = (0.10_f32.ln() / (attack_msec * self.sample_rate / 1000.0)).exp();
        }
    }
    pub fn update_release(&mut self, release_msec: f32) {
        let release_msec = release_msec.max(0.0);
        self.release_msec = release_msec;

        if release_msec == 0.0 {
            self.release_coeff_lin = 0.0;
            self.release_coeff = 0.0;
        } else {
            self.release_coeff_lin = 10.0 / (release_msec*self.sample_rate/1000.0);
            self.release_coeff = (0.10_f32.ln() / (release_msec * self.sample_rate / 1000.0)).exp();
        }
    }

    pub fn get_ideal_reduction(&self, input_level: f32) -> f32 {
        let diff_threshold = input_level - self.threshold;

        if self.knee_width == 0.0 {
            if diff_threshold <= 0.0 {
                return 0.0;
            } else {
                return diff_threshold * self.ratio;
            }
        }

        match diff_threshold.total_cmp(&self.knee_width_x0_5) {
            Ordering::Less => 0.0,
            Ordering::Greater => diff_threshold * self.ratio,
            Ordering::Equal => {
                let factor = diff_threshold + self.knee_width_x0_5;
                let factor_sq = factor * factor;

                (factor_sq / self.knee_width_x2) * self.ratio
            }
        }
    }

    fn curve_lin(&self, curr_reduction: f32, new_reduction: f32) -> (f32,f32) {
        if new_reduction >= curr_reduction {
            let res = run_alpha_beta(self.attack_coeff, curr_reduction, new_reduction);
            return (res,res)

        } else {
            let reduction = curr_reduction - self.release_coeff_lin;

            if reduction < new_reduction {
                (new_reduction,reduction)
            } else {
                (reduction,reduction)
            }
        }
    }

    fn curve_smoothdecoupled(&self, _curr_reduction: f32, _new_reduction: f32) -> (f32,f32) {
        
        todo!()
    }
    fn curve_smoothbranching(&self, _curr_reduction: f32, _new_reduction: f32) -> (f32,f32) {
        todo!()
    }

    pub fn apply_curve(
        &self,
        curr_reduction: f32,
        new_reduction: f32,
        curve_type: &CurveType,
    ) -> (f32,f32) {
        match curve_type {
            CurveType::LogLin => self.curve_lin(curr_reduction, new_reduction),
            CurveType::LogSmoothDecoupled => {
                self.curve_smoothdecoupled(curr_reduction, new_reduction)
            }
            CurveType::LogSmoothBranching => {
                self.curve_smoothbranching(curr_reduction, new_reduction)
            }
        }
    }
}
