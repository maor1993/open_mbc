use egui_plot::Plot;
use std::{
    f32, f64,
    sync::{Arc, Mutex},
};

use egui_knob::Knob;

use splines::{self, Key};

use nih_plug::{editor::Editor, log::info, prelude::ParamSetter};
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, widgets::ProgressBar, Vec2},
    resizable_window::ResizableWindow,
    EguiState,
};

use nih_plug::util::MINUS_INFINITY_DB;

use crate::OpenMbcParams;
use crate::MAX_MBCS;

use crate::{FREQ_RANGE_MAX, FREQ_RANGE_MIN};
pub const NUM_OF_FILTER_POINTS: usize = 1024; //used for visualization, might need to interpolate
pub const NUM_OF_VIZ_FFT_POINTS: usize = 2048; // window size of ~20msec

use std::sync::OnceLock;

pub const FREQ_RANGE_MAX_LOG10: f64 = 4.38021124_f64; //manually calculated from 24Khz
pub const FREQ_RANGE_MIN_LOG10: f64 = 1.301029996_f64; //manually calculated from 20Hz

pub const MIN_POWER_DB: isize = -80;
pub const MAX_POWER_DB: isize = 12;

pub const FREQ_STEP: f64 =
    (FREQ_RANGE_MAX_LOG10 - FREQ_RANGE_MIN_LOG10) / (NUM_OF_FILTER_POINTS as f64 - 1.0);

static FREQUENCIES: OnceLock<[f64; NUM_OF_FILTER_POINTS]> = OnceLock::new();

pub fn get_frequencies() -> &'static [f64; NUM_OF_FILTER_POINTS] {
    FREQUENCIES.get_or_init(|| {
        let mut arr = [0.0; NUM_OF_FILTER_POINTS];

        for i in 0..NUM_OF_FILTER_POINTS {
            arr[i] = 10.0f64.powf(FREQ_RANGE_MIN_LOG10 + (i as f64) * FREQ_STEP);
        }
        arr
    })
}

pub mod utils;
pub struct UiData {
    curr_mbc_idx: usize,
    pub sample_rate: f32,
    filter_shapes: Vec<[f64; NUM_OF_FILTER_POINTS]>,
    pub prev_spectrogram: [f32; NUM_OF_VIZ_FFT_POINTS / 2],
    pub signal_spectrogram_pre: [f32; NUM_OF_VIZ_FFT_POINTS / 2],
    pub signal_spectrogram_post: [f32; NUM_OF_VIZ_FFT_POINTS / 2],
    pub gain_reduction: [f32; MAX_MBCS],
}

impl Default for UiData {
    fn default() -> Self {
        Self {
            curr_mbc_idx: 0,
            sample_rate: 0.0,
            filter_shapes: vec![[0.0_f64; NUM_OF_FILTER_POINTS]; (MAX_MBCS * 3) + 1],
            prev_spectrogram: [f32::NEG_INFINITY; NUM_OF_VIZ_FFT_POINTS / 2],
            signal_spectrogram_pre: [f32::NEG_INFINITY; NUM_OF_VIZ_FFT_POINTS / 2],
            signal_spectrogram_post: [f32::NEG_INFINITY; NUM_OF_VIZ_FFT_POINTS / 2],
            gain_reduction: [0.0_f32; MAX_MBCS],
        }
    }
}

impl UiData {
    pub fn set_filter_shape(&mut self, value: f64) {
        for filter in self.filter_shapes.iter_mut() {
            filter.fill(value);
        }
    }

    pub fn add_filter_shape(&mut self) {
        self.filter_shapes.push([0.0_f64; NUM_OF_FILTER_POINTS]);
    }

    pub fn borrow_filter_shape(&mut self, index: usize) -> Option<&mut [f64]> {
        if index < self.filter_shapes.len() {
            return Some(&mut self.filter_shapes[index]);
        } else {
            return None;
        }
    }

    // pub fn add_filter_gain(&mut self,gain: f32){
    //     for mag in self.filter_shape.iter_mut(){
    //         *mag = *mag * gain as f64
    //     }
    //  }
}

