use egui_plot::Plot;
use std::sync::{Arc, Mutex};

use nih_plug::editor::Editor;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets, EguiState,
};

use nih_plug::util::MINUS_INFINITY_DB;

use crate::OpenMbcParams;
use crate::MAX_MBCS;

use crate::{FREQ_RANGE_MAX, FREQ_RANGE_MIN};
pub const NUM_OF_FILTER_POINTS: usize = 1000; //used for visualization, might need to interpolate

use std::sync::OnceLock;

static FREQUENCIES: OnceLock<[f64; NUM_OF_FILTER_POINTS]> = OnceLock::new();

pub fn get_frequencies() -> &'static [f64; NUM_OF_FILTER_POINTS] {
    FREQUENCIES.get_or_init(|| {
        let mut arr = [0.0; NUM_OF_FILTER_POINTS];

        // Log-spacing formula: freq = 10^(log10(min) + i * step)
        let log_min = (FREQ_RANGE_MIN as f64).log10();
        let log_max = (FREQ_RANGE_MAX as f64).log10();
        let step = (log_max - log_min) / (NUM_OF_FILTER_POINTS as f64 - 1.0);

        for i in 0..NUM_OF_FILTER_POINTS {
            arr[i] = 10.0f64.powf(log_min + (i as f64) * step);
        }
        arr
    })
}

pub mod utils;
pub struct UiData {
    curr_mbc_idx: usize,
    pub sample_rate: f32,
    filter_shapes: Vec<[f64; NUM_OF_FILTER_POINTS]>,
}

impl Default for UiData {
    fn default() -> Self {
        Self {
            curr_mbc_idx: 0,
            sample_rate: 0.0,
            filter_shapes: vec![[0.0_f64; NUM_OF_FILTER_POINTS]; (MAX_MBCS * 3) + 1],
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
                .min_size(Vec2::new(128.0, 128.0))
                .show(egui_ctx, egui_state.as_ref(), |ui| {
                    ui.heading("Open Multi Band Compressor");

                    let last_idx = ui_data.lock().unwrap().curr_mbc_idx;
                    let mut idx = last_idx;
                    ui.add(egui::widgets::Slider::new(&mut idx, 0..=MAX_MBCS - 1));

                    if idx != last_idx {
                        ui_data.lock().unwrap().curr_mbc_idx = idx;
                    }

                    // for idx in 0..MAX_MBCS{
                    let mut enable_0 = params.comps[idx].enable.value();
                    ui.checkbox(&mut enable_0, format!("Enable {}", idx));

                    if params.comps[idx].enable.value() != enable_0 {
                        setter.begin_set_parameter(&params.comps[idx].enable);
                        setter.set_parameter(&params.comps[idx].enable, enable_0);
                        setter.end_set_parameter(&params.comps[idx].enable);
                    }
                    ui.label(format!("Octaves {}", idx));
                    ui.add(widgets::ParamSlider::for_param(
                        &params.comps[idx].q,
                        setter,
                    ));
                    ui.label(format!("center {}", idx));
                    ui.add(widgets::ParamSlider::for_param(
                        &params.comps[idx].center_freq,
                        setter,
                    ));
                    ui.label(format!("Gain {}", idx));
                    ui.add(widgets::ParamSlider::for_param(
                        &params.comps[idx].gain,
                        setter,
                    ));

                    let peak_meter = -33.0;
                    let peak_meter_text = if peak_meter > MINUS_INFINITY_DB {
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
                                    marks.push(egui_plot::GridMark {
                                        value: val,
                                        step_size: 0.1, // Thinner lines
                                    });
                                }
                            }
                            marks
                        })
                        .x_axis_formatter(|mark, _range| {
                            // Convert the log value back to a readable string (e.g., 10, 100, 1000)
                            format!("{:.0}", 10.0f64.powf(mark.value))
                        })
                        .label_formatter(|name, value| {
                            format!("freq:{:.0}\nGain:{:.2}", 10.0_f64.powf(value.x), value.y)
                        });
                    // .label_formatter(|_, _| "".to_owned()); // Disable default tooltip
                    plot.show(ui, |plot_ui| {
                        plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                            [1.0, -60.0],
                            [(FREQ_RANGE_MAX as f64).log10(), 20.0],
                        ));
                        {
                            let uidata = ui_data.lock().unwrap();
                            for filter_shape in uidata.filter_shapes.iter() {
                                let points: egui_plot::PlotPoints = (0..NUM_OF_FILTER_POINTS)
                                    .map(|i| {
                                        let x = get_frequencies()[i].log10();
                                        let y =
                                            nih_plug::util::gain_to_db_fast(filter_shape[i] as f32)
                                                as f64;
                                        [x, y]
                                    })
                                    .collect();

                                let line = egui_plot::Line::new("", points)
                                    // .color(egui::Color32::from_rgb(100, 200, 255))
                                    .width(2.0);
                                plot_ui.line(line);
                            }
                        }
                        // 2. Cursor Logic: Check proximity to the line
                        if let Some(pointer_pos) = plot_ui.pointer_coordinate() {
                            let curve_y = 0.0;
                            let dist = (pointer_pos.y - curve_y).abs();

                            // Threshold for "being on the line" in plot units
                            // Adjust based on your Y-axis scale (Gain range)
                            if dist < -5.0 {
                                // self.snapped_point = Some([pointer_pos.x, curve_y]);
                                // Draw a highlight point
                                let highlight =
                                    egui_plot::Points::new("", vec![[pointer_pos.x, curve_y]])
                                        .color(egui::Color32::WHITE)
                                        .radius(10.0);
                                plot_ui.points(highlight);
                            } else {
                                // self.snapped_point = None;
                            }
                        }
                    });

                    ui.allocate_space(Vec2::splat(16.0));
                    /*                    ui.add(
                        egui::widgets::ProgressBar::new(plotresp.inner.y)
                            .text(peak_meter_text),
                    ); */
                });
        },
    )
}
