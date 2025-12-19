use cute_dsp::filters::FilterType;
use egui_plot::Plot;
use nih_plug::prelude::*;
use std::sync::Arc;

mod compressor;

use compressor::Compressor;

mod crossover;
use crossover::Crossover;

use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets, EguiState,
};

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

pub struct OpenMbc {
    params: Arc<OpenMbcParams>,
    sample_rate: f32,
    comp_filt_state: [CompFilter; MAX_MBCS],
}

#[derive(Params)]
struct OpenMbcParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

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


        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            Default::default(),
            |_, _, _| {},
            move |egui_ctx, setter, _queue, _state| {
                // Generate x values
                let xs: Vec<f64> = (FREQ_RANGE_MIN as usize..FREQ_RANGE_MAX as usize)
                    .map(|x| x as f64)
                    .collect();
                
                // Generate sin(x) and bounds
                let ys: Vec<f64> = xs.iter().map(|&x| 0.0).collect();
                // Create the center line
                let sin_line = egui_plot::Line::new(
                    "sin(x)",
                    xs.iter()
                        .zip(ys.iter())
                        .map(|(&x, &y)| [x, y])
                        .collect::<egui_plot::PlotPoints<'_>>(),
                )
                .width(2.0).color(egui::Color32::from_rgb(200, 100, 100));

                ResizableWindow::new("res-wind")
                    .min_size(Vec2::new(128.0, 128.0))
                    .show(egui_ctx, egui_state.as_ref(), |ui| {
                        // This is a fancy widget that can get all the information it needs to properly
                        // display and modify the parameter from the parametr itself
                        // It's not yet fully implemented, as the text is missing.
                        
                        ui.label("Some random integer");
                        ui.add(widgets::ParamSlider::for_param(&params.comps[0].q, setter));

                        ui.label("Gain");
                        ui.add(widgets::ParamSlider::for_param(&params.comps[0].gain, setter));

                        ui.label(
                        "Also gain, but with a standard widget. Note that it doesn't properly take the parameter curve into account!",
                        );

                        // This is a simple naive version of a parameter slider that's not aware of how
                        // the parameters work
                        let prev_value = nih_plug::util::gain_to_db(params.comps[0].gain.value());
                        let mut new_value = prev_value;
                        ui.add(
                            egui::widgets::Slider::new(&mut new_value, -30.0..=30.0).suffix(" dB"),
                        );
                        if new_value != prev_value {
                            setter.begin_set_parameter(&params.comps[0].gain);
                            setter
                                .set_parameter(&params.comps[0].gain, nih_plug::util::db_to_gain(new_value));
                            setter.end_set_parameter(&params.comps[0].gain);
                        }

                        // TODO: Add a proper custom widget instead of reusing a progress bar
                        // let peak_meter =
                        //     util::gain_to_db(peak_meter.load(std::sync::atomic::Ordering::Relaxed));
                        let peak_meter = -33.0;
                        let peak_meter_text = if peak_meter > util::MINUS_INFINITY_DB {
                            format!("{peak_meter:.1} dBFS")
                        } else {
                            String::from("-inf dBFS")
                        }; 
                        
                        let peak_meter_normalized = (peak_meter + 60.0) / 60.0;


                                    let plot = Plot::new("eq_plot")
                .height(300.0)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .show_axes([true, true])
                .label_formatter(|_, _| "".to_owned()); // Disable default tooltip

                        plot.show(ui, |plot_ui| {
                            // Generate line points
                            let n = 512;
                            let points: egui_plot::PlotPoints = (0..n)
                                .map(|i| {
                                    let x = 20.0 * (20000.0 / 20.0f64).powf(i as f64 / n as f64);
                                    let y = 0.0;
                                    [x.ln(), y] // Using log scale for X axis visualization
                                })
                                .collect();

                            let line = egui_plot::Line::new("",points).color(egui::Color32::from_rgb(100, 200, 255)).width(2.0);
                            plot_ui.line(line);

                            // 2. Cursor Logic: Check proximity to the line
                            if let Some(pointer_pos) = plot_ui.pointer_coordinate() {
                                let curve_y = 0.0;
                                let dist = (pointer_pos.y - curve_y).abs();

                                // Threshold for "being on the line" in plot units
                                // Adjust based on your Y-axis scale (Gain range)
                                if dist < 0.5 {
                                    // self.snapped_point = Some([pointer_pos.x, curve_y]);
                                    
                                    // Draw a highlight point
                                    let highlight = egui_plot::Points::new("",vec![[pointer_pos.x, curve_y]])
                                        .color(egui::Color32::WHITE)
                                        .radius(4.0);
                                    plot_ui.points(highlight);

                                    // Using the modern tooltip API
                                    // egui::Tooltip::always_open(ui.ctx(), parent_layer, parent_widget, anchor)



                                    // egui::show_tooltip_at_pointer(ui.ctx(), egui::Id::new("eq_tooltip"), "",|ui| {
                                    //     ui.label(format!("Freq: {:.0} Hz", pointer_pos.x.exp()));
                                    //     ui.label(format!("Gain: {:.1} dB", curve_y));
                                    // });
                                } else {
                                    // self.snapped_point = None;
                                }
                            }
                        });

                        ui.allocate_space(Vec2::splat(2.0));
     /*                    ui.add(
                            egui::widgets::ProgressBar::new(plotresp.inner.y)
                                .text(peak_meter_text),
                        ); */
                        
                    });
            },
        )
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut tot_enabled_chs = 1; //small kombina, we start with one as we always have the aux channel
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
                        if self.params.comps[idx].enable.value() {
                            comp * self.params.comps[idx].gain.smoothed.next()
                                * (1.0 / tot_enabled_chs as f32)
                                + filt_aux * (1.0 / tot_enabled_chs as f32)
                        } else {
                            filt_aux * (1.0 / MAX_MBCS as f32)
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