macro_rules! update_param {
    ($a:expr,$b:expr,$c:expr) => {
        if $b.value() != $c {
            $a.begin_set_parameter($b);
            $a.set_parameter($b, $c);
            $a.end_set_parameter($b);
        }
    };
}

fn create_state_tooltip(
    ui: &mut egui::Ui,
    params: &Arc<OpenMbcParams>,
    ui_data: &Arc<Mutex<UiData>>,
    setter: &ParamSetter<'_>,
) {
    let last_idx = ui_data.lock().unwrap().curr_mbc_idx;

    let mut idx = last_idx;
    ui.add(egui::widgets::Slider::new(&mut idx, 0..=MAX_MBCS - 1));

    if idx != last_idx {
        ui_data.lock().unwrap().curr_mbc_idx = idx;
    }
    //TODO: currently we're locking and unlocking many times, might need to be more efficient here.
    let curr_gain_reduction = ui_data.lock().unwrap().gain_reduction.clone();

    {
        let mut enable = params.comps[idx].enable.value();
        ui.checkbox(&mut enable, format!("Enable {}", idx));
        update_param!(setter, &params.comps[idx].enable, enable);
    }

    // ui.label(format!("Octaves {}", idx));

    ui.vertical_centered_justified(|ui| {
        ui.horizontal(|ui| {
            ui.add(
                ProgressBar::new((60.0 - curr_gain_reduction[idx]) / 60.0)
                    .show_percentage()
                    .text(format!("reduction: {:.1}dB", curr_gain_reduction[idx]))
                    .desired_width(300.0),
            );
        });
        ui.horizontal(|ui| {
            let mut octaves = params.comps[idx].q.value();
            ui.add(
                Knob::new(&mut octaves, 0.01, 10.0, egui_knob::KnobStyle::Wiper)
                    .with_label("Octaves", egui_knob::LabelPosition::Bottom),
            );
            update_param!(setter, &params.comps[idx].q, octaves);

            let mut freq = params.comps[idx].center_freq.value();
            ui.add(
                Knob::new(
                    &mut freq,
                    FREQ_RANGE_MIN,
                    FREQ_RANGE_MAX,
                    egui_knob::KnobStyle::Wiper,
                )
                .with_label("Center", egui_knob::LabelPosition::Bottom),
            );
            update_param!(setter, &params.comps[idx].center_freq, freq);

            let mut gain = params.comps[idx].gain.value();
            ui.add(
                Knob::new(&mut gain, 0.03, 30.0, egui_knob::KnobStyle::Wiper)
                    .with_label("Gain[dB]", egui_knob::LabelPosition::Bottom)
                    .with_label_format(|val| format!("{:.1}", (20.0_f32 * (val.log10())))),
            );
            update_param!(setter, &params.comps[idx].gain, gain);
        });
        ui.horizontal(|ui| {
            let mut threshold = params.comps[idx].threshold.value();
            ui.add(
                Knob::new(&mut threshold, 1e-3, 1.0, egui_knob::KnobStyle::Wiper)
                    .with_label("Threshold", egui_knob::LabelPosition::Bottom)
                    .with_label_format(|val| format!("{:.1}", (20.0_f32 * (val.log10())))),
            );
            update_param!(setter, &params.comps[idx].threshold, threshold);

            let mut ratio = params.comps[idx].ratio.value();
            ui.add(
                Knob::new(&mut ratio, 1.0, 10.0, egui_knob::KnobStyle::Wiper)
                    .with_label("Ratio", egui_knob::LabelPosition::Bottom),
            );
            update_param!(setter, &params.comps[idx].ratio, ratio);

            let mut sidechain = params.comps[idx].sidechain.value();

            ui.add(egui::widgets::Checkbox::new(&mut sidechain, "sidechain"));

            update_param!(setter, &params.comps[idx].sidechain, sidechain);

            // let mut gain = params.comps[idx].gain.value();
            // ui.add(
            //     Knob::new(&mut gain, 0.03, 30.0, egui_knob::KnobStyle::Wiper)
            //         .with_label("Gain[dB]", egui_knob::LabelPosition::Bottom)
            //         .with_label_format(|val| {
            //             format!("{:.1}", (20.0_f32 * (val.log10())))
            //         }),
            // );
            // update_param!(setter, &params.comps[idx].gain, gain);
        })
    });
}

