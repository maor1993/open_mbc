use cute_dsp::filters::{Biquad, FilterType};

pub struct Crossover {
    pub sample_rate: f32,
    pub center_freq: f32,
    pub octaves: f32,
    pub _spacing: f32,
    pub mode: FilterType,
    filters: [Biquad<f64>; 2],
}

impl Crossover {
    pub fn new(sample_rate: f32, mode: FilterType) -> Self {
        Self {
            sample_rate,
            center_freq: 0.0,
            octaves: 0.0,
            _spacing: 500.0,
            mode,
            filters: std::array::from_fn(|_| Biquad::new(true)),
        }
    }

    pub fn configure(&mut self) {
        match self.mode {
            FilterType::Bandpass => {
                self.filters[0].bandpass_q(
                    (self.center_freq / self.sample_rate) as f64,
                    self.octaves as f64,
                );
                self.filters[1].bandpass_q(
                    (self.center_freq / self.sample_rate) as f64,
                    self.octaves as f64,
                );
            },
            FilterType::Lowpass => {
                self.filters[0].lowpass(
                    (self.center_freq / self.sample_rate) as f64,
                    self.octaves as f64,
                    cute_dsp::filters::BiquadDesign::Bilinear
                );
                self.filters[1].lowpass(
                    (self.center_freq / self.sample_rate) as f64,
                    self.octaves as f64,
                    cute_dsp::filters::BiquadDesign::Bilinear
                );
            },
            FilterType::Highpass => {
                self.filters[0].highpass(
                    (self.center_freq / self.sample_rate) as f64,
                    self.octaves as f64,
                    cute_dsp::filters::BiquadDesign::Bilinear
                );
                self.filters[1].highpass(
                    (self.center_freq / self.sample_rate) as f64,
                    self.octaves as f64,
                    cute_dsp::filters::BiquadDesign::Bilinear
                );
            }
            _ => todo!(),
        };
    }

    pub fn process(&mut self, sample: f32,sc:Option<f32>) -> (f32, Option<f32>) {
        let main = self.filters[0].process(sample as f64) as f32;
    
        match sc {
            Some(smp) => (main,Some(self.filters[1].process(smp as f64) as f32)),
            None => (main,None)
        }
        
    }

    pub fn get_main_filter(&self) -> &Biquad<f64> {
        &self.filters[0]
    }
    pub fn _get_aux_filter(&self) -> &Biquad<f64> {
        &self.filters[1]
    }
}
