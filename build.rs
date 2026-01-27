use anyhow::Result;
use vergen_git2::{Emitter, Git2Builder};

pub fn main() -> Result<()> {
    Emitter::default()
        .add_instructions(&Git2Builder::all_git()?)?
        .emit()
}
