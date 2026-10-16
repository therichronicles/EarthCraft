//! Nintendo full input reports (0x30/0x31) and read-only SPI calibration replies.
//! Wire format: dekuNukem/Nintendo_Switch_Reverse_Engineering, bluetooth_hid_notes.md.
use earthcraft_core::controller::BUTTON_COUNT;

const STICK_CENTER: u16 = 2048;
const STICK_MAX: u16 = 4095;
const FACTORY_CALIBRATION_ADDRESS: u32 = 0x603d;
const USER_CALIBRATION_ADDRESS: u32 = 0x8010;
const USER_CALIBRATION_MAGIC: [u8; 2] = [0xb2, 0xa1];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Sample {
    pub axes: [f32; 4],
    pub buttons: [bool; BUTTON_COUNT],
}

#[derive(Clone, Copy)]
struct Calibration {
    center: u16,
    below: u16,
    above: u16,
}
impl Default for Calibration {
    fn default() -> Self {
        Self {
            center: STICK_CENTER,
            below: STICK_CENTER,
            above: STICK_MAX - STICK_CENTER,
        }
    }
}
impl Calibration {
    fn sample(self, value: u16) -> f32 {
        let delta = f32::from(value) - f32::from(self.center);
        let extent = if delta < 0.0 { self.below } else { self.above };
        (delta / f32::from(extent)).clamp(-1.0, 1.0)
    }
}

#[derive(Default)]
pub(super) struct Decoder {
    calibration: [Calibration; 4],
    user_calibrated: [bool; 2],
}
impl Decoder {
    pub fn decode(&mut self, report: &[u8]) -> Option<Sample> {
        if report.first() == Some(&0x21) {
            self.calibration_reply(report);
            // A calibration reply alone must not take over a working simple-HID device.
            return None;
        }
        if !matches!(report.first(), Some(0x30 | 0x31)) || report.len() < 13 {
            return None;
        }
        let left = pair(&report[6..9]);
        let right = pair(&report[9..12]);
        let axes = [left[0], left[1], right[0], right[1]];
        // Positional gilrs layout: South/East/North/West, shoulders, triggers,
        // Select/Start, stick clicks, D-pad, Mode. Capture has no gilrs equivalent.
        let bits = [
            (3, 2),
            (3, 3),
            (3, 1),
            (3, 0),
            (5, 6),
            (3, 6),
            (5, 7),
            (3, 7),
            (4, 0),
            (4, 1),
            (4, 3),
            (4, 2),
            (5, 1),
            (5, 0),
            (5, 3),
            (5, 2),
            (4, 4),
        ];
        let mut sample = Sample::default();
        for (index, value) in axes.into_iter().enumerate() {
            sample.axes[index] = self.calibration[index].sample(value);
        }
        for (index, (byte, bit)) in bits.into_iter().enumerate() {
            sample.buttons[index] = report[byte] & (1 << bit) != 0;
        }
        Some(sample)
    }

    fn calibration_reply(&mut self, report: &[u8]) {
        // Reply header: ACK at 13, subcommand at 14, address at 15, count at 19.
        if report.len() < 20 || report[13] & 0x80 == 0 || report[14] != 0x10 {
            return;
        }
        let address = u32::from_le_bytes(report[15..19].try_into().unwrap());
        let Some(data) = report.get(20..20 + usize::from(report[19])) else {
            return;
        };
        match address {
            FACTORY_CALIBRATION_ADDRESS if data.len() == 18 => {
                for stick in 0..2 {
                    if !self.user_calibrated[stick] {
                        self.stick_calibration(stick, &data[stick * 9..stick * 9 + 9]);
                    }
                }
            }
            USER_CALIBRATION_ADDRESS if data.len() == 22 => {
                for stick in 0..2 {
                    let offset = stick * 11;
                    if data[offset..offset + 2] == USER_CALIBRATION_MAGIC {
                        self.user_calibrated[stick] =
                            self.stick_calibration(stick, &data[offset + 2..offset + 11]);
                    }
                }
            }
            _ => {}
        }
    }

    fn stick_calibration(&mut self, stick: usize, data: &[u8]) -> bool {
        let values = [pair(&data[..3]), pair(&data[3..6]), pair(&data[6..9])];
        let (above, center, below) = if stick == 0 {
            (values[0], values[1], values[2])
        } else {
            (values[2], values[0], values[1])
        };
        if (0..2).any(|axis| {
            center[axis] == 0
                || center[axis] >= STICK_MAX
                || below[axis] == 0
                || above[axis] == 0
                || below[axis] > center[axis]
                || above[axis] > STICK_MAX - center[axis]
        }) {
            return false;
        }
        for axis in 0..2 {
            self.calibration[stick * 2 + axis] = Calibration {
                center: center[axis],
                below: below[axis],
                above: above[axis],
            };
        }
        true
    }
}

fn pair(bytes: &[u8]) -> [u16; 2] {
    [
        u16::from(bytes[0]) | (u16::from(bytes[1] & 0x0f) << 8),
        u16::from(bytes[1] >> 4) | (u16::from(bytes[2]) << 4),
    ]
}

