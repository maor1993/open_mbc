use cute_dsp::filters::Biquad;
use nih_plug::prelude::*;
use std::sync::Arc;


mod compressor;

use compressor::Compressor;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

struct OpenMbc {
    params: Arc<OpenMbcParams>,
    sample_rate: f32,
    comp_filt_state: [CompFilter; MAX_MBCS],
}

#[derive(Params)]
struct OpenMbcParams {
    /// The parameter's ID is used to identify the parameter in the wrappred plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined. In this case, this
    /// gain parameter is stored as linear gain while the values are displayed in decibels.
    #[nested(array, group = "Comps")]
    pub comps: [CompParams; MAX_MBCS],
}

const MAX_MBCS: usize = 5;
const FREQ_RANGE_MIN: f32 = 20.0;
const FREQ_RANGE_MAX: f32 = 20_000.0;

#[derive(Params)]
struct CompParams {
    #[id = "enable"]
    pub enable: BoolParam,

    #[id = "center_freq"]
    pub center_freq: FloatParam,

    #[id = "q"]
    pub q: FloatParam,
    //TODO:
    #[id = "threshold"]
    pub threshold: FloatParam,

    #[id = "ratio"]
    pub ratio: FloatParam,

    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,

    #[id = "gain"]
    pub gain: FloatParam,
}

impl Default for CompParams {
    fn default() -> Self {
        Self {
            enable: BoolParam::new("Enable", false),
            center_freq: FloatParam::new(
                "Center",
                1000.0,
                FloatRange::Linear {
                    min: FREQ_RANGE_MIN,
                    max: FREQ_RANGE_MAX,
                },
            ),
            ratio: FloatParam::new(
                "Ratio",
                1.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 10.0,
                },
            ),
            q: FloatParam::new(
                "Q",
                1.0,
                FloatRange::Linear {
                    min: 0.1,
                    max: 10.0,
                },
            ),
            threshold: FloatParam::new(
                "Threshold",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-99.0),
                    max: util::db_to_gain(0.0),
                    factor: 0.7,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            attack: FloatParam::new(
                "Attack",
                10.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 1000.0,
                },
            ),
            release: FloatParam::new(
                "Release",
                100.0,
                FloatRange::Linear {
                    min: 10.0,
                    max: 10000.0,
                },
            ),
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-10.0),
                    max: util::db_to_gain(30.0),
                    factor: 0.7,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

struct CompFilter {
    comp: Compressor,
    filt: Biquad<f32>,
}
impl Default for CompFilter {
    fn default() -> Self {
        Self {
            comp: Compressor::new(0.0),
            filt: Biquad::<f32>::new(true),
        }
    }
}

impl Default for OpenMbc {
    fn default() -> Self {
        Self {
            params: Arc::new(OpenMbcParams::default()),
            sample_rate: 0.0,
            comp_filt_state: std::array::from_fn(|_| CompFilter::default()),
        }
    }
}

impl Default for OpenMbcParams {
    fn default() -> Self {
        Self {
            // This gain is stored as linear gain. NIH-plug comes with useful conversion functions
            // to treat these kinds of parameters as if we were dealing with decibels. Storing this
            // as decibels is easier to work with, but requires a conversion for every sample.
            comps: std::array::from_fn(|_| CompParams::default()),
        }
    }
}

impl Plugin for OpenMbc {
    const NAME: &'static str = "Open Mbc";
    const VENDOR: &'static str = "Maor Malka";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "maor1993@outlook.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = _buffer_config.sample_rate;

        for (idx, comp_filt) in self.comp_filt_state.iter_mut().enumerate() {
            comp_filt.filt.bandpass(
                self.sample_rate / self.params.comps[idx].center_freq.value(),
                self.params.comps[idx].q.value(),
            );
        }

        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        //reconfigure all states
        for (idx, comp_filt) in self.comp_filt_state.iter_mut().enumerate() {
            comp_filt.filt.bandpass(
                self.params.comps[idx].center_freq.value() / self.sample_rate,
                self.params.comps[idx].q.value(),
            );

        }
        //THIS IS STEREO!
        for channel_samples in buffer.iter_samples() {
            for sample in channel_samples {
                // feed the signal to each filter seperately
                let total = self
                    .comp_filt_state
                    .iter_mut()
                    .map(|comp_filt| {
                        // let smp = comp_filt.filt.process(*sample);
                        comp_filt.comp.process(*sample, None)
                    })
                    .enumerate()
                    .map(|(idx, smp)| {
                        if self.params.comps[idx].enable.value() {
                            smp * self.params.comps[idx].gain.smoothed.next()
                                * (1.0 / MAX_MBCS as f32)
                        } else {
                            0.0
                        }
                    })
                    .sum();

                *sample = total;

                *sample = sample.clamp(-1.5, 1.5); //hard limit to no more than 3.5dB over
            }
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for OpenMbc {
    const VST3_CLASS_ID: [u8; 16] = *b"openmbc_mm123456";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_vst3!(OpenMbc);
