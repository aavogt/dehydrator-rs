#![feature(trait_alias)]
#![feature(step_trait)]
#![feature(generic_arg_infer)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]


use acs712::ACS172;
use embedded_hal::blocking::i2c::{WriteRead, Write};
use embedded_svc::http::{Method, server::{Response, Request}};
use esp_idf_hal::{prelude::Peripherals, units::Hertz, i2c::{self, I2cDriver, I2c}, gpio::{AnyIOPin, InputPin, OutputPin, PinDriver, IOPin}, peripheral::Peripheral, delay::FreeRtos, spi::SpiDeviceDriver};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::{NvsCustom, EspNvs, EspNvsPartition, EspDefaultNvsPartition}, http::server::EspHttpServer};
use esp_idf_sys::{self as _};
use linearly_calibrated::CalibratedSensor;

// If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use std::{ops::{DerefMut, Deref}, sync::{Mutex, Arc}, thread, ptr::null_mut};

use anyhow::{Context, anyhow};
use shared_bus::{I2cProxy, NullMutex, BusManager, BusMutex};
use sht31::{SHT31, prelude::{Periodic, MPS, Sht31Measure, Sht31Reader}, DeviceAddr::*};
use serde::{Serialize,Deserialize};
use hx711_spi::{self, Hx711};

/// connects to wifi
mod wifi;

/// essid and psk for wifi
mod secrets;

/// stepper motor driver
mod stepper;

/// a function to call both SHT sensors in one go
mod on_both;

/// flashes an infrared led to signal shutdown
mod ir;

/// find and manipulate keys for flash storage
mod nvs;

/// compress/decompress measurements
mod meas;

/// ACS712 current sensor
mod acs712;

/// wrapper for hx711 and acs712 to make and apply calibrations
mod linearly_calibrated;


use on_both::OnBoth;
use meas::Meas;
use ir::IrShutdown;

include!("json.rs");


fn mk_i2c_bus<'d>(i2c : impl Peripheral<P=impl I2c> + 'd,
              sda : impl Peripheral<P=impl InputPin + OutputPin> + 'd,
              scl : impl Peripheral<P=impl InputPin + OutputPin> + 'd)
    -> anyhow::Result<BusManager<NullMutex<I2cDriver<'d>>>> {
    let i2c_config = i2c::config::Config::new()
                    .baudrate(Hertz(100_000));
    let i2c_driver = I2cDriver::new(i2c,
                                    sda, scl, &i2c_config)?;
    Ok(shared_bus::BusManagerSimple::new(i2c_driver))
}

/// create two SHT31 drivers on the same bus with different addresses
fn mk_shts<T : BusMutex>(i2c_bus : &BusManager<T>) -> OnBoth<SHT31<Periodic, I2cProxy<'_, T>>> where
    <T as BusMutex>::Bus : WriteRead + Write {
    let mut sht1 = SHT31::new(i2c_bus.acquire_i2c())
        .with_mode(Periodic::new().with_mps(MPS::Normal))
        .with_address(AD0);
    // or should sht2 = sht1.clone().with_address()?
    let mut sht2 = SHT31::new(i2c_bus.acquire_i2c())
        .with_mode(Periodic::new().with_mps(MPS::Normal))
        .with_address(AD1);
    sht1.set_unit(sht31::TemperatureUnit::Celsius);
    sht2.set_unit(sht31::TemperatureUnit::Celsius);
    OnBoth(sht1, sht2)
}




/// the closure returns true if the hall sensor GPIO is high
fn mk_hall<'d> (pin : impl Peripheral<P=impl InputPin > + 'd) -> anyhow::Result<Box<impl Fn() -> bool + 'd>> {
    let hall = PinDriver::input(pin)?;
    // set pullup / down?
    Ok(Box::new(move || hall.is_high()))
}


