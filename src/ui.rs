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

pub const FREQUENCIES: [f32; NUM_OF_FILTER_POINTS] = {
    let mut arr = [0.0; NUM_OF_FILTER_POINTS];
    let range = FREQ_RANGE_MAX - FREQ_RANGE_MIN;
    let step = range / (NUM_OF_FILTER_POINTS as f32 - 1.0);

    let mut i = 0;
    while i < NUM_OF_FILTER_POINTS {
        arr[i] = FREQ_RANGE_MIN + (i as f32) * step;
        i += 1;
    }
    arr
};

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
            filter_shapes: vec![[0.0_f64; NUM_OF_FILTER_POINTS]; MAX_MBCS * 3],
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

                    // }

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
                        .show_axes([true, true]);
                    // .label_formatter(|_, _| "".to_owned()); // Disable default tooltip
                    plot.show(ui, |plot_ui| {
                        plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                            [FREQ_RANGE_MIN as f64, -60.0],
                            [FREQ_RANGE_MAX as f64, 20.0],
                        ));
                        // Generate line points
                        let slope = (FREQ_RANGE_MAX - FREQ_RANGE_MIN) / NUM_OF_FILTER_POINTS as f32;

                        {
                            let uidata = ui_data.lock().unwrap();
                            for filter_shape in uidata.filter_shapes.iter() {
                                let points: egui_plot::PlotPoints = (0..NUM_OF_FILTER_POINTS)
                                    .map(|i| {
                                        let x = i as f64 * (slope as f64);
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

                    ui.allocate_space(Vec2::splat(2.0));
                    /*                    ui.add(
                        egui::widgets::ProgressBar::new(plotresp.inner.y)
                            .text(peak_meter_text),
                    ); */
                });
        },
    )
}
