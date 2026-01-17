use anyhow::Result;
use vergen_git2::{BuildBuilder, CargoBuilder, Emitter, Git2Builder, RustcBuilder, SysinfoBuilder};

pub fn main() -> Result<()> {
    Emitter::default()
        .add_instructions(&Git2Builder::all_git()?)?
        .emit()
}
