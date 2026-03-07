use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct CalibPoint {
    pub px: f32,
    pub py: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataPoint {
    pub px: f32,
    pub py: f32,
    pub lx: f64,
    pub ly: f64,
    pub group_id: usize,
}

// Math Engine
pub fn recalculate_data(
    calib_pts: &[CalibPoint],
    data_pts: &mut [DataPoint],
    x1_val: &str,
    x2_val: &str,
    y1_val: &str,
    y2_val: &str,
    log_x: bool,
    log_y: bool,
) {
    let x1_lz: f64 = x1_val.parse().unwrap_or(f64::NAN);
    let x2_lz: f64 = x2_val.parse().unwrap_or(f64::NAN);
    let y1_lz: f64 = y1_val.parse().unwrap_or(f64::NAN);
    let y2_lz: f64 = y2_val.parse().unwrap_or(f64::NAN);

    // Default to NaN to ensure bad calibration results in NaN, not wrong data.
    for p in data_pts.iter_mut() {
        p.lx = f64::NAN;
        p.ly = f64::NAN;
    }

    if calib_pts.len() >= 2 {
        let px1 = calib_pts[0].px as f64;
        let px2 = calib_pts[1].px as f64;

        if (px2 - px1).abs() > 1e-6 {
            for p in data_pts.iter_mut() {
                let px = p.px as f64;

                // X Mapping
                if log_x {
                    if x1_lz > 0.0 && x2_lz > 0.0 {
                        let log_x1 = x1_lz.log10();
                        let log_x2 = x2_lz.log10();
                        let log_lx = log_x1 + (px - px1) * (log_x2 - log_x1) / (px2 - px1);
                        p.lx = 10.0_f64.powf(log_lx);
                    }
                } else {
                    p.lx = x1_lz + (px - px1) * (x2_lz - x1_lz) / (px2 - px1);
                }
            }
        }
    }

    if calib_pts.len() >= 4 {
        let py1 = calib_pts[2].py as f64;
        let py2 = calib_pts[3].py as f64;

        if (py2 - py1).abs() > 1e-6 {
            for p in data_pts.iter_mut() {
                let py = p.py as f64;

                // Y Mapping
                if log_y {
                    if y1_lz > 0.0 && y2_lz > 0.0 {
                        let log_y1 = y1_lz.log10();
                        let log_y2 = y2_lz.log10();
                        let log_ly = log_y1 + (py - py1) * (log_y2 - log_y1) / (py2 - py1);
                        p.ly = 10.0_f64.powf(log_ly);
                    }
                } else {
                    p.ly = y1_lz + (py - py1) * (y2_lz - y1_lz) / (py2 - py1);
                }
            }
        }
    }
}
