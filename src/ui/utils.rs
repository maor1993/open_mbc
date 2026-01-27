use crate::crossover::{Crossover, FilterSlope};

use super::get_frequencies;
use super::NUM_OF_FILTER_POINTS;

use num_complex::Complex64;

pub fn get_filter_shape(
    sample_rate: f32,
    filter: &Crossover,
    slope: FilterSlope,
    gain: f64,
) -> [Complex64; NUM_OF_FILTER_POINTS] {
    //TODO: perhaps save normalized frequency array as well, to avoid running division for every freq
    let mut res = [Complex64::new(0.0, 0.0); NUM_OF_FILTER_POINTS];

    for (idx, freq) in get_frequencies().iter().enumerate() {
        res[idx] = gain * filter.get_main_filter_response(freq / sample_rate as f64, &slope);
    }

    res
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
pub(crate) use update_param;
