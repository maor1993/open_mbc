use cute_dsp::filters::{Biquad, FilterType};

pub struct Crossover {
    pub sample_rate: f32,
    pub center_freq: f32,
    pub octaves: f32,
    pub mode: FilterType,
    main_filter: Biquad<f64>,
    aux_filter: Biquad<f64>,
}

impl Crossover {
    pub fn new(sample_rate: f32, mode: FilterType) -> Self {
        let main_filter = Biquad::<f64>::new(true);
        let aux_filter = Biquad::<f64>::new(true);
        Self {
            sample_rate,
            center_freq: 0.0,
            octaves: 0.0,
            mode,
            main_filter,
            aux_filter,
        }
    }

    pub fn configure(&mut self) {
        match self.mode {
            FilterType::Bandpass => {
                self.main_filter
                    .bandpass((self.center_freq / self.sample_rate) as f64, self.octaves  as f64);
                self.aux_filter
                    .notch((self.center_freq / self.sample_rate) as f64, self.octaves as f64)
            }
            _ => todo!(),
        };
    }

    pub fn process(&mut self, sample: f32) -> (f32, f32) {
        (
            self.main_filter.process(sample as f64) as f32,
            self.aux_filter.process(sample as f64) as f32,
        )
    }

    pub fn get_main_filter(&self) -> &Biquad<f64> {
        &self.main_filter
    }
    pub fn get_aux_filter(&self) -> &Biquad<f64> {
        &self.aux_filter
    }
}