/// Read-only requests; never write controller calibration or change report modes.
pub(super) fn calibration_requests() -> [[u8; 49]; 2] {
    [
        (FACTORY_CALIBRATION_ADDRESS, 18),
        (USER_CALIBRATION_ADDRESS, 22),
    ]
    .map(|(address, length)| {
        let mut request = [0; 49];
        request[0] = 0x01;
        request[2..10].copy_from_slice(&[0, 1, 0x40, 0x40, 0, 1, 0x40, 0x40]);
        request[10] = 0x10;
        request[11..15].copy_from_slice(&address.to_le_bytes());
        request[15] = length;
        request
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Header captured from the physical Pro Controller; timer/IMU omitted.
    const NEUTRAL: [u8; 13] = [
        0x30, 0xe6, 0x80, 0, 0, 0, 0, 0xf8, 0x7f, 0, 0xf8, 0x7f, 0x0c,
    ];

    #[test]
    fn physical_report_decodes_neutral_sticks_and_released_triggers() {
        let sample = Decoder::default().decode(&NEUTRAL).unwrap();
        assert!(sample.axes.iter().all(|axis| axis.abs() < 0.01));
        assert_eq!(sample.buttons, [false; BUTTON_COUNT]);
    }

    #[test]
    fn independent_sticks_and_positional_buttons_decode_from_full_reports() {
        let report = [
            0x30, 0, 0, 0x84, 0x0a, 0x82, 0xff, 0x0f, 0, 0, 0xf0, 0xff, 0,
        ];
        let sample = Decoder::default().decode(&report).unwrap();
        assert_eq!(sample.axes, [1.0, -1.0, -1.0, 1.0]);
        for index in [0, 6, 7, 9, 10, 12] {
            assert!(sample.buttons[index]);
        }
        assert!(!sample.buttons[1]);
        assert!(!sample.buttons[11]);
    }

    #[test]
    fn ignores_other_reports_and_all_truncated_prefixes() {
        for length in 0..NEUTRAL.len() {
            assert!(Decoder::default().decode(&NEUTRAL[..length]).is_none());
        }
        let mut other = NEUTRAL;
        other[0] = 0x3f;
        assert!(Decoder::default().decode(&other).is_none());
        other[0] = 0x21;
        assert!(Decoder::default().decode(&other).is_none());
        other[0] = 0x31;
        assert!(Decoder::default().decode(&other).is_some());
    }

    #[test]
    fn calibration_uses_asymmetric_extents_and_rejects_invalid_flash() {
        let mut decoder = Decoder::default();
        // Left: extent +1024, center 2048, extent -512 on both axes.
        assert!(decoder.stick_calibration(0, &[0, 4, 0x40, 0, 8, 0x80, 0, 2, 0x20]));
        let mut report = NEUTRAL;
        report[6..9].copy_from_slice(&[0, 0x0a, 0x70]); // x=2560, y=1792
        assert_eq!(decoder.decode(&report).unwrap().axes[..2], [0.5, -0.5]);
        assert!(!decoder.stick_calibration(0, &[0xff; 9]));
        assert_eq!(decoder.decode(&report).unwrap().axes[..2], [0.5, -0.5]);
    }

    #[test]
    fn spi_replies_prefer_valid_user_calibration_and_bound_the_payload() {
        let mut decoder = Decoder::default();
        let mut factory = vec![0x21; 20];
        factory[13..20].copy_from_slice(&[0x90, 0x10, 0x3d, 0x60, 0, 0, 18]);
        factory.extend_from_slice(&[0, 4, 0x40, 0, 8, 0x80, 0, 2, 0x20]);
        factory.extend_from_slice(&[0, 8, 0x80, 0, 2, 0x20, 0, 4, 0x40]);
        decoder.decode(&factory);
        let mut report = NEUTRAL;
        report[6..12].copy_from_slice(&[0, 0x0a, 0x70, 0, 0x0a, 0x70]);
        assert_eq!(
            decoder.decode(&report).unwrap().axes,
            [0.5, -0.5, 0.5, -0.5]
        );

        let mut user = vec![0x21; 20];
        user[13..20].copy_from_slice(&[0x90, 0x10, 0x10, 0x80, 0, 0, 22]);
        // Left user calibration has different extents; missing right magic keeps factory.
        user.extend_from_slice(&[0xb2, 0xa1, 0, 2, 0x20, 0, 8, 0x80, 0, 4, 0x40]);
        user.extend_from_slice(&[0xff; 11]);
        decoder.decode(&user);
        decoder.decode(&factory); // Late factory reply cannot overwrite valid user values.
        assert_eq!(
            decoder.decode(&report).unwrap().axes,
            [1.0, -0.25, 0.5, -0.5]
        );
        user[19] = u8::MAX;
        decoder.decode(&user); // Truncated/oversized SPI reply cannot change calibration.
        assert_eq!(
            decoder.decode(&report).unwrap().axes,
            [1.0, -0.25, 0.5, -0.5]
        );
    }
}
