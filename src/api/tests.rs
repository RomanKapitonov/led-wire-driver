use super::{
    BackendChannelId, ChannelId, Driver, PixelLayout, PreparedBinding, PreparedSetup,
    RegisterError, Rgb48, RuntimeError,
    backend::{BackendCapabilities, BackendError},
};
use crate::test_support::{FakeBackend, FakeBackendHandle};

fn test_backend() -> (FakeBackend, FakeBackendHandle) {
    FakeBackend::new(BackendCapabilities {
        max_channels: 4,
        max_bytes_per_channel: Some(12),
    })
}

fn one_channel_setup() -> PreparedSetup {
    PreparedSetup::from_bindings([PreparedBinding::new(
        ChannelId::new(0),
        BackendChannelId::new(0),
        2,
        PixelLayout::Grb,
    )])
    .expect("test setup should validate")
}

fn ready_driver() -> (
    Driver<FakeBackend, super::Ready>,
    FakeBackendHandle,
    super::channel::Channel,
) {
    let (backend, handle) = test_backend();
    let mut configuring = Driver::new(backend).expect("driver init should succeed");
    let handles = configuring
        .configure_prepared(&one_channel_setup())
        .expect("configuration should succeed");
    let channel = handles
        .get(ChannelId::new(0))
        .expect("configured handle should exist");
    (configuring.finalize(), handle, channel)
}

#[test]
fn configure_prepared_rejects_empty_setup() {
    let (backend, _handle) = test_backend();
    let mut driver = Driver::new(backend).expect("driver init should succeed");
    let empty = PreparedSetup::from_bindings(core::iter::empty::<PreparedBinding>())
        .expect("empty prepared setup is structurally valid");

    let err = driver
        .configure_prepared(&empty)
        .expect_err("empty setup should be rejected at configuration time");

    assert_eq!(err, RegisterError::EmptyConfiguration);
}

#[test]
fn configure_prepared_is_single_shot() {
    let (backend, _handle) = test_backend();
    let mut driver = Driver::new(backend).expect("driver init should succeed");
    let setup = one_channel_setup();

    driver
        .configure_prepared(&setup)
        .expect("first configuration should succeed");

    let err = driver
        .configure_prepared(&setup)
        .expect_err("second configuration should be rejected");

    assert_eq!(err, RegisterError::AlreadyConfigured);
}

#[test]
fn channel_rejects_handle_from_another_driver() {
    let (driver_a, _handle_a, channel_a) = ready_driver();
    let (mut driver_b, _handle_b, _channel_b) = ready_driver();

    let _ = driver_a;
    let err = driver_b
        .channel(channel_a)
        .err()
        .expect("cross-driver handle should be rejected");

    assert_eq!(err, RuntimeError::InvalidChannel);
}

#[test]
fn runtime_maps_backend_contract_violation_distinctly() {
    let (mut driver, handle, channel) = ready_driver();
    handle.script_acquire_wrong_length(BackendChannelId::new(0), 3);

    let err = driver
        .channel(channel)
        .expect("channel handle should belong to driver")
        .write_rgb48(&[Rgb48 { r: 0, g: 0, b: 0 }; 2])
        .expect_err("wrong target length should surface as backend-contract failure");

    assert_eq!(err, RuntimeError::BackendContract);
}

#[test]
fn runtime_maps_backend_fault_distinctly() {
    let (mut driver, handle, channel) = ready_driver();
    handle.script_acquire_error(
        BackendChannelId::new(0),
        BackendError::TransportFault { raw_code: 9 },
    );

    let err = driver
        .channel(channel)
        .expect("channel handle should belong to driver")
        .write_rgb48(&[Rgb48 { r: 0, g: 0, b: 0 }; 2])
        .expect_err("backend transport fault should surface as backend runtime error");

    assert_eq!(err, RuntimeError::Backend);
}

#[test]
fn idle_transfer_complete_surfaces_backend_contract_once() {
    let (mut driver, _handle, _channel) = ready_driver();

    driver.on_backend_event(super::backend::BackendEvent::TransferComplete);

    let err = driver
        .service()
        .expect_err("idle completion should surface as backend contract violation");
    assert_eq!(err, RuntimeError::BackendContract);

    driver
        .service()
        .expect("latched ingress violation should clear after one surfaced error");
}
