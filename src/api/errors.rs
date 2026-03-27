#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// The setup or backend binding shape is invalid for this driver/backend.
    InvalidBinding,
    /// Two bindings resolved to the same logical channel during registration.
    DuplicateChannel,
    /// The caller attempted to configure with an empty prepared setup.
    EmptyConfiguration,
    /// The driver already committed one configuration; registration is
    /// single-shot.
    AlreadyConfigured,
    /// Backend configuration failed for a backend-owned reason.
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DriverInitError {
    /// Backend initialization failed before the driver entered registration.
    Backend,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// The backend cannot currently hand out a writable target.
    Busy,
    /// The supplied channel handle is invalid for this driver or channel.
    InvalidChannel,
    /// The supplied source slice length does not match the configured channel.
    LengthMismatch,
    /// The backend violated a runtime contract expected by the driver.
    BackendContract,
    /// A genuine backend-owned runtime failure occurred.
    Backend,
}
