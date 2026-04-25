/// Optional egui GUI module (behind `gui` feature flag).
/// This is a stub — full implementation is planned for a future release.

#[cfg(feature = "gui")]
pub fn launch_gui() -> anyhow::Result<()> {
    tracing::info!("GUI mode is not yet implemented. Please use the web dashboard at localhost:4001.");
    Ok(())
}

#[cfg(not(feature = "gui"))]
pub fn launch_gui() -> anyhow::Result<()> {
    tracing::warn!("GUI feature is not enabled. Build with --features gui to enable.");
    tracing::info!("Using web dashboard at localhost:4001 instead.");
    Ok(())
}
