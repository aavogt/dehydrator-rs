
use std::sync::{Arc, Mutex};

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
pub trait ConvertedRead {
    fn read(&mut self) -> anyhow::Result<f32>;
}

impl<'d> ConvertedRead for Hx711<SpiDeviceDriver<'d, SpiDriver<'d>>> {
    fn read(&mut self) -> anyhow::Result<f32> {
        self.read()
            .map_err(move |e| anyhow!("hx711 error: {:?}", e))
            .map(|x| x as f32)
    }
}

impl<'d, PIN: ADCPin, ADC: Adc> ConvertedRead for ACS172<'d, PIN, ADC> {
    fn read(&mut self) -> anyhow::Result<f32> {
        self.driver.read(&mut self.channel)
            .map_err(|e : EspError| anyhow!("adc error: {:?}", e))
            .map(|x| x as f32)
    }
}

pub struct CalibratedSensor<'d> {
    pub driver : Arc<Mutex<Box< dyn ConvertedRead + Send + 'd>>>,
    /// calibration stored in memory
    pub calibration : LinearCalibration,
    /// access calibration stored in flash
    pub nvs : Arc<Mutex<EspNvs<NvsDefault>>>,
    pub name : String,
}

impl <'d>CalibratedSensor<'d>  {
    pub fn new(driver : impl ConvertedRead + Send + 'd, nvs : Arc<Mutex<EspNvs<NvsDefault>>>, name : String) -> Self {
        let b = Box::new(driver) as Box<dyn ConvertedRead + Send>;
        Self {
            driver : Arc::new(Mutex::new(b)),
            calibration : LinearCalibration::new(),
            nvs,
            name,
        }
    }

    pub fn read(&mut self) -> anyhow::Result<f32> {
        let x = self.driver.lock().unwrap().read()?;
        Ok(self.calibration.predict(x))
    }

    /// overwrite one point of the calibration stored in memory
    /// y is the desired output value for the raw x value from the
    /// raw_read call
    ///
    /// strictly speaking it's only "tare" if y is 0.0
    pub fn tare_measurement(&mut self, y: f32) -> anyhow::Result<()> {
        let x = self.driver.lock().unwrap().read()?;

        if y == 0.0 {
            self.calibration.x0 = x;
            self.calibration.y0 = y;
        } else {
            self.calibration.x1 = x;
            self.calibration.y1 = y;
        }
        Ok(())
    }

    pub fn save_calibration(&mut self) -> anyhow::Result<()> {
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

#[derive(Serialize, Deserialize, PartialEq, Clone, Copy)]
pub struct LinearCalibration {
    x0 : f32,
    x1 : f32,
    y0 : f32,
    y1 : f32,
}
impl LinearCalibration {
    fn new() -> Self { Self { x0 : 0., x1 : 1., y0 : 0.0, y1 : 1.0 } }

    fn predict(&self, x : f32) -> f32 {
        self.y0 + (x - self.x0) * (self.y1 - self.y0) / (self.x1 - self.x0)
    }
}
