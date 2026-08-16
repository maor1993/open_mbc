use nice_plug::prelude::*;
use nice_plug::util::gain_to_db_fast;
use num_complex::Complex64;
use std::sync::Arc;
use std::sync::Mutex;

mod compressor;

use compressor::Compressor;

mod crossover;
use crossover::Crossover;

mod ui;
use ui::build_editor;
use ui::PeakMeter;
use ui::UiData;

use nice_plug_egui::EguiState;

use cute_dsp::stft::STFT;

use crate::ui::NUM_OF_FILTER_POINTS;
use crate::ui::NUM_OF_VIZ_FFT_POINTS;
pub struct OpenMbc {
    params: Arc<OpenMbcParams>,
    sample_rate: f32,
    comp_filt_state: [[CompFilter; 2]; MAX_MBCS], //stereo channel per compressor
    ui_data: Arc<Mutex<UiData>>,
    spectrum_handle: SpecturmSubSystem,
    peak_meter: [ui::PeakMeter; 2],
    peak_meter_result: Arc<[AtomicF32; 2]>,
}

#[derive(Params)]
struct OpenMbcParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[nested(array, group = "Comps")]
    pub comps: [CompParams; MAX_MBCS],

    #[id = "solo"]
    pub solo: IntParam,

    #[id = "stereo_mix"]
    pub stereo_mix: FloatParam,

    #[id = "mid_side"]
    pub mid_side: BoolParam,
}

const MAX_MBCS: usize = 5;
const MAX_MBCS_2X: usize = MAX_MBCS * 2;
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

    #[id = "slope"]
    pub slope: EnumParam<crossover::FilterSlope>,

    #[id = "filtertype"]
    pub filtertype: EnumParam<crossover::FilterTypeCx>,

    #[id = "threshold"]
    pub threshold: FloatParam,

    #[id = "range"]
    pub range: FloatParam,

    #[id = "ratio"]
    pub ratio: FloatParam,

    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "sidechain"]
    pub sidechain: BoolParam,
}

struct SpecturmSubSystem {
    pre_comp_stft_handle: STFT<f32>,
    post_comp_stft_handle: STFT<f32>,
    sc_stft_handle: STFT<f32>,
    samples_in_buf: usize,
}

impl SpecturmSubSystem {
    fn new() -> Self {
        Self {
            pre_comp_stft_handle: STFT::new(false),
            post_comp_stft_handle: STFT::new(false),
            sc_stft_handle: STFT::new(false),
            samples_in_buf: 0,
        }
    }

    fn configure(&mut self) {
        self.pre_comp_stft_handle.configure(
            1,
            0,
            NUM_OF_VIZ_FFT_POINTS,
            0,
            NUM_OF_VIZ_FFT_POINTS / 4,
        );
        self.post_comp_stft_handle.configure(
            1,
            0,
            NUM_OF_VIZ_FFT_POINTS,
            0,
            NUM_OF_VIZ_FFT_POINTS / 4,
        );
        self.sc_stft_handle
            .configure(1, 0, NUM_OF_VIZ_FFT_POINTS, 0, NUM_OF_VIZ_FFT_POINTS / 4);
    }
}

