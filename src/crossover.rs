use cute_dsp::filters::{Biquad, FilterType};

pub struct Crossover {
    pub sample_rate: f32,
    pub center_freq: f32,
    pub octaves: f32,
    pub mode: FilterType,
    main_filter: Biquad<f32>,
    aux_filter: Biquad<f32>,
}

impl Crossover {
    pub fn new(sample_rate: f32, mode: FilterType) -> Self {
        let main_filter = Biquad::<f32>::new(true);
        let aux_filter = Biquad::<f32>::new(true);
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
                    .bandpass(self.center_freq / self.sample_rate, self.octaves);
                self.aux_filter
                    .notch(self.center_freq / self.sample_rate, self.octaves)
            }
            _ => todo!(),
        };
    }

    pub fn process(&mut self, sample: f32) -> (f32, f32) {
        (
            self.main_filter.process(sample),
            self.aux_filter.process(sample),
        )
    }
}