/// implement std::io::Write in terms of embedded_svc::io::blocking::Write
/// for serde_json
struct WriteWrapper<'d, 'a> (Response<&'d mut esp_idf_svc::http::server::EspHttpConnection<'a>>);

impl std::io::Write for WriteWrapper<'_, '_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

/// implement std::io::Read in terms of embedded_svc::io::blocking::Read for serde_json
struct ReadWrapper<'d, 'a> (Request<&'d mut esp_idf_svc::http::server::EspHttpConnection<'a>>);
impl std::io::Read for ReadWrapper<'_, '_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}


impl CalibrationRequest {
    fn apply(&self, calib : &mut [CalibratedSensor]) -> anyhow::Result<()>{
        for (i, &save) in self.save.iter().enumerate() {
            if let Some(y) = self.y[i] {
                calib[i].tare_measurement(y)?;
            }
            if save {
                calib[i].save_calibration()?;
            }
        }
        Ok(())
    }

}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();

    let peripherals = Peripherals::take().context("expected peripherals")?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take().unwrap();
    let _ = wifi::connect(peripherals.modem, &sysloop, nvs.clone())?;


    // pins are assigned below
    let i2c_bus = mk_i2c_bus(peripherals.i2c0,
                       peripherals.pins.gpio6,
                       peripherals.pins.gpio7)?;
    let mut shts = mk_shts(&i2c_bus);

    let _activate_shts = shts(SHT31::measure)?;

    let stepper = Arc::new(Mutex::new({
        let step = stepper::HalfStep::init(peripherals.pins.gpio8,
                           peripherals.pins.gpio9,
                           peripherals.pins.gpio10,
                           peripherals.pins.gpio5)?;
        let hall1 = mk_hall(peripherals.pins.gpio3)?;
        let hall2 = mk_hall(peripherals.pins.gpio4)?;
        let hall3 = mk_hall(peripherals.pins.gpio11)?;

        stepper::calibrate(step, hall1, hall2, hall3)?
    }));

    let ir_shutdown = IrShutdown::new(peripherals.pins.gpio13,
                                    peripherals.rmt.channel0)?;

    let hx711_raw = {
        let mosi = peripherals.pins.gpio14.downgrade();
        let miso = peripherals.pins.gpio19.downgrade();
        let sclk = peripherals.pins.gpio18.downgrade();
        let nocs : Option<AnyIOPin> = None;
        let mut config : esp_idf_hal::spi::config::Config = Default::default(); 
        config.baudrate = Hertz(1_000_000); // override others?

        Hx711::new(SpiDeviceDriver::new_single(
                    peripherals.spi2, mosi, miso, Some(sclk),
                                        esp_idf_hal::spi::Dma::Disabled,
                                        nocs,
                                        &config)?)
    };

    let acs712_raw = ACS172::new( peripherals.pins.gpio0, peripherals.adc1)?;
    // pins are assigned above

    // should this instead be a record of Arc<Mutex<>> to keep
    // threads more independent?
    let config = Arc::new(Mutex::new(Config { 
        step_times : [0;20],
        step_fracs : [0.0;20],
        measurement_period_ms : 2000,
        n_wavelets: 40,
        w_cut: 12.0,
        last_modified: unsafe { esp_idf_sys::time(null_mut()) },
    }));

    // step_index_completed is for getting how far the stepper has moved
    // into the http thread. And it is for the http thread to reset the stepper
    // when the user requests it directly or indirectly (by changing the
    // temperature profile).
    // Assuming the stepper thread has run, this is true:
    // > let i = step_index_completed.lock().unwrap();
    // > time >= config.step_times[i]
    // > stepper.set_fraction(config.lock().unwrap().step_fracs[i]) // stepper doesn't move
    let step_index_completed = Arc::new(Mutex::new(0usize));

    // flash storage for compressed sensor data
    let measured_partition = EspNvsPartition::<NvsCustom>::take("measured")?;
    let comp = Arc::new(Mutex::new(EspNvs::new(measured_partition, "comp",true)?));
    let calib = Arc::new(Mutex::new(EspNvs::new(nvs, "calib", true)?));

    let calibrated_sensors = Arc::new(Mutex::new([
        CalibratedSensor::new(acs712_raw,
            calib.clone(),
            "ACS712".to_string()),
        CalibratedSensor::new(hx711_raw,
            calib,
            "HX711".to_string()),
    ]));

    let mut http = EspHttpServer::new(&Default::default())?;


    // set/save calibration
    let calibrated_sensors1 = calibrated_sensors.clone();
    http.fn_handler("/calib", Method::Post, move |rq| {
        let calib_rq : CalibrationRequest = serde_json::from_reader(ReadWrapper(rq))?;
        let mut cs = calibrated_sensors1.lock().unwrap();
        calib_rq.apply(&mut *cs)?;
        Ok(())
    })?;

    // report the current calibrations
    let calibrated_sensors1 = calibrated_sensors.clone();
    http.fn_handler("/calib", Method::Get, move |rq| {
        let cs = calibrated_sensors1.lock().unwrap();
        // TODO figure out cs.map(|x| x.calibration);
        let calibs = [ cs[0].calibration, cs[1].calibration];
        serde_json::to_writer(WriteWrapper(rq.into_ok_response()?),
                    &calibs)?;
        Ok(())
    })?;

    // remove redundancy?
    // serve www/index.html included in the binary
    http.fn_handler("/", Method::Get, move |rq| {
        let mut rsp = rq.into_ok_response()?;
        let file = include_bytes!("../www/index.html");
        rsp.write(file)?;
        Ok(())
    })?;

    // serve www/app.js included in the binary
    http.fn_handler("/app.ts", Method::Get, move |rq| {
        let mut rsp = rq.into_ok_response()?;
        let file = include_bytes!("../www/app.ts");
        rsp.write(file)?;
        Ok(())
    })?;

    http.fn_handler("/shutdown", Method::Post, move |_rq| {
        ir_shutdown();
        Ok(())
    })?;

    let i_min = step_index_completed.clone();
    http.fn_handler("/restart", Method::Post, move |_rq| {
        *i_min.lock().unwrap() = 0;
        Ok(())
    })?;

    // get config
    let config1 = config.clone();
    http.fn_handler("/config", Method::Get, move |rq| {
        let conf = config1.lock().unwrap();
        let rsp = rq.into_ok_response()?;
        serde_json::to_writer(WriteWrapper(rsp), conf.deref())?;
        Ok(())
    })?;

    // set config
    let i_min = step_index_completed.clone();
    let config1 = config.clone();
    http.fn_handler("/config", Method::Post, move |rq| {
        let mut config = config1.lock().unwrap();
        let mut read_conf : Config = serde_json::from_reader(ReadWrapper(rq))?;

        let mut i_min = i_min.lock().unwrap();
        if config.step_times[..*i_min] == read_conf.step_times[..*i_min] &&
            config.step_fracs[..*i_min] == read_conf.step_fracs[..*i_min] {
            *i_min = 0;
            unsafe { esp_idf_sys::time(&mut read_conf.last_modified) };
        };

        *config = read_conf;
        Ok(())
    })?;

    // get measurement
    let comp1 = comp.clone();
    http.fn_handler("/measurement.csv", Method::Get, move |rq| {
         let mut rsp = rq.into_ok_response()?;
         let j0 = nvs::Key::get_first_comp();
         let j_n = nvs::Key::get_last_comp();

         // header
         embedded_svc::io::Write::write_fmt(&mut rsp, format_args!("j,i,time,i_T,i_RH,o_I,o_RH,amps,grams\n"))?;

         // body
         for mut j in j0 ..= j_n {
             let mut comp1 = comp1.lock().unwrap();
             let b = meas::decompress(
                      ciborium::de::from_reader(
                          nvs::ReadWrite(comp1.deref_mut(),
                            &mut j))?);
             // or use a csv writing library?
             for i in 0..b.inside_temp.len() {
                 embedded_svc::io::Write::write_fmt(&mut rsp, format_args!("{},{},{},{},{},{},{},{},{}\n",
                               j.to_str(),
                               i,
                               b.time, // time is per blob. could be interpolated using i but then
                                       // we need the time from the previous blob or otherwise
                                       // assume a constant time step
                               b.inside_temp[i],
                               b.inside_rh[i],
                               b.outside_temp[i],
                               b.outside_rh[i],
                               b.amps[i],
                               b.grams[i]))?;
             }

         }
         Ok(())
    })?;


    let mut meas = Meas::new();

    let mut j = nvs::Key::get_last_comp();

    // moves the stepper following the piecewise constant function
    // specified by step_fracs and step_times
    let config1 = config.clone();
    thread::spawn(move || {
        loop {

            FreeRtos::delay_ms(1000); // configurable?
            // get the current time since boot up in seconds
            let mut t = unsafe { esp_idf_sys::time(null_mut()) };

            let config = config1.lock().unwrap();
            let mut i_min = step_index_completed.lock().unwrap();
            if *i_min == 0 {
                stepper.lock().unwrap().set_fraction(config.step_fracs[0]).unwrap();
                *i_min += 1;
            }
            t -= config.last_modified;
            for i in *i_min..config.step_times.len() {
                if config.step_times[i] > t {
                    stepper.lock().unwrap().set_fraction(config.step_fracs[i]).unwrap();
                    // remove second unwrap somehow?
                    *i_min = (i+1).min(config.step_times.len());
                    break;
                }
            }
        };
    });
    
    // make measurements and save to nvs
    loop {
        j.next();
        
        meas.cutoffs = 0;
        // get N1 measurements
        for i in 0..meas::N1 {
            let (inside,outside) = shts(SHT31::read)?;
            FreeRtos::delay_ms(config.lock().unwrap().measurement_period_ms);


            // copy into Meas
            meas.inside_temp[i] = inside.temperature;
            meas.outside_temp[i] = outside.temperature;
            meas.inside_rh[i] = inside.humidity;
            meas.outside_rh[i] = outside.humidity;
            {
                let mut calib = calibrated_sensors.lock().unwrap();
                meas.amps[i] = calib[0].read()?;
                meas.grams[i] = calib[1].read()?;
            }
            let w = abs_humidity_g_per_m3(inside.temperature, inside.humidity);
            if w < config.lock().unwrap().w_cut { meas.cutoffs += 1; }
        }
        unsafe { esp_idf_sys::time(&mut meas.time) };
        // now meas is full

        let mut comp = comp.lock().unwrap();
        let writer = nvs::ReadWrite(comp.deref_mut(), &mut j);

        // write the compressed meas into the nvs
        ciborium::ser::into_writer(&meas::compress(meas), writer)?;
    }

}

/// absolute humidity in g/m3 according to
/// <https://webbook.nist.gov/cgi/cbook.cgi?ID=C7732185&Mask=4&Type=ANTOINE&Plot=on#ANTOINE>
/// temp should be between -17 and 100°C, rh_percent is 0 to 100
fn abs_humidity_g_per_m3(temp_celsius : f32, rh_percent : f32) -> f32 {
    // Stull 1947 antione equation for water vapor pressure from NIST
    const A : f32 = 4.6543;
    const B : f32 = 1435.264;
    const C : f32 = -64.848;
    const MW : f32 = 18.01528; // g/mol
    const R : f32 = 8.31446261815324; // J/(mol K)
    const PA_PER_BAR : f32 = 1e5;

    let kelvin = temp_celsius + 273.15;
    let bar_pw = 10f32.powf(A - B / (kelvin + C)) * rh_percent / 100.0;
    let p = PA_PER_BAR * bar_pw;

    // ideal gas law MW n/V = MW p/R/T
    MW * p / kelvin / R
}
