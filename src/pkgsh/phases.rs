use scallop::Result;

pub(crate) mod eapi0;

pub(crate) type PhaseFn = fn() -> Result<()>;

pub(crate) fn phase_stub() -> Result<()> {
    Ok(())
}