const DEFAULT_SMOOTHING_MSEC: f32 = 5.0;
impl Default for CompParams {
    fn default() -> Self {
        Self {
            enable: BoolParam::new("Enable", false),
            center_freq: FloatParam::new(
                "Freq",
                1000.0,
                FloatRange::Linear {
                    min: FREQ_RANGE_MIN,
                    max: FREQ_RANGE_MAX,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(DEFAULT_SMOOTHING_MSEC)),
            ratio: FloatParam::new(
                "Ratio",
                4.0,
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
            slope: EnumParam::<crossover::FilterSlope>::new(
                "Slope",
                crossover::FilterSlope::Slope12dB,
            ),
            filtertype: EnumParam::<crossover::FilterTypeCx>::new(
                "Filtertype",
                crossover::FilterTypeCx::Bandpass,
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
            range: FloatParam::new(
                "Range",
                util::db_to_gain(-10.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
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

            sidechain: BoolParam::new("sidechain", false),
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
            filt: Crossover::new(0.0),
        }
    }
}

impl Default for OpenMbc {
    fn default() -> Self {
        Self {
            params: Arc::new(OpenMbcParams::default()),
            sample_rate: 0.0,
            comp_filt_state: std::array::from_fn(|_| {
                std::array::from_fn(|_| CompFilter::default())
            }),
            ui_data: Arc::new(Mutex::new(UiData::default())),
            spectrum_handle: SpecturmSubSystem::new(),
            peak_meter: std::array::from_fn(|_| PeakMeter::new()),
            peak_meter_result: Arc::new([AtomicF32::new(0.0), AtomicF32::new(0.0)]),
        }
    }
}

impl Default for OpenMbcParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(nice_plug::editor::dpi::LogicalSize { width: 1024.0, height: 640.0 }),

            comps: std::array::from_fn(|_| CompParams::default()),
            solo: IntParam::new(
                "solo_filter",
                -1,
                IntRange::Linear {
                    min: -1,
                    max: MAX_MBCS as i32,
                },
            )
            .hide(),
            stereo_mix: FloatParam::new(
                "stereo_mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(DEFAULT_SMOOTHING_MSEC)),

            mid_side: BoolParam::new("mid_side", false),
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
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),

            aux_input_ports: &[new_nonzero_u32(2)],

            ..AudioIOLayout::const_default()
        },
        // AudioIOLayout {
        //     main_input_channels: NonZeroU32::new(1),
        //     main_output_channels: NonZeroU32::new(1),

        //     aux_input_ports: &[new_nonzero_u32(1)],

        //     ..AudioIOLayout::const_default()
        // },
    ];

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
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = buffer_config.sample_rate;

        {
            let mut uidata = self.ui_data.lock().unwrap();
            uidata.sample_rate = self.sample_rate;

            for peakmeter in self.peak_meter.iter_mut() {
                peakmeter.update_decay(ui::PEAK_METER_DECAY_MS, self.sample_rate);
            }
        }
        self.spectrum_handle.configure();

        //Note that this section also handles restoration of parameters when project is loaded, 
        //as such, we will take the values provided by tool.
        for (idx, cf_channel) in self.comp_filt_state.iter_mut().enumerate() {
            for comp_filt in cf_channel.iter_mut() {
                comp_filt.filt.sample_rate = self.sample_rate;
                comp_filt.filt.center_freq = self.params.comps[idx].center_freq.value();
                comp_filt.filt.octaves = self.params.comps[idx].q.value();

                comp_filt.filt.configure();

                comp_filt.comp.update_sample_rate(self.sample_rate);

                comp_filt.comp.solver.threshold = gain_to_db_fast(self.params.comps[idx].threshold.value());
                comp_filt.comp.max_reduciton = -gain_to_db_fast(self.params.comps[idx].range.value());
      
                comp_filt.comp.solver.update_attack(self.params.comps[idx].attack.value());
                comp_filt.comp.solver.update_release(self.params.comps[idx].release.value());
                comp_filt.comp.solver.update_ratio(self.params.comps[idx].ratio.value());
            }
        }

        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = params.editor_state.clone();
        let ui_data = self.ui_data.clone();
        let pmv = self.peak_meter_result.clone();
        build_editor(params, egui_state, ui_data, pmv)
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        //redraw filters (when allowed)

        if self.params.editor_state.is_open() {
            if let Ok(mut uidata) = self.ui_data.try_lock() {
                //TODO: might skip this as we process all the relevant indecies anyway
                uidata.set_filter_shape(0.0);

                let mut main_filters = [[Complex64::new(0.0, 0.0); NUM_OF_FILTER_POINTS]; MAX_MBCS];
                //visualize each filter seperately
                self.comp_filt_state
                    .iter()
                    .enumerate()
                    .for_each(|(idx, comp_filt)| {
                        if self.params.comps[idx].enable.value() {
                            let main_filter_mag = ui::utils::get_filter_shape(
                                self.sample_rate,
                                &comp_filt[0].filt,
                                self.params.comps[idx].slope.value(),
                                1.0,
                            );

                            main_filters[idx].copy_from_slice(&main_filter_mag);

                            // create solo shape
                            let gain = self.params.comps[idx].gain.value();
                            {
                                let filt_plot = uidata.borrow_filter_shape(idx).unwrap();
                                if self.params.solo.value() == idx as i32 {
                                    for i in 0..NUM_OF_FILTER_POINTS {
                                        filt_plot[i] = main_filter_mag[i].norm()
                                    }
                                } else {
                                    for i in 0..NUM_OF_FILTER_POINTS {
                                        filt_plot[i] = (Complex64::ONE - main_filter_mag[i]
                                            + main_filter_mag[i]
                                                * (self.params.comps[idx].range.value() * gain)
                                                    as f64)
                                            .norm();
                                    }
                                }
                            }

                            // create no range plot
                            {
                                let filt_plot_no_range =
                                    uidata.borrow_filter_shape(MAX_MBCS + idx).unwrap();

                                for i in 0..NUM_OF_FILTER_POINTS {
                                    filt_plot_no_range[i] = (Complex64::ONE - main_filter_mag[i]
                                        + main_filter_mag[i] * gain as f64)
                                        .norm();
                                }
                            }
                        }
                    });

                for i in 0..NUM_OF_FILTER_POINTS {
                    let mut h_total_filtered = Complex64::ZERO;
                    let mut h_static_sum = Complex64::ZERO;
                    let mut h_total_gain_only = Complex64::ZERO;
                    for (idx, comp_filt) in self.comp_filt_state.iter().enumerate() {
                        //TODO: all of these enables should be downstreamed to the struct and read from there.
                        if self.params.comps[idx].enable.value() {
                            let gain = self.params.comps[idx].gain.value() as f64;
                            let comp_as_gain = nice_plug::util::db_to_gain_fast(
                                -comp_filt[0].comp.curr_reduction_post_model,
                            ) as f64;

                            h_total_filtered += main_filters[idx][i] * comp_as_gain * gain;
                            h_static_sum += main_filters[idx][i];

                            h_total_gain_only += main_filters[idx][i] * gain;
                        }

                        {
                            let sum_plot = uidata.borrow_filter_shape(3 * MAX_MBCS).unwrap();
                            sum_plot[i] = (Complex64::ONE - h_static_sum + h_total_filtered).norm();
                        }
                        {
                            let sum_plot_no_compress =
                                uidata.borrow_filter_shape(3 * MAX_MBCS - 1).unwrap();
                            sum_plot_no_compress[i] =
                                (Complex64::ONE - h_static_sum + h_total_gain_only).norm();
                        }
                    }
                }

                // GAIN REDUCTION
                for i in 0..MAX_MBCS {
                    uidata.gain_reduction[i] =
                        self.comp_filt_state[i][0].comp.curr_reduction_post_model;
                }

                // SPECTURM
                if self.spectrum_handle.samples_in_buf >= NUM_OF_VIZ_FFT_POINTS {
                    let spectrum_pre = self
                        .spectrum_handle
                        .pre_comp_stft_handle
                        .process_block_to_spectrum(0);
                    let spectrum_post = self
                        .spectrum_handle
                        .post_comp_stft_handle
                        .process_block_to_spectrum(0);

                    let spectrum_sc = self
                        .spectrum_handle
                        .sc_stft_handle
                        .process_block_to_spectrum(0);

                    for i in 0..NUM_OF_VIZ_FFT_POINTS / 2 {
                        uidata.prev_spectrogram_pre[i] = uidata.signal_spectrogram_pre[i];
                        uidata.signal_spectrogram_pre[i] =
                            spectrum_pre[i].norm() / (NUM_OF_VIZ_FFT_POINTS / 2) as f32;
                        uidata.prev_spectrogram_post[i] = uidata.signal_spectrogram_post[i];
                        uidata.signal_spectrogram_post[i] =
                            spectrum_post[i].norm() / (NUM_OF_VIZ_FFT_POINTS / 2) as f32;

                        uidata.prev_spectrogram_sc[i] = uidata.signal_spectrogram_sc[i];
                        uidata.signal_spectrogram_sc[i] =
                            spectrum_sc[i].norm() / (NUM_OF_VIZ_FFT_POINTS / 2) as f32;
                    }

                    self.spectrum_handle.samples_in_buf = 0;
                    self.spectrum_handle
                        .pre_comp_stft_handle
                        .move_input(NUM_OF_VIZ_FFT_POINTS);
                    self.spectrum_handle
                        .post_comp_stft_handle
                        .move_input(NUM_OF_VIZ_FFT_POINTS);
                }
            }
        }

        let mut buf_pre = [0.0_f32; NUM_OF_VIZ_FFT_POINTS];
        let mut buf_post = [0.0_f32; NUM_OF_VIZ_FFT_POINTS];
        let mut buf_sc = [0.0_f32; NUM_OF_VIZ_FFT_POINTS];

        let mut samples_to_copy = 0;
        for (smp_idx, (mut channel_samples, aux_samples)) in buffer
            .iter_samples()
            .zip(aux.inputs[0].iter_samples())
            .enumerate()
        {
            //note that we get rows here, as in each channle samples is only two samples, one for L and one for R
            let mut mid_side = [0.0, 0.0];
            let mut left_right = [0.0, 0.0];
            let mut aux_left_right = [0.0, 0.0];
            //create mid side version (cheaper to do it always)
            // as in mid = L+R, side = L-R
            for (ch, (sample, aux)) in channel_samples.iter_mut().zip(aux_samples).enumerate() {
                left_right[ch] = *sample;
                aux_left_right[ch] = *aux;
            }
            // mid side has double the power
            mid_side[0] = 0.5 * (left_right[0] + left_right[1]);
            mid_side[1] = 0.5 * (left_right[0] - left_right[1]);

            for (ch, sample) in channel_samples.iter_mut().enumerate() {
                //replace the input sample if user selected mid side.
                if self.params.mid_side.value() {
                    *sample = mid_side[ch];
                }

                // we let this run in the sample loop due to the way smoothing works, however, we might need to trick smoother
                // if we don't want to spend a lot of time each sample to reconfigure blocks
                for (idx, comp_filt) in self.comp_filt_state.iter_mut().enumerate() {
                    let this_comp_params = &self.params.comps[idx];
                    // handle bpf update
                    if this_comp_params.center_freq.smoothed.is_smoothing()
                        || this_comp_params.q.smoothed.is_smoothing()
                        || this_comp_params.gain.smoothed.is_smoothing()
                        || (this_comp_params.filtertype.value() != comp_filt[ch].filt.mode)
                    {
                        comp_filt[ch].filt.center_freq =
                            this_comp_params.center_freq.smoothed.next();
                        comp_filt[ch].filt.octaves = this_comp_params.q.smoothed.next();

                        comp_filt[ch].filt.mode = this_comp_params.filtertype.value();

                        comp_filt[ch].filt.configure();
                    }

                    // handle comp update
                    if this_comp_params.attack.smoothed.is_smoothing() {
                        comp_filt[ch]
                            .comp
                            .solver
                            .update_attack(this_comp_params.attack.smoothed.next());
                    }

                    if this_comp_params.release.smoothed.is_smoothing() {
                        comp_filt[ch]
                            .comp
                            .solver
                            .update_release(this_comp_params.release.smoothed.next());
                    }

                    if this_comp_params.ratio.smoothed.is_smoothing() {
                        comp_filt[ch]
                            .comp
                            .solver
                            .update_ratio(this_comp_params.ratio.smoothed.next());
                    }
                    if this_comp_params.threshold.smoothed.is_smoothing() {
                        comp_filt[ch].comp.solver.threshold =
                            gain_to_db_fast(this_comp_params.threshold.smoothed.next());
                    }
                    if this_comp_params.range.smoothed.is_smoothing() {
                        comp_filt[ch].comp.max_reduciton =
                            -gain_to_db_fast(this_comp_params.range.smoothed.next());
                    }

                    //TODO: missing params - knee width, compressor type
                }

                if ch == 0 {
                    buf_pre[smp_idx] = *sample;
                    buf_sc[smp_idx] = aux_left_right[0];
                    samples_to_copy += 1;
                }
                let mut total_crossover = 0.0;
                let mut total_filt_comp = 0.0;
                let mut sc_from_solo = 0.0;
                for i in 0..MAX_MBCS {
                    if self.params.comps[i].enable.value() {
                        let comp_filt = &mut self.comp_filt_state[i];

                        let stereo_mix = self.params.stereo_mix.value() / 2.0;
                        //TODO: missing condtion on if we're using an actual side chain!
                        let sc_input = if self.params.comps[i].sidechain.value() {
                            aux_left_right
                        } else {
                            left_right
                        };

                        let sc = match stereo_mix {
                            0.0 => None,
                            _ => Some(
                                sc_input[ch] * (1.0 - stereo_mix) + sc_input[1 - ch] * stereo_mix,
                            ),
                        };

                        //filter
                        let (filt_main, sc) = comp_filt[ch].filt.process(
                            *sample,
                            sc,
                            self.params.comps[i].slope.value(),
                        );

                        if self.params.solo.value() == i as i32 {
                            sc_from_solo = sc.unwrap_or(filt_main);
                        }

                        //compress
                        let comp = comp_filt[ch].comp.process(filt_main, sc);

                        let gain = self.params.comps[i].gain.smoothed.next();

                        total_crossover += filt_main;
                        total_filt_comp += comp * gain;
                    }
                }

                let total_output = if self.params.solo.value() == -1 {
                    total_filt_comp + *sample - total_crossover
                } else {
                    sc_from_solo
                };

                *sample = total_output;

                //write back the result to the mid-side buffer, we will then use it to override the data
                mid_side[ch] = sample.clamp(-1.5, 1.5); //hard limit to no more than 3.5dB over

                if ch == 0 {
                    buf_post[smp_idx] = mid_side[ch];
                }
            }

            //handle L-R Reconstruction if we processed using mid-side
            for (ch, sample) in channel_samples.iter_mut().enumerate() {
                if self.params.mid_side.value() {
                    if ch == 0 {
                        *sample = mid_side[0] + mid_side[1];
                    } else {
                        *sample = mid_side[0] - mid_side[1];
                    }
                } else {
                    *sample = mid_side[ch];
                }

                let peak_result = self.peak_meter[ch].step(sample.abs());
                self.peak_meter_result[ch].store(peak_result, std::sync::atomic::Ordering::Relaxed);
            }

            //write peak meter value
        }
        if self.spectrum_handle.samples_in_buf < NUM_OF_VIZ_FFT_POINTS {
            if self.params.editor_state.is_open() {
                if self.spectrum_handle.samples_in_buf + samples_to_copy > NUM_OF_VIZ_FFT_POINTS {
                    samples_to_copy = NUM_OF_VIZ_FFT_POINTS - self.spectrum_handle.samples_in_buf;
                }
                self.spectrum_handle.pre_comp_stft_handle.write_input(
                    0,
                    self.spectrum_handle.samples_in_buf,
                    samples_to_copy,
                    &buf_pre,
                );
                self.spectrum_handle.post_comp_stft_handle.write_input(
                    0,
                    self.spectrum_handle.samples_in_buf,
                    samples_to_copy,
                    &buf_post,
                );

                self.spectrum_handle.sc_stft_handle.write_input(
                    0,
                    self.spectrum_handle.samples_in_buf,
                    samples_to_copy,
                    &buf_sc,
                );

                self.spectrum_handle.samples_in_buf += samples_to_copy;
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

nice_export_vst3!(OpenMbc);
