use cute_dsp::filters::Biquad;
use nih_plug::prelude::Enum;
use num_complex::Complex64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Enum)]
pub enum FilterSlope {
    #[default]
    Slope12dB = 1,
    Slope24dB = 2,
    Slope36dB = 3,
    Slope48dB = 4,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Enum)]
pub enum FilterTypeCx {
    #[default]
    Bandpass = 0,
    Lowpass = 1,
    Highpass = 2,
}

pub struct Crossover {
    pub sample_rate: f32,
    pub center_freq: f32,
    pub octaves: f32,
    pub _spacing: f32,
    pub mode: FilterTypeCx,
    filters: [[Biquad<f64>; 2]; 5],
}

impl Crossover {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            center_freq: 0.0,
            octaves: 0.0,
            _spacing: 500.0,
            mode: FilterTypeCx::Bandpass,
            filters: std::array::from_fn(|_| std::array::from_fn(|_| Biquad::new(true))),
        }
    }

    pub fn configure(&mut self) {
        for filts in self.filters.iter_mut() {
            match self.mode {
                FilterTypeCx::Bandpass => {
                    filts[0].bandpass_q(
                        (self.center_freq / self.sample_rate) as f64,
                        self.octaves as f64,
                    );
                    filts[1].bandpass_q(
                        (self.center_freq / self.sample_rate) as f64,
                        self.octaves as f64,
                    );
                }
                FilterTypeCx::Lowpass => {
                    filts[0].low_shelf_q(
                        (self.center_freq / self.sample_rate) as f64,
                        self.octaves as f64,
                    );
                    filts[1].low_shelf_q(
                        (self.center_freq / self.sample_rate) as f64,
                        self.octaves as f64,
                    );
                }
                FilterTypeCx::Highpass => {
                    filts[0].high_shelf_q(
                        (self.center_freq / self.sample_rate) as f64,
                        self.octaves as f64,
                    );
                    filts[1].high_shelf_q(
                        (self.center_freq / self.sample_rate) as f64,
                        self.octaves as f64,
                    );
                }
                _ => todo!(),
            }
        }
    }

    pub fn process(
        &mut self,
        sample: f32,
        sc: Option<f32>,
        stages: FilterSlope,
    ) -> (f32, Option<f32>) {
        let init_values = match sc {
            Some(smp) => (sample, smp),
            None => (sample, sample),
        };

        let results =
            self.filters
                .iter_mut()
                .take(stages as usize)
                .fold(init_values, |prev, filt| {
                    (
                        filt[0].process(prev.0 as f64) as f32,
                        filt[1].process(prev.1 as f64) as f32,
                    )
                });

        if sc.is_some() {
            (results.0, Some(results.1))
        } else {
            (results.0, None)
        }
    }

    /*     pub fn get_main_filter(&self,) -> &Biquad<f64> {
        &self.filters[0]
    } */

    pub fn get_main_filter_response(
        &self,
        normalized_freq: f64,
        stages: &FilterSlope,
    ) -> Complex64 {
        self.filters
            .iter()
            .take(*stages as usize)
            .fold(Complex64::ONE, |prev, filt| {
                prev * filt[0].get_complex_response(normalized_freq)
            })
    }
}