pub fn build_editor(
    params: Arc<OpenMbcParams>,
    egui_state: Arc<EguiState>,
    ui_data: Arc<Mutex<UiData>>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        egui_state.clone(),
        (),
        Default::default(),
        |_, _, _| {},
        move |egui_ctx, setter, _queue, _state| {
            ResizableWindow::new("res-wind")
                .min_size(Vec2::new(480.0, 480.0))
                .show(egui_ctx, egui_state.as_ref(), |ui| {
                    ui.vertical(|ui| {
                        ui.vertical_centered(|ui| ui.heading("Open Multi Band Compressor"));

                        // create_state_tooltip(ui, &params, &ui_data, setter);
                        let resp = ui.vertical_centered(|ui| {
                            let plot = Plot::new("eq_plot")
                                .height(ui.available_height() - 50.0)
                                .allow_zoom(false)
                                .allow_drag(false)
                                .allow_scroll(false)
                                .show_axes([true, true])
                                .x_grid_spacer(|input| {
                                    let mut marks = Vec::new();

                                    // Calculate the range of powers of 10 visible in the current view
                                    let start_pow = input.bounds.0.floor() as i32;
                                    let end_pow = input.bounds.1.ceil() as i32;

                                    for pow in start_pow..=end_pow {
                                        let base = 10.0f64.powi(pow);

                                        // Major tick (the power of 10)
                                        marks.push(egui_plot::GridMark {
                                            value: pow as f64,
                                            step_size: 1.0, // Used by egui to determine line thickness
                                        });

                                        // Minor ticks (2, 3, 4... 9 * 10^pow)
                                        // Only draw if we aren't zoomed out too far to keep the UI clean
                                        for i in 2..10 {
                                            let val = (i as f64 * base).log10();

                                            let step_size =
                                                if i == 2 || i == 5 { 0.3 } else { 0.1 };

                                            marks.push(egui_plot::GridMark {
                                                value: val,
                                                step_size: step_size, // Thinner lines
                                            });
                                        }
                                    }
                                    marks
                                })
                                .x_axis_formatter(|mark, _range| {
                                    // Convert the log value back to a readable string (e.g., 10, 100, 1000)

                                    let res = 10.0f64.powf(mark.value);

                                    if res >= 1000.0 {
                                        format!("{:.0}k", res / 1000.0)
                                    } else {
                                        format!("{:.0}", res)
                                    }
                                })
                                .label_formatter(|name, value| {
                                    format!(
                                        "freq:{:.0}\nGain:{:.2}",
                                        10.0_f64.powf(value.x),
                                        value.y
                                    )
                                });
                            // .label_formatter(|_, _| "".to_owned()); // Disable default tooltip
                            let resp = plot.show(ui, |plot_ui| {
                                plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                                    [FREQ_RANGE_MIN_LOG10, MIN_POWER_DB as f64],
                                    [FREQ_RANGE_MAX_LOG10, MAX_POWER_DB as f64],
                                ));
                                {
                                    let uidata = ui_data.lock().unwrap();
                                    for filter_shape in uidata.filter_shapes.iter() {
                                        let points: egui_plot::PlotPoints = (0
                                            ..NUM_OF_FILTER_POINTS)
                                            .map(|i| {
                                                let x = get_frequencies()[i].log10();
                                                let y = nih_plug::util::gain_to_db_fast(
                                                    filter_shape[i] as f32,
                                                )
                                                    as f64;
                                                [x, y]
                                            })
                                            .collect();

                                        let line = egui_plot::Line::new("", points)
                                            // .color(egui::Color32::from_rgb(100, 200, 255))
                                            .width(2.0);
                                        plot_ui.line(line);
                                    }

                                    //draw stft graph
                                    //TODO: no need to recalculate this every time, only on sample rate updates
                                    let mut fft_freqs: [f64; NUM_OF_VIZ_FFT_POINTS / 2] =
                                        std::array::from_fn(|i| {
                                            (uidata.sample_rate as f64 * i as f64
                                                / NUM_OF_VIZ_FFT_POINTS as f64)
                                                .log10()
                                        });
                                    fft_freqs[0] = 1.0;

                                    // let points_pre: egui_plot::PlotPoints = (0
                                    //     ..NUM_OF_VIZ_FFT_POINTS / 2)
                                    //     .map(|i| {
                                    //         let x = fft_freqs[i];
                                    //         let y = nih_plug::util::gain_to_db_fast(
                                    //             uidata.signal_spectrogram_pre[i],
                                    //         );

                                    //         [x, y as f64]
                                    //     })
                                    //     .collect();

                                    let points_pre = splines::Spline::from_vec(
                                        (0..NUM_OF_VIZ_FFT_POINTS / 2)
                                            .map(|i| {
                                                let x = fft_freqs[i];
                                                let y = nih_plug::util::gain_to_db_fast(
                                                    uidata.signal_spectrogram_pre[i],
                                                )
                                                    as f64;

                                                Key::new(x, y, splines::Interpolation::Cosine)
                                            })
                                            .collect(),
                                    );
                                    let interp_size = 8;
                                    let points_pre: egui_plot::PlotPoints = (0
                                        ..(NUM_OF_VIZ_FFT_POINTS * interp_size / 2))
                                        .map(|i| {
                                            let x = (uidata.sample_rate as f64 * i as f64
                                                / ((NUM_OF_VIZ_FFT_POINTS * interp_size) as f64))
                                                .log10();

                                            let y = points_pre.clamped_sample(x).unwrap();

                                            [x, y]
                                        })
                                        .collect();

                                    let points_post: egui_plot::PlotPoints = (0
                                        ..NUM_OF_VIZ_FFT_POINTS / 2)
                                        .map(|i| {
                                            let x = fft_freqs[i];
                                            let y = nih_plug::util::gain_to_db_fast(
                                                uidata.signal_spectrogram_post[i],
                                            );

                                            [x, y as f64]
                                        })
                                        .collect();

                                    let line = egui_plot::Line::new("stft_pre", points_pre)
                                        .color(egui::Color32::from_rgb(200, 200, 200))
                                        .width(2.0)
                                        // .fill(MIN_POWER_DB as f32)
                                        .fill_alpha(0.8);
                                    plot_ui.line(line);

                                    let line = egui_plot::Line::new("stft_post", points_post)
                                        .color(egui::Color32::from_rgb(200, 10, 20))
                                        .width(1.0)
                                        // .fill(MIN_POWER_DB as f32)
                                        .fill_alpha(0.8);
                                    plot_ui.line(line);
                                }

                                for i in 0..MAX_MBCS {
                                    if params.comps[i].enable.value() {
                                        let pnt = [
                                            params.comps[i].center_freq.value().log10() as f64,
                                            nih_plug::util::gain_to_db_fast(
                                                params.comps[i].gain.value(),
                                            ) as f64,
                                        ];
                                        let point_size =
                                            if ui_data.lock().unwrap().curr_mbc_idx == i {
                                                8.0
                                            } else {
                                                3.0
                                            };
                                        plot_ui.points(
                                            egui_plot::Points::new(format!("Filter {}", i), pnt)
                                                .radius(point_size)
                                                .filled(true),
                                        );

                                        let filt_min = (params.comps[i].center_freq.value()
                                            / 2.0_f32.powf(params.comps[i].q.value() / 2.0))
                                            as f64;
                                        let filt_max = filt_min
                                            * 2.0_f32.powf(params.comps[i].q.value()) as f64;
                                        let span = egui_plot::Span::new(
                                            format!("filter {}", i),
                                            filt_min.log10()..=filt_max.log10(),
                                        );

                                        plot_ui.span(span);
                                    }
                                }

                                if plot_ui.response().clicked() {
                                    if let Some(mouse_pos) = plot_ui.pointer_coordinate() {
                                        let mut min_dist = f64::MAX;
                                        let mut selected_idx: Option<usize> = None;
                                        for i in 0..MAX_MBCS {
                                            if !params.comps[i].enable.value() {
                                                continue;
                                            }

                                            let dist = (mouse_pos.x
                                                - params.comps[i].center_freq.value().log10()
                                                    as f64)
                                                .abs();

                                            if (dist < min_dist) & (dist < 0.2) {
                                                min_dist = dist;
                                                selected_idx = Some(i);
                                            }
                                        }

                                        {
                                            let mut uidata = ui_data.lock().unwrap();

                                            match selected_idx {
                                                Some(i) => {
                                                    uidata.curr_mbc_idx = i;
                                                }
                                                None => {
                                                    if let Some(idx) = params
                                                        .comps
                                                        .iter()
                                                        .position(|x| !x.enable.value())
                                                    {
                                                        update_param!(
                                                            setter,
                                                            &params.comps[idx].enable,
                                                            true
                                                        );
                                                        update_param!(
                                                            setter,
                                                            &params.comps[idx].center_freq,
                                                            10.0_f32
                                                                .powf(mouse_pos.x as f32)
                                                                .round()
                                                        );

                                                        uidata.curr_mbc_idx = idx;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                let selected_index = ui_data.lock().unwrap().curr_mbc_idx;
                                if plot_ui.response().dragged() {
                                    let drag_delta = plot_ui.response().drag_motion();

                                    let new_center_freq = 10.0_f32.powf(
                                        params.comps[selected_index].center_freq.value().log10()
                                            + drag_delta.x * FREQ_STEP as f32,
                                    );

                                    let new_gain = params.comps[selected_index].gain.value()
                                        - drag_delta.y * 0.01;
                                    // info!("drag delta :{:?} new freq: {} new gain: {}", drag_delta, new_center_freq,new_gain);
                                    update_param!(
                                        setter,
                                        &params.comps[selected_index].center_freq,
                                        new_center_freq
                                    );
                                    update_param!(
                                        setter,
                                        &params.comps[selected_index].gain,
                                        new_gain
                                    );
                                }
                            });
                            resp
                        });

                        let selected_index = ui_data.lock().unwrap().curr_mbc_idx;
                        let old_octaves = params.comps[selected_index].q.value();
                        if resp.inner.response.hovered() {
                            let delta = ui.input(|i| {
                                i.events.iter().find_map(|e| match e {
                                    egui::Event::MouseWheel { delta, .. } => Some(*delta),
                                    _ => None,
                                })
                            });

                            if let Some(delta) = delta {
                                update_param!(
                                    setter,
                                    &params.comps[selected_index].q,
                                    old_octaves + delta.y * 0.1
                                );
                            }
                        }

                        egui::Popup::menu(&resp.inner.response)
                            .id(egui::Id::new("popup"))
                            .align(egui::RectAlign::BOTTOM)
                            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                            .open(params.comps[selected_index].enable.value())
                            .show(|ui| {
                                create_state_tooltip(ui, &params, &ui_data, setter);
                            });

                        ui.vertical_centered(|ui| {
                            ui.horizontal(|ui| {
                                let mut checked = params.mid_side.value();

                                ui.add(egui::widgets::Checkbox::new(&mut checked, "mid-side"));

                                update_param!(setter, &params.mid_side, checked);

                                let mut stereo_mix = params.stereo_mix.value();
                                ui.label("Stereo Mix ");
                                ui.add(
                                    egui::widgets::DragValue::new(&mut stereo_mix)
                                        .range(0.0..=1.0)
                                        .speed(0.01)
                                        .custom_formatter(|val, _| format!("{:.0}%", val * 100.0)),
                                );
                                update_param!(setter, &params.stereo_mix, stereo_mix);
                            })
                        });
                    });
                });
        },
    )
}
