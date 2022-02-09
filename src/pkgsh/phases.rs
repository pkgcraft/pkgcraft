use scallop::Result;

pub(crate) type PhaseFn = fn() -> Result<()>;

pub(crate) fn phase_stub() -> Result<()> {
    Ok(())
}
