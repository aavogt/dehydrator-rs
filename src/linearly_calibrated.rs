
use std::{sync::{Arc, Mutex}, error::Error};

use esp_idf_hal::{gpio::ADCPin, adc::Adc, spi::{SpiDeviceDriver, SpiDriver}};
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use hx711_spi::Hx711;
use serde::{Serialize, Deserialize};

use crate::{acs712::ACS172, nvs::{self, EmptyBlob}};
use esp_idf_sys::EspError;
use anyhow::anyhow;

/// allow reading i32 from Hx711 or a u16 from ACS712
///
/// maybe it belongs in acs172.rs or a file imported by it, so that acs172.rs can be more private
pub trait RawRead {
    type X;
    fn raw_read(&mut self) -> anyhow::Result<Self::X>;
}

impl<'d> RawRead for Hx711<SpiDeviceDriver<'d, SpiDriver<'d>>> {
    type X=i32;
    fn raw_read(&mut self) -> anyhow::Result<Self::X> {
        self.read()
            .map_err(move |e| anyhow!("hx711 error: {:?}", e))
    }
}

impl<'d, PIN: ADCPin, ADC: Adc> RawRead for ACS172<'d, PIN, ADC> {
    type X=u16;
    fn raw_read(&mut self) -> anyhow::Result<Self::X> {
        self.driver.read(&mut self.channel)
            .map_err(|e : EspError| anyhow!("adc error: {:?}", e))
    }
}



/// LinearCalibration is used with T=HX711 and T=ACS172
pub struct LinearlyCalibratedSensor<T : RawRead> {
    pub driver : T,
    /// calibration stored in memory
    pub calibration : LinearCalibration<f32, <T as RawRead>::X>,
    /// access calibration stored in flash
    pub nvs : Arc<Mutex<EspNvs<NvsDefault>>>,
    pub name : String,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct LinearCalibration<Y,X> {
    x0 : X,
    x1 : X,
    y0 : Y,
    y1 : Y,
}


pub trait LinearCalibrated<Y,X> {
    fn new() -> Self;
    fn predict(&self, x : X) -> Y;
}

/// TODO very redundant it's not quite clear how to use the num crate
impl LinearCalibrated<f32,u16> for LinearCalibration<f32, u16> {
    fn new() -> Self {
        Self { x0 : 0, x1 : 1, y0 : 0.0, y1 : 1.0 }
    }
    fn predict(&self, x : u16) -> f32 {
        self.y0 + (x - self.x0) as f32 * (self.y1 - self.y0) / (self.x1 - self.x0) as f32
    }
}
impl LinearCalibrated<f32, i32> for LinearCalibration<f32, i32> {
    fn new() -> Self {
        Self { x0 : 0, x1 : 1, y0 : 0.0, y1 : 1.0 }
    }
    fn predict(&self, x : i32) -> f32 {
        self.y0 + (x - self.x0) as f32 * (self.y1 - self.y0) / (self.x1 - self.x0) as f32
    }
}
    
impl<T : RawRead> LinearlyCalibratedSensor<T> where
    LinearCalibration<f32, <T as RawRead>::X> : LinearCalibrated<f32, <T as RawRead>::X>
{
    pub fn read(&mut self) -> anyhow::Result<f32> {
        let x = self.driver.raw_read()?;
        Ok(self.calibration.predict(x))
    }
}

impl<T> LinearlyCalibratedSensor<T>
where T : RawRead,
      for<'a> <T as RawRead>::X: Serialize +  Deserialize<'a>,
      LinearCalibration<f32, <T as RawRead>::X>: LinearCalibrated<f32, <T as RawRead>::X>
{
    /// overwrite one point of the calibration stored in memory
    /// y is the desired output value for the raw x value from the
    /// raw_read call
    pub fn tare_measurement(&mut self, y: f32) -> anyhow::Result<()> {
        let x = self.driver.raw_read()?;

        if y == 0.0 {
            self.calibration.x0 = x;
            self.calibration.y0 = y;
        } else {
            self.calibration.x1 = x;
            self.calibration.y1 = y;
        }
        Ok(())
    }

    pub fn save_calibration(&mut self) -> anyhow::Result<()>
        where <T as RawRead>::X: PartialEq
    {
        let mut nvs = self.nvs.lock().unwrap();
        let writer = nvs::ReadWriteStr(&mut nvs, self.name.as_str());

        if let Ok(saved) = ciborium::de::from_reader(&writer) {
            if self.calibration == saved {
                // skip saving duplicated
                return Ok(());
            };
        };

        ciborium::ser::into_writer(&self.calibration, writer)?;
        Ok(())
    }

    /// load calibration from flash Ok(false) if it was not found
    /// and instead the default calibration was loaded
    pub fn load_calibration(&mut self) -> anyhow::Result<bool> {
        let mut nvs = self.nvs.lock().unwrap();
        let reader = nvs::ReadWriteStr(&mut nvs, self.name.as_str());
        // when the calibration is not found, we use the default calibration
        match ciborium::de::from_reader(reader) {
            Ok(calibration) => { 
                self.calibration = calibration; 
                return Ok(true);},
            Err(ciborium::de::Error::Io(e)) if e.is::<EmptyBlob>() => 
                { self.calibration = LinearCalibration::new();
                  return Ok(false);},
            Err(e) => { return Err(anyhow!("other error {}", e)); },
        }
    }
}
