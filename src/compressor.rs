use nih_plug::prelude::*;

mod models;
mod process;

use process::CompressorSolver;

use crate::compressor::{models::CompressionEmulationEnum, process::CurveType};

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
        //println!("ideal_reduction step 1: {}", ideal_reduction);
        //step 2: apply smoothing
        let output_reduction;
        (output_reduction, self.curr_reduction) =
            self.solver
                .apply_curve(self.curr_reduction, ideal_reduction, &self.curve_type);
        //println!("ideal_reduction step 2: {}", self.curr_reduction);
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
            Some(x) => util::gain_to_db_fast(x.abs()),
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
    use wavers::Wav;

    #[derive(Default)]
    struct ResultSample {
        idx: usize,
        input: f32,
        output: f32,
        reduciton: f32,
    }

    #[derive(Default)]
    struct Results {
        filename: &'static str,
        samples: Vec<ResultSample>,
    }
    impl Results {
        fn draw_plot(&self) -> Result<(), Error> {
            let root = BitMapBackend::new(self.filename, (1024, 768)).into_drawing_area();
            root.fill(&WHITE)?;

            let mut chart = ChartBuilder::on(&root)
                .margin(5)
                .x_label_area_size(30)
                .y_label_area_size(30)
                .right_y_label_area_size(30)
                .build_cartesian_2d(0f32..self.samples.len() as f32, -0.1f32..1f32)?
                .set_secondary_coord(0f32..self.samples.len() as f32, -100f32..0f32);

            chart.configure_mesh().draw()?;

            chart
                .configure_secondary_axes()
                .y_desc("Reduction(dB)")
                .draw()?;

            chart.draw_series(LineSeries::new(
                self.samples.iter().map(|x| (x.idx as f32, x.input)),
                &RED,
            ))?;

            chart.draw_series(LineSeries::new(
                self.samples.iter().map(|x| (x.idx as f32, x.output)),
                &BLUE,
            ))?;

            chart.draw_secondary_series(LineSeries::new(
                self.samples.iter().map(|x| (x.idx as f32, x.reduciton)),
                &GREEN,
            ))?;

            Ok(())
        }

        fn draw_plot_db(&self) -> Result<(), Error> {
            let root = BitMapBackend::new(self.filename, (1024, 768)).into_drawing_area();
            root.fill(&WHITE)?;

            let mut chart = ChartBuilder::on(&root)
                .margin(5)
                .x_label_area_size(30)
                .y_label_area_size(30)
                .right_y_label_area_size(30)
                .build_cartesian_2d(0f32..self.samples.len() as f32, -100f32..0f32)?;

            chart.configure_mesh().draw()?;

            chart.draw_series(LineSeries::new(
                self.samples
                    .iter()
                    .map(|x| (x.idx as f32, nih_plug::util::gain_to_db(x.input.abs()))),
                &RED,
            ))?;

            chart.draw_series(LineSeries::new(
                self.samples
                    .iter()
                    .map(|x| (x.idx as f32, nih_plug::util::gain_to_db(x.output.abs()))),
                &BLUE,
            ))?;

            chart.draw_series(LineSeries::new(
                self.samples.iter().map(|x| (x.idx as f32, x.reduciton)),
                &GREEN,
            ))?;

            Ok(())
        }
    }

    fn get_wave_stream(samplepath: &str) -> Vec<f32> {
        let mut wav: Wav<f32> = Wav::from_path(samplepath).unwrap();

        let samples: &[f32] = &wav.read().unwrap();

        samples.to_vec()
    }

    fn gen_test_ramp() -> Vec<f32> {
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

        samples
    }

    //test1 - sanity with a ramp
    //test2 - kick example
    //test3 - long note example
    //test4 - sidechain
    //test5 - models

    #[test]
    fn run_compressor_ex001() {
        let mut comp = Compressor::new(44100.0);

        comp.solver.threshold = -22.0;
        comp.solver.update_ratio(12.0);
        comp.curve_type = super::process::CurveType::LogSmoothBranching;
        //comp.solver.update_knee_width(3.0);
        comp.solver.update_attack(5.0);
        comp.solver.update_release(100.0); //TODO: under 1msec starts oscilating

        let samples = gen_test_ramp();

        let mut testresults = Results {
            filename: "tmp/ex001.png",
            ..Default::default()
        };

        for (idx, sample) in samples.iter().enumerate() {
            let smp = comp.process(*sample, None);

            testresults.samples.push(ResultSample {
                idx,
                input: *sample,
                output: smp,
                reduciton: -comp.curr_reduction,
            });
        }
        testresults.draw_plot().unwrap();
    }

    #[test]
    fn run_compressor_ex002() {
        let mut comp = Compressor::new(44100.0);

        comp.solver.threshold = -55.0;
        comp.solver.update_ratio(4.0);
        comp.curve_type = super::process::CurveType::LogSmoothBranching;
        //comp.solver.update_knee_width(3.0);
        comp.solver.update_attack(5.0);
        comp.solver.update_release(10.0);

        let samples = get_wave_stream("testfiles/good-kick-single-hit-a-key-135-Hqj.wav");

        let mut testresults = Results {
            filename: "tmp/ex002.png",
            ..Default::default()
        };

        for (idx, sample) in samples.iter().enumerate() {
            let smp = comp.process(*sample, None);

            testresults.samples.push(ResultSample {
                idx,
                input: *sample,
                output: smp,
                reduciton: -comp.curr_reduction,
            });
        }
        testresults.draw_plot_db().unwrap();
    }

    #[test]
    fn run_compressor_ex003() {
        let mut comp = Compressor::new(44100.0);

        comp.solver.threshold = -55.0;
        comp.solver.update_ratio(4.0);
        comp.curve_type = super::process::CurveType::LogSmoothBranching;
        //comp.solver.update_knee_width(3.0);
        comp.solver.update_attack(5.0);
        comp.solver.update_release(10.0);

        let samples = get_wave_stream("testfiles/tambourine_studio_short_loop_2_cF2.wav");

        let mut testresults = Results {
            filename: "tmp/ex003.png",
            ..Default::default()
        };

        for (idx, sample) in samples.iter().enumerate() {
            let smp = comp.process(*sample, None);

            testresults.samples.push(ResultSample {
                idx,
                input: *sample,
                output: smp,
                reduciton: -comp.curr_reduction,
            });
        }
        testresults.draw_plot_db().unwrap();
    }

    #[test]
    fn run_compressor_ex004() {
        let mut comp = Compressor::new(44100.0);

        comp.solver.threshold = -55.0;
        comp.solver.update_ratio(12.0);
        comp.curve_type = super::process::CurveType::LogSmoothBranching;
        //comp.solver.update_knee_width(3.0);
        comp.solver.update_attack(5.0);
        comp.solver.update_release(10.0);

        let samples_main = get_wave_stream("testfiles/tambourine_studio_short_loop_2_cF2.wav");
        let mut samples_sidechain =
            get_wave_stream("testfiles/good-kick-single-hit-a-key-135-Hqj.wav");

        samples_sidechain = samples_sidechain.repeat(10);

        let mut testresults = Results {
            filename: "tmp/ex004.png",
            ..Default::default()
        };

        for (idx, (sample, sidechain)) in
            samples_main.into_iter().zip(samples_sidechain).enumerate()
        {
            let smp = comp.process(sample, Some(sidechain));

            testresults.samples.push(ResultSample {
                idx,
                input: sample,
                output: smp,
                reduciton: -comp.curr_reduction,
            });
        }
        testresults.draw_plot().unwrap();
    }
}
