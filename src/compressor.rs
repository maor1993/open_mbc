use nih_plug::prelude::*;

mod models;
mod process;

use process::CompressorSolver;

use crate::compressor::{
    models::{CompressionEmulationEnum, CompressionModel},
    process::CurveType,
};

#[derive(Debug)]
pub struct Compressor {
    bypass: bool,
    curr_reduction: f32,
    makeup_gain_db: f32,
    curve_type: CurveType,
    compressor_model: CompressionEmulationEnum,
    solver: CompressorSolver,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        Compressor {
            bypass: false,
            curr_reduction: 0.0,
            makeup_gain_db: 0.0,
            curve_type: CurveType::default(),
            compressor_model: CompressionEmulationEnum::Ideal(models::IdealCompressor), //TODO: make this better.
            solver: CompressorSolver::new(sample_rate),
        }
    }

    //TODO: inline everything?
    //process sidechain
    fn handle_reduction_calc(&mut self, sidechain_db: f32) -> f32 {
        // step 1: get the ideal reduction needed given the current state of the filter
        let ideal_reduction = self.solver.get_ideal_reduction(sidechain_db);
        println!("ideal_reduction step 1: {}", ideal_reduction);
        //step 2: apply smoothing
        let output_reduction;
        (output_reduction, self.curr_reduction) =
            self.solver
                .apply_curve(self.curr_reduction, ideal_reduction, &self.curve_type);
        println!("ideal_reduction step 2: {}", self.curr_reduction);
        //step 3: apply compressor modeling
        let model_reduciton = self
            .compressor_model
            .get_gain_reduction(output_reduction, ideal_reduction);

        //step 4: filtering
        //TODO

        return model_reduciton;
    }

    pub fn process(&mut self, smp: f32, sidechain: Option<f32>) -> f32 {
        let smp_db = util::gain_to_db_fast(smp.abs());

        if self.bypass {
            return smp;
        }

        let sidechain_db = match sidechain {
            Some(x) => util::db_to_gain_fast(x.abs()),
            None => smp_db,
        };

        let reduction_db = self.handle_reduction_calc(sidechain_db);
        smp * util::db_to_gain_fast(-reduction_db + self.makeup_gain_db)
    }
}

#[cfg(test)]
mod tests {
    use super::Compressor;
    use anyhow::Error;
    use plotters::prelude::*;

    const OUT_FILE_NAME: &str = "tmp/stock.png";
    fn draw_plot(results: Vec<(f32, f32, f32, f32)>) -> Result<(), Error> {
        let root = BitMapBackend::new(OUT_FILE_NAME, (1024, 768)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .right_y_label_area_size(30)
            .build_cartesian_2d(0f32..1024f32, -0.1f32..1f32)?
            .set_secondary_coord(0f32..1024f32, -100.0f32..100.0f32);

        chart.configure_mesh().draw()?;

        chart
            .configure_secondary_axes()
            .y_desc("Reduction(dB)")
            .draw()?;

        chart.draw_series(LineSeries::new(
            results
                .clone()
                .into_iter()
                .map(|(idx, pre, _, _)| (idx, pre)),
            &RED,
        ))?;

        chart.draw_series(LineSeries::new(
            results
                .clone()
                .into_iter()
                .map(|(idx, _, post, _)| (idx, post)),
            &BLUE,
        ))?;

        chart.draw_secondary_series(LineSeries::new(
            results.into_iter().map(|(idx, _, _, comp)| (idx, comp)),
            &GREEN,
        ))?;

        Ok(())
    }

    fn draw_plot_db(results: Vec<(f32, f32, f32, f32)>) -> Result<(), Error> {
        let root = BitMapBackend::new(OUT_FILE_NAME, (1024, 768)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .right_y_label_area_size(30)
            .build_cartesian_2d(0f32..1024f32, -100f32..0f32)?;

        chart.configure_mesh().draw()?;

        chart.draw_series(LineSeries::new(
            results
                .clone()
                .into_iter()
                .map(|(idx, pre, _, _)| (idx, pre)),
            &RED,
        ))?;

        chart.draw_series(LineSeries::new(
            results
                .clone()
                .into_iter()
                .map(|(idx, _, post, _)| (idx, post)),
            &BLUE,
        ))?;

        chart.draw_series(LineSeries::new(
            results.into_iter().map(|(idx, _, _, comp)| (idx, comp)),
            &GREEN,
        ))?;

        Ok(())
    }

    #[test]
    fn run_compressor_ex001() {
        let mut comp = Compressor::new(44100.0);

        comp.solver.threshold = -20.0;
        comp.solver.update_ratio(4.0);
        //comp.solver.update_knee_width(3.0);
        comp.solver.update_attack(0.0);
        comp.solver.update_release(1.0); //TODO: under 1msec starts oscilating 
        
        let mut samples = Vec::<f32>::new();

        for i in 0..250 {
            samples.push(((i as f32) * 0.002).min(0.250));
        }

        for _i in 0..10 {
            samples.push(0.125);
        }
        for _i in 0..10 {
            samples.push(0.250);
        }
        for i in 0..250 {
            samples.push(0.250 - (i as f32) * 0.001)
        }

        let mut results = Vec::<(f32, f32, f32, f32)>::new();
        for (idx, sample) in samples.iter().enumerate() {
            let smp = comp.process(*sample, None);

            results.push((
                idx as f32,
                nih_plug::util::gain_to_db(*sample),
                nih_plug::util::gain_to_db(smp),
                -comp.curr_reduction,
            ));
        }
        draw_plot_db(results).unwrap();
    }
}
