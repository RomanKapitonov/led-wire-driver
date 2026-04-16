use std::vec;

use super::{
    EngineError, LedEngine,
    runtime::TransferState,
};
use crate::{
    api::{
        BackendChannelId, ChannelId, PixelLayout, PreparedBinding, PreparedSetup,
        backend::{BackendCapabilities, BackendError, BackendEvent, StartTransfer},
        channel::DriverId,
    },
    model::FrameEpoch,
    test_support::{FakeBackend, FakeBackendHandle},
};

fn test_backend() -> (LedEngine<FakeBackend>, FakeBackendHandle) {
    let (backend, handle) = FakeBackend::new(BackendCapabilities {
        max_channels: 4,
        max_bytes_per_channel: Some(12),
    });
    let mut engine = LedEngine::new(backend);
    engine.init().expect("fake backend init should succeed");
    (engine, handle)
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

fn configure_ready_engine() -> (LedEngine<FakeBackend>, FakeBackendHandle) {
    let (mut engine, handle) = test_backend();
    let plan = engine
        .build_registration_plan(&one_channel_setup(), DriverId::new(1))
        .expect("registration plan should build");
    engine
        .apply_registration_plan(&plan)
        .expect("registration plan should apply");
    engine.enter_ready_state();
    (engine, handle)
}

#[test]
fn registration_apply_commits_records_on_success() {
    let (mut engine, handle) = test_backend();
    let plan = engine
        .build_registration_plan(&one_channel_setup(), DriverId::new(1))
        .expect("registration plan should build");

    engine
        .apply_registration_plan(&plan)
        .expect("registration plan should apply");

    let channel = engine
        .channels
        .record(0, engine.max_channels())
        .expect("channel should be registered after apply");
    assert_eq!(channel.backend_channel, BackendChannelId::new(0));
    assert_eq!(channel.len_pixels, 2);
    assert_eq!(channel.layout, PixelLayout::Grb);
    assert_eq!(handle.log().configured_specs.len(), 1);
}

#[test]
fn registration_apply_failure_does_not_commit_records() {
    let (mut engine, handle) = test_backend();
    let plan = engine
        .build_registration_plan(&one_channel_setup(), DriverId::new(1))
        .expect("registration plan should build");
    handle.script_configure_error(BackendError::InvalidBinding);

    let err = engine
        .apply_registration_plan(&plan)
        .expect_err("scripted configure failure should surface");

    assert_eq!(err, EngineError::Backend(BackendError::InvalidBinding));
    assert_eq!(
        engine.channels.record(0, engine.max_channels()),
        Err(EngineError::ChannelNotRegistered)
    );
}

#[test]
fn dropped_unpublished_write_is_retryable() {
    let (mut engine, _handle) = configure_ready_engine();

    let write = engine
        .acquire_prepared_write(0)
        .expect("first acquire should succeed");
    drop(write);

    engine
        .acquire_prepared_write(0)
        .expect("dropped unpublished lease should abort and become retryable");
}

#[test]
fn publish_failure_aborts_write_and_is_retryable() {
    let (mut engine, handle) = configure_ready_engine();
    handle.script_publish_error(
        BackendChannelId::new(0),
        BackendError::TransportFault { raw_code: 7 },
    );

    let mut write = engine
        .acquire_prepared_write(0)
        .expect("first acquire should succeed");
    let err = write
        .publish()
        .expect_err("scripted publish failure should surface");
    assert_eq!(
        err,
        EngineError::Backend(BackendError::TransportFault { raw_code: 7 })
    );
    drop(write);

    engine
        .acquire_prepared_write(0)
        .expect("failed publish lease should abort on drop and become retryable");
}

#[test]
fn busy_submit_preserves_pending_and_phase_advances_only_on_started() {
    let (mut engine, handle) = configure_ready_engine();

    {
        let mut write = engine
            .acquire_prepared_write(0)
            .expect("acquire should succeed");
        write.publish().expect("publish should succeed");
    }
    engine
        .mark_channel_published(0)
        .expect("mark published should succeed");
    engine.submit_dirty().expect("submit_dirty should succeed");

    handle.script_submit_result(Ok(StartTransfer::Busy));
    engine.service().expect("busy service should succeed");

    let ready = engine.state.ready().expect("engine should be ready");
    assert_eq!(ready.pending_mask.bits(), 1);
    assert_eq!(ready.transfer, TransferState::Idle);
    assert_eq!(
        engine
            .channels
            .record(0, engine.max_channels())
            .expect("channel should stay registered")
            .frame_phase,
        FrameEpoch::ZERO
    );

    handle.script_submit_result(Ok(StartTransfer::Started));
    engine.service().expect("started service should succeed");

    let ready = engine.state.ready().expect("engine should be ready");
    assert_eq!(ready.pending_mask.bits(), 0);
    assert!(matches!(
        ready.transfer,
        TransferState::InFlight {
            dma_complete_pending: false,
            submitted_mask
        } if submitted_mask.bits() == 1
    ));
    assert_eq!(
        engine
            .channels
            .record(0, engine.max_channels())
            .expect("channel should stay registered")
            .frame_phase,
        FrameEpoch::ZERO.wrapping_add(1)
    );
    assert_eq!(handle.log().submit_masks, vec![1, 1]);
}

#[test]
fn transfer_complete_event_clears_inflight_on_service() {
    let (mut engine, handle) = configure_ready_engine();

    {
        let mut write = engine
            .acquire_prepared_write(0)
            .expect("acquire should succeed");
        write.publish().expect("publish should succeed");
    }
    engine
        .mark_channel_published(0)
        .expect("mark published should succeed");
    engine.submit_dirty().expect("submit_dirty should succeed");
    handle.script_submit_result(Ok(StartTransfer::Started));
    engine.service().expect("service should start transfer");

    engine.on_backend_event(BackendEvent::TransferComplete);
    let ready = engine.state.ready().expect("engine should be ready");
    assert!(matches!(
        ready.transfer,
        TransferState::InFlight {
            dma_complete_pending: true,
            submitted_mask
        } if submitted_mask.bits() == 1
    ));

    engine
        .service()
        .expect("service should clear completed transfer");
    let ready = engine.state.ready().expect("engine should stay ready");
    assert_eq!(ready.transfer, TransferState::Idle);
    assert_eq!(handle.log().events, vec![BackendEvent::TransferComplete]);
}

#[test]
fn transfer_complete_while_idle_is_latched_as_ingress_violation() {
    let (mut engine, handle) = configure_ready_engine();

    engine.on_backend_event(BackendEvent::TransferComplete);

    let ready = engine.state.ready().expect("engine should be ready");
    assert_eq!(ready.transfer, TransferState::Idle);
    assert!(ready.ingress_violation);
    assert_eq!(handle.log().events, vec![BackendEvent::TransferComplete]);
}

#[test]
fn latched_ingress_violation_surfaces_once_on_service() {
    let (mut engine, _handle) = configure_ready_engine();

    engine.on_backend_event(BackendEvent::TransferComplete);

    let err = engine
        .service()
        .expect_err("latched idle completion should surface on next runtime call");
    assert_eq!(
        err,
        EngineError::BackendContractViolation(
            super::BackendContractViolation::TransferCompleteWhileIdle
        )
    );

    let ready = engine.state.ready().expect("engine should stay ready");
    assert!(!ready.ingress_violation);

    engine
        .service()
        .expect("latched violation should clear after being surfaced once");
}

#[test]
fn scripted_busy_write_is_reported_without_consuming_retry() {
    let (mut engine, handle) = configure_ready_engine();
    handle.script_acquire_busy(BackendChannelId::new(0));

    let err = engine
        .acquire_prepared_write(0)
        .err()
        .expect("scripted busy acquire should surface");
    assert_eq!(err, EngineError::WriteBusy);

    engine
        .acquire_prepared_write(0)
        .expect("next acquire should succeed after one-shot busy response");
}
