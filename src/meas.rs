use serde::{Deserialize, Serialize};

/// number of measurements saved in ram to be compressed
/// and saved in a single blob
pub const N1 : usize = 100;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Meas<T> { 
    pub time : i64,
    /// number of times inside absolute humidity is below the cutoff in this blob
    pub cutoffs : i32,
    pub inside_temp : T,
    pub outside_temp : T,
    pub inside_rh : T,
    pub outside_rh : T,
    pub grams : T,
    pub amps : T,
}

impl Meas<[f32;N1]> {
    pub fn new() -> Self {
        Meas {
            time : 0,
            cutoffs : 0,
            inside_temp : [0.0;N1],
            outside_temp : [0.0;N1],
            inside_rh : [0.0;N1],
            outside_rh : [0.0;N1],
            grams : [0.0;N1],
            amps : [0.0;N1],
        }
    }
}

pub fn compress(x : Meas<[f32; N1]>) -> Meas<Vec<u8>> {
        Meas {
            time : x.time,
            cutoffs : x.cutoffs,
            inside_temp : q_compress::auto_compress(&x.inside_temp, 8),
            outside_temp : q_compress::auto_compress(&x.outside_temp, 8),
            inside_rh : q_compress::auto_compress(&x.inside_rh, 8),
            outside_rh : q_compress::auto_compress(&x.outside_rh, 8),
            grams : q_compress::auto_compress(&x.grams, 8),
            amps : q_compress::auto_compress(&x.amps, 8),
        }
    }

pub fn decompress(x : Meas<Vec<u8>>) -> Meas<Vec<f32>> {
        Meas {
            time : x.time,
            cutoffs : x.cutoffs,
            inside_temp : q_compress::auto_decompress(&x.inside_temp).unwrap(),
            outside_temp : q_compress::auto_decompress(&x.outside_temp).unwrap(),
            inside_rh : q_compress::auto_decompress(&x.inside_rh).unwrap(),
            outside_rh : q_compress::auto_decompress(&x.outside_rh).unwrap(),
            grams : q_compress::auto_decompress(&x.grams).unwrap(),
            amps : q_compress::auto_decompress(&x.amps).unwrap(),
        }
}
