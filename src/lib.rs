use cute_dsp::filters::FilterType;
use nih_plug::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;

mod compressor;

use compressor::Compressor;

mod crossover;
use crossover::Crossover;

mod ui;
use ui::build_editor;
use ui::UiData;

use nih_plug_egui::EguiState;

use crate::ui::NUM_OF_FILTER_POINTS;
pub struct OpenMbc {
    params: Arc<OpenMbcParams>,
    sample_rate: f32,
    comp_filt_state: [CompFilter; MAX_MBCS],
    ui_data: Arc<Mutex<UiData>>,
}

#[derive(Params)]
struct OpenMbcParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[nested(array, group = "Comps")]
    pub comps: [CompParams; MAX_MBCS],
}

const MAX_MBCS: usize = 5;
const FREQ_RANGE_MIN: f32 = 10.0;
const FREQ_RANGE_MAX: f32 = 20_000.0;

#[derive(Params)]
struct CompParams {
    #[id = "enable"]
    pub enable: BoolParam,

    #[id = "center_freq"]
    pub center_freq: FloatParam,

    #[id = "q"] //TODO: rename to octaves or convert to Q...
    pub q: FloatParam,

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

const DEFAULT_SMOOTHING_MSEC: f32 = 5.0;
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
            )
            .with_smoother(SmoothingStyle::Linear(DEFAULT_SMOOTHING_MSEC)),
            ratio: FloatParam::new(
                "Ratio",
                1.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 10.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(DEFAULT_SMOOTHING_MSEC)),
            q: FloatParam::new(
                "Q",
                1.0,
                FloatRange::Linear {
                    min: 0.1,
                    max: 10.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(DEFAULT_SMOOTHING_MSEC)),
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
            )
            .with_smoother(SmoothingStyle::Linear(DEFAULT_SMOOTHING_MSEC)),
            release: FloatParam::new(
                "Release",
                100.0,
                FloatRange::Linear {
                    min: 10.0,
                    max: 10000.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(DEFAULT_SMOOTHING_MSEC)),
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
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
    filt: Crossover,
}
impl Default for CompFilter {
    fn default() -> Self {
        Self {
            comp: Compressor::new(0.0),
            filt: Crossover::new(0.0, FilterType::Bandpass),
        }
    }
}

impl Default for OpenMbc {
    fn default() -> Self {
        Self {
            params: Arc::new(OpenMbcParams::default()),
            sample_rate: 0.0,
            comp_filt_state: std::array::from_fn(|_| CompFilter::default()),
            ui_data: Arc::new(Mutex::new(UiData::default())),
        }
    }
}

impl Default for OpenMbcParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1024, 480),

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

        {
            let mut uidata = self.ui_data.lock().unwrap();

            uidata.sample_rate = self.sample_rate;
        }

        for (idx, comp_filt) in self.comp_filt_state.iter_mut().enumerate() {
            comp_filt.filt.sample_rate = self.sample_rate;
            comp_filt.filt.center_freq = self.params.comps[idx].center_freq.value();
            comp_filt.filt.octaves = self.params.comps[idx].q.value();
            comp_filt.filt.configure();

            comp_filt.comp.update_sample_rate(self.sample_rate);
        }

        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = params.editor_state.clone();
        let ui_data = self.ui_data.clone();
        build_editor(params, egui_state, ui_data)
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut tot_enabled_chs = 0; //small kombina, we start with one as we always have the aux channel
                                     //reconfigure all states
        for (idx, comp_filt) in self.comp_filt_state.iter_mut().enumerate() {
            let this_comp = &self.params.comps[idx];

            if this_comp.enable.value() {
                tot_enabled_chs += 1;
            }

            // handle bpf update
            if (this_comp.center_freq.smoothed.is_smoothing())
                || (this_comp.q.smoothed.is_smoothing())
            {
                comp_filt.filt.center_freq = this_comp.center_freq.smoothed.next();
                comp_filt.filt.octaves = this_comp.q.smoothed.next();
                comp_filt.filt.configure();
            }

            // handle comp update
            if this_comp.attack.smoothed.is_smoothing() {
                comp_filt
                    .comp
                    .solver
                    .update_attack(this_comp.attack.smoothed.next());
            }

            if this_comp.release.smoothed.is_smoothing() {
                comp_filt
                    .comp
                    .solver
                    .update_release(this_comp.release.smoothed.next());
            }

            if this_comp.ratio.smoothed.is_smoothing() {
                comp_filt
                    .comp
                    .solver
                    .update_ratio(this_comp.ratio.smoothed.next());
            }
            if this_comp.threshold.smoothed.is_smoothing() {
                comp_filt.comp.solver.threshold = this_comp.threshold.smoothed.next();
            }

            //TODO: missing params - makeup gain, knee width, compressor type

            //TODO: missing settings - side chain!
        }

        //redraw filters (when allowed)
        if let Ok(mut uidata) = self.ui_data.try_lock() {
            let mut none_enabled = true;
            uidata.set_filter_shape(0.0);

            //visualize each filter seperately
            self.comp_filt_state
                .iter()
                .enumerate()
                .for_each(|(idx, comp_filt)| {
                    if self.params.comps[idx].enable.value() {
                        let main_filter_mag = ui::utils::get_filter_shape(
                            self.sample_rate,
                            comp_filt.filt.get_main_filter(),
                            1.0,
                        );
                        let aux_filter_mag = ui::utils::get_filter_shape(
                            self.sample_rate,
                            comp_filt.filt.get_aux_filter(),
                            1.0,
                        );

                        let mut sum_filter_mag = [0.0_f64; NUM_OF_FILTER_POINTS];
                        let sum_all = uidata.borrow_filter_shape(3 * MAX_MBCS).unwrap();
                        for i in 0..NUM_OF_FILTER_POINTS {
                            sum_filter_mag[i] = main_filter_mag[i] + aux_filter_mag[i];
                            sum_all[i] += sum_filter_mag[i] / tot_enabled_chs as f64;
                        }

                        uidata
                            .borrow_filter_shape(3 * idx)
                            .unwrap()
                            .copy_from_slice(&main_filter_mag);
                        uidata
                            .borrow_filter_shape(3 * idx + 1)
                            .unwrap()
                            .copy_from_slice(&aux_filter_mag);
                        // uidata
                        //     .borrow_filter_shape(3 * idx + 2)
                        //     .unwrap()
                        //     .copy_from_slice(&sum_filter_mag);
                    }
                });
        }

        //THIS IS STEREO!
        for channel_samples in buffer.iter_samples() {
            for sample in channel_samples {
                // feed the signal to each filter seperately
                let total = self
                    .comp_filt_state
                    .iter_mut()
                    .enumerate()
                    .map(|(idx, comp_filt)| {
                        let this_comp_params = &self.params.comps[idx];

                        //filter
                        let (filt_main, filt_aux) = comp_filt.filt.process(*sample);

                        //compress
                        let comp = comp_filt.comp.process(filt_main, None);

                        //bypass
                        //TODO: maybe change this such that we don't even run the filter and compressor if not enabled, cheaper on processor
                        //FIXME: this is wrong, we're infact summing a notched singal MAX_MBCS times
                        if self.params.comps[idx].enable.value() {
                            comp * self.params.comps[idx].gain.smoothed.next()
                                * (1.0 / tot_enabled_chs as f32)
                                * 0.5
                                + filt_aux * (1.0 / tot_enabled_chs as f32) * 0.5
                        } else {
                            *sample * (1.0 / MAX_MBCS as f32)
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
