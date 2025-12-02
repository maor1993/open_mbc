use nih_plug::prelude::*;
use open_mbc::OpenMbc;

fn main() {
    nih_export_standalone::<OpenMbc>();
}