use nih_plug_egui::egui::{self, Color32, Rect, Response, Sense, Ui, Vec2, Widget};

pub struct VolumeMeter<'a> {
    value: &'a f32,
    range: std::ops::RangeInclusive<f32>,
    width: f32,
}

impl<'a> VolumeMeter<'a> {
    pub fn new(value: &'a f32, min_db: f32, max_db: f32) -> Self {
        Self {
            value,
            range: min_db..=max_db,
            width: 20.0, // Default width
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl<'a> Widget for VolumeMeter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // 1. Calculate the desired size
        // We take the full available height, or a minimum default if inside a scroll area
        let desired_size = Vec2::new(self.width, ui.available_height());

        // 2. Allocate the space in the UI
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

        // 3. Drawing logic
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Constants for our "steps"
            let step_height = 4.0;
            let gap = 1.0;
            let total_step_size = step_height + gap;

            // Calculate how many steps fit in the height
            let num_steps = (rect.height() / total_step_size).floor() as i32;

            // Normalize the current value to a 0.0 - 1.0 range
            let powerspan = *self.range.end() - *self.range.start();
            let intensity = egui::remap_clamp(*self.value, self.range, 0.0..=1.0);

            let lit_steps = (intensity * num_steps as f32).round() as i32;
            for i in 0..num_steps {
                // Draw from bottom to top
                // i = 0 is the bottom-most rectangle
                let bottom_y = rect.bottom() - (i as f32 * total_step_size);
                let top_y = bottom_y - step_height;

                let step_rect = Rect::from_min_max(
                    egui::pos2(rect.left(), top_y),
                    egui::pos2(rect.right(), bottom_y),
                );

                // Determine color based on whether the step is "lit"
                let color = if i < lit_steps {
                    // Simple color gradient: Green -> Yellow -> Red
                    let progress = i as f32 / num_steps as f32;
                    if progress < ((powerspan - 12.0) / powerspan) {
                        Color32::from_rgb(50, 200, 50) // Green
                    } else if progress < ((powerspan - 6.0) / powerspan) {
                        Color32::from_rgb(220, 220, 50) // Yellow
                    } else {
                        Color32::from_rgb(200, 50, 50) // Red
                    }
                } else {
                    // Unlit color (dark grey/empty)
                    ui.visuals().faint_bg_color
                };

                painter.rect_filled(step_rect, 0.0, color);
            }
            painter.text(
                rect.min,
                egui::Align2::LEFT_TOP,
                format!("{:.1}", self.value.max(-99.0)),
                egui::FontId::proportional(10.0),
                Color32::GOLD,
            );
        }

        response
    }
}
