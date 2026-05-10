use super::utils::update_param;
use egui_knob::Knob;
use nih_plug::{
    params::{FloatParam, Param},
    prelude::ParamSetter,
    util::{db_to_gain_fast, gain_to_db_fast},
};
use egui;

pub type Format = Option<fn(f32) -> String>;

pub fn build_knob(
    ui: &mut egui::Ui,
    param: &FloatParam,
    setter: &ParamSetter<'_>,
    name: &'static str,
    is_db: bool,
    step: Option<f32>,
    index_changed: bool,
    format: Option<impl Fn(f32) -> String + 'static>,
) {
    let mut val = param.value();

    let mut limits = match param.range() {
        nih_plug::prelude::FloatRange::Linear { min, max } => (min, max),
        nih_plug::prelude::FloatRange::Skewed { min, max, .. } => (min, max),
        _ => unreachable!(),
    };

    if is_db {
        val = gain_to_db_fast(val);
        limits = (gain_to_db_fast(limits.0), gain_to_db_fast(limits.1));
    }

    let mut knob = Knob::new(&mut val, limits.0, limits.1, egui_knob::KnobStyle::Wiper)
        .with_double_click_reset(param.default_plain_value())
        .with_middle_scroll()
        .with_label(name, egui_knob::LabelPosition::Bottom)
        .with_step(step);

    if let Some(format) = format {
        knob = knob.with_label_format(format);
    }

    let resp = ui.add(knob);

    if !index_changed {
        let resp = egui::Popup::menu(&resp)
            .id(format!("value_popup_{}", name).into())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                let resp = ui.add(
                    egui::widgets::DragValue::new(&mut val)
                        .range(limits.0..=limits.1)
                        .update_while_editing(false),
                );
                resp.request_focus();
            });

        if let Some(_) = resp {
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
                egui::containers::Popup::close_id(ui.ctx(), format!("value_popup_{}", name).into());
            }
        }
    }

    if is_db {
        val = db_to_gain_fast(val);
    }

    update_param!(setter, param, val);
}
