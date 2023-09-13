/// types shared with the frontend. The /build.rs uses this src/json.rs to generate json.ts via typescript_type_def
/// which is used by www/app.ts
use typescript_type_def::TypeDef; // probably should be optional

#[derive(Serialize, Deserialize, TypeDef)]
pub struct Config {
    /// step_times and step_fracs define a piecewise constant function
    /// for the stepper motor position which in turn determines the temperature profile
    ///
    /// Step times is the seconds (since the config was last considered "modified")
    /// at which stepper should move to the new position. The first element should be 0.
    step_times: [i64; 20],

    /// position of the stepper motor as a fraction (0,1) of the full range
    step_fracs: [f32; 20],

    /// the time to wait between measurements in milliseconds
    measurement_period_ms : u32,

    /// smoothing parameter
    n_wavelets: u16,

    /// humidity threshold for dehydrator to be shut down
    w_cut: f32,

    /// system time when the config was last modified (by http). It is subtracted from
    /// libc::time to get step_times
    last_modified: i64,
}

/// Used to deserialize requests like `{"save":[true,false],"y":[3.14,null]}` from app.js
/// That request in particular says that the first sensor's calibration should be changed
/// so that the current measurement is 3.14. The second sensor's calibration is unchanged.
/// Only the first sensor's calibration is saved in flash.
///
/// TODO remove hardcoded 2?
#[derive(Serialize, Deserialize, TypeDef)]
pub struct CalibrationRequest {
    save : [bool;2],
    y : [Option<f32>;2],
}

pub type API = (Config, CalibrationRequest);
