use zbus::zvariant::{serialized::Context, to_bytes, Type, LE};

use super::{PopupAdmissionView, PopupDeliveryStage};

#[test]
fn popup_admission_wire_values_remain_stable_and_complete() {
    for (admission, expected) in [
        (PopupAdmissionView::Show, 0_u8),
        (PopupAdmissionView::Rule, 1),
        (PopupAdmissionView::Dnd, 2),
        (PopupAdmissionView::Inhibitor, 3),
        (PopupAdmissionView::RendererUnavailable, 4),
        (PopupAdmissionView::RendererDisabled, 5),
    ] {
        let encoded = to_bytes(Context::new_dbus(LE, 0), &admission)
            .expect("popup admission should serialize");

        assert_eq!(encoded.bytes(), &[expected]);
    }

    assert_eq!(PopupAdmissionView::signature(), u8::signature());
}

#[test]
fn popup_delivery_stage_wire_values_remain_stable_and_complete() {
    for (stage, expected) in [
        (PopupDeliveryStage::Suppressed, 0_u8),
        (PopupDeliveryStage::Admitted, 1),
        (PopupDeliveryStage::FanoutFailed, 2),
        (PopupDeliveryStage::RendererFetched, 3),
        (PopupDeliveryStage::Materialized, 4),
        (PopupDeliveryStage::Visible, 5),
    ] {
        let encoded = to_bytes(Context::new_dbus(LE, 0), &stage)
            .expect("popup delivery stage should serialize");

        assert_eq!(encoded.bytes(), &[expected]);
    }

    assert_eq!(PopupDeliveryStage::signature(), u8::signature());
}

#[test]
fn only_show_admission_permits_popup_rendering() {
    assert!(PopupAdmissionView::Show.should_show());

    for admission in [
        PopupAdmissionView::Rule,
        PopupAdmissionView::Dnd,
        PopupAdmissionView::Inhibitor,
        PopupAdmissionView::RendererUnavailable,
        PopupAdmissionView::RendererDisabled,
    ] {
        assert!(
            !admission.should_show(),
            "{admission:?} should keep the popup hidden",
        );
    }
}
