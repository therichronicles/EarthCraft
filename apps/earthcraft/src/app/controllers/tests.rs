use super::*;
use earthcraft_core::controller::ControllerProfile;
#[test]
fn neutral_gate_ignores_unbound_controls_and_requires_bound_controls_released() {
    let mut state = ControllerState::default();
    let mut profile = ControllerProfile::default();
    assert!(neutral(&state, &profile));
    state.axes[3] = 1.0;
    assert!(!neutral(&state, &profile));
    profile.axes[5].input = None;
    assert!(neutral(&state, &profile));
    state.buttons[18] = true;
    assert!(neutral(&state, &profile));
    state.buttons[0] = true;
    assert!(!neutral(&state, &profile));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a physical Pro Controller; move both sticks and press/release a button"]
fn nintendo_hardware_input() {
    let mut controllers = Controllers::new();
    let mut prefs = ControllerPreferences {
        enabled: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut moved = [false; 4];
    let mut button_seen = false;
    let mut motion_seen = false;
    let mut last_report = Instant::now();
    while Instant::now() < deadline {
        let (motion, _) = controllers.poll(&ctx, &mut prefs, true);
        motion_seen |= motion.is_active();
        if let Some(device) = controllers
            .devices
            .iter()
            .find(|device| device.key == prefs.selected)
        {
            for (seen, value) in moved.iter_mut().zip(device.state.axes) {
                *seen |= value.abs() > 0.5;
            }
            button_seen |= device.state.buttons.iter().any(|down| *down);
            if last_report.elapsed() >= Duration::from_secs(1) {
                eprintln!(
                    "axes={:?} moved={moved:?} button={button_seen} motion={motion_seen} error={:?}",
                    device.state.axes, controllers.error
                );
                last_report = Instant::now();
            }
            if moved.iter().all(|seen| *seen)
                && button_seen
                && motion_seen
                && neutral(&device.state, &prefs.profile(&device.key))
            {
                return;
            }
        }
        std::thread::sleep(ACTIVE_POLL_INTERVAL);
    }
    panic!(
        "Controller verification incomplete: axes={moved:?}, button={button_seen}, motion={motion_seen}, error={:?}",
        controllers.error
    );
}
