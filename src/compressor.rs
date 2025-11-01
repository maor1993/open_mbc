use nih_plug::prelude::*;

#[derive(Default, Debug)]
pub struct Compressor {
    triggered: bool,
    threshold: f32,
    ratio: f32,
    sample_cnt: f32,
    last_ratio: f32,
    release_sample_cnt: f32,
    attack_samples: f32,
    release_samples: f32,
}

impl Compressor {
    pub fn update_params(
        &mut self,
        sample_rate: f32,
        threshold: f32,
        ratio: f32,
        attack_msec: f32,
        release_msec: f32,
    ) {
        self.threshold = threshold;
        self.ratio = 1.0 - (1.0 / ratio);
        self.attack_samples = (sample_rate * attack_msec / 1000.0).round();
        self.release_samples = (sample_rate * release_msec / 1000.0).round();
    }

    pub fn process(&mut self, smp: f32, _sidechain: Option<f32>) -> f32 {
        let smp_db = util::gain_to_db_fast(smp.abs());

        if smp_db > self.threshold {
            if !self.triggered {
                self.triggered = true;

                //if start will be zero, if in middle of compression will be the last sample value
                self.sample_cnt = self.release_sample_cnt;
            }
        } else if self.triggered {
            self.triggered = false;
            self.release_sample_cnt = self.sample_cnt;
            self.sample_cnt = 0.0;
        }

        self.last_ratio = if self.triggered {
            if self.sample_cnt < self.attack_samples {
                self.sample_cnt += 1.0;
                1.0 - (self.ratio * (self.sample_cnt / self.attack_samples))
            } else {
                self.ratio
            }
        } else {
            if self.sample_cnt < self.release_samples {
                self.sample_cnt += 1.0;
                self.release_sample_cnt -= 1.0;
                (1.0 - self.ratio) * (self.sample_cnt / self.release_samples) + self.ratio
            } else {
                self.release_sample_cnt = 0.0;
                1.0
            }
        };
        smp * self.last_ratio
    }
}

#[cfg(test)]
mod tests {
    use super::Compressor;
    #[test]
    fn run_compressor_ex001() {
        let mut comp = Compressor {
            attack_samples: 10.0,
            release_samples: 100.0,
            ratio: 1.0 / 4.0,
            threshold: -12.0,
            ..Default::default()
        };

        //let samples:[f32;1000] = std::array::from_fn(|x| ((x as f32)*0.001).min(0.252));

        let mut samples = Vec::<f32>::new();

        for i in 0..250 {
            samples.push(((i as f32) * 0.002).min(0.252));
        }

        for _i in 0..10 {
            samples.push(0.0);
        }
        for _i in 0..10 {
            samples.push(0.252);
        }
        for i in 0..250 {
            samples.push(0.252 - (i as f32) * 0.001)
        }

        for (idx, sample) in samples.iter().enumerate() {
            let smp = comp.process(*sample, None);
            println!(
                "idx:{},smp_in:{:.3},smp_out:{:.3}, compressor {:?}",
                idx, sample, smp, comp
            );
        }
    }
}
