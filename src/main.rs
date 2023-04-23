#![feature(trait_alias)]
#![feature(step_trait)]
#![feature(generic_arg_infer)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]


use embedded_hal::blocking::i2c::{WriteRead, Write};
use embedded_svc::http::{Method, server::{Response, Request}};
use esp_idf_hal::{prelude::Peripherals, units::Hertz, i2c::{self, I2cDriver, I2c}, gpio::{AnyIOPin, InputPin, OutputPin, PinDriver, IOPin}, peripheral::Peripheral, delay::FreeRtos, adc::{AdcDriver, AdcChannelDriver, Atten0dB}, spi::SpiDeviceDriver};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::{NvsCustom, EspNvs, EspNvsPartition}, http::server::EspHttpServer};
use esp_idf_sys::{self as _, EspError}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use std::{ops::{DerefMut, Deref}, sync::{Mutex, Arc}, time::UNIX_EPOCH, thread, ptr::null_mut};

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

use on_both::OnBoth;
use meas::Meas;
use ir::IrShutdown;


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


#[derive(Serialize, Deserialize)]
struct Config {
    /// step_times and step_fracs define a piecewise constant function
    /// for the stepper motor position
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



fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();

    let peripherals = Peripherals::take().context("expected peripherals")?;
    let sysloop = EspSystemEventLoop::take()?;
    let _ = wifi::connect(peripherals.modem, &sysloop)?;


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

    let ir_shutdown = IrShutdown::new(peripherals.pins.gpio13.into(),
                                    peripherals.rmt.channel0)?;

    // TODO tare and calibrate
    let mut hx711 = {
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

    // TODO calibrate
    let mut acs712 = {
        let conf = Default::default();
        let mut driver = AdcDriver::new(peripherals.adc1, &conf)?;
        let mut channel : AdcChannelDriver<_, Atten0dB<_>> = AdcChannelDriver::new(peripherals.pins.gpio0)?;
        Ok(move || driver.read(&mut channel)
                .map_err(|e : EspError| anyhow!("adc error: {:?}", e))
           ) .map_err(|e : EspError| anyhow!("adc init error: {:?}", e))
    }?;

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

    // stepper thread sets this, the http thread reads it to decide
    // whether to reset last_modified
    let step_times_past_index = Arc::new(Mutex::new(0usize));

    // flash storage for compressed sensor data
    let measured_partition = EspNvsPartition::<NvsCustom>::take("measured")?;
    let comp = Arc::new(Mutex::new(EspNvs::new(measured_partition, "comp",true)?));

    let mut http = EspHttpServer::new(&Default::default())?;

    // remove redundancy?
    // serve www/index.html included in the binary
    http.fn_handler("/", Method::Get, move |rq| {
        let mut rsp = rq.into_ok_response()?;
        let file = include_bytes!("../www/index.html");
        rsp.write(file)?;
        Ok(())
    })?;

    // serve www/app.js included in the binary
    http.fn_handler("/app.js", Method::Get, move |rq| {
        let mut rsp = rq.into_ok_response()?;
        let file = include_bytes!("../www/app.js");
        rsp.write(file)?;
        Ok(())
    })?;

    http.fn_handler("/shutdown", Method::Get, move |_rq| {
        ir_shutdown();
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
    let config1 = config.clone();
    http.fn_handler("/config", Method::Post, move |rq| {
        let mut config = config1.lock().unwrap();
        let mut read_conf : Config = serde_json::from_reader(ReadWrapper(rq))?;

        let i_min = step_times_past_index.lock().unwrap(); // +1?
        if config.step_times[..i_min] == read_conf.step_times[..i_min] &&
            config.step_fracs[..imin] == read_conf.step_times[..i_min] {
            i_min = 0;
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
             // maybe I should use the other interface?
             let mut comp1 = comp1.lock().unwrap();
             let bk = nvs::ReadWrite(
                        comp1.deref_mut(),
                        &mut j);
             let b : Meas<Vec<u8>> = ciborium::de::from_reader(bk)?;
             // call the decompress method
             let b = b.decompress();
             // or use a csv writing library?
             for i in 0..b.inside_temp.len() {
                 embedded_svc::io::Write::write_fmt(&mut rsp, format_args!("{},{},{},{},{},{},{},{},{}\n",
                               j.to_str(),
                               i,
                               b.time, // time is per blob. could be interpolated using i but then
                                       // we need the time from the previous blob
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
            let i_min = step_times_past_index.lock().unwrap();
            if i_min == 0 {
                stepper.lock().unwrap().set_fraction(config.step_fracs[0]).unwrap();
                i_min += 1;
            }
            t -= last_modified;
            for i in i_min..config.step_times.len() {
                if config.step_times[i] > t {
                    stepper.lock().unwrap().set_fraction(config.step_fracs[i]).unwrap();
                    // remove second unwrap somehow?
                    i_min = (i+1).min(config.step_times.len());
                    break;
                }
            }
        };
    });
    
    // make measurements and save to nvs
    loop {
        j.next();
        
        // get N1 measurements
        for i in 0..meas::N1 {
            let (inside,outside) = shts(SHT31::read)?;
            let g = hx711.read()
                    .map_err(|e| anyhow::anyhow!("spi error: {:?}", e))?;
            let amps = acs712()?;
            FreeRtos::delay_ms(config.lock().unwrap().measurement_period_ms);

            // copy into Meas
            meas.inside_temp[i] = inside.temperature;
            meas.outside_temp[i] = outside.temperature;
            meas.inside_rh[i] = inside.humidity;
            meas.outside_rh[i] = outside.humidity;
            // confirm conversion
            meas.amps[i] = amps as f32;
            meas.grams[i] = g as f32;
        }
        unsafe { esp_idf_sys::time(&mut meas.time) };
        // now meas is full

        let mut comp = comp.lock().unwrap();
        let writer = nvs::ReadWrite(comp.deref_mut(), &mut j);

        // write the compressed meas into the nvs
        ciborium::ser::into_writer(&meas.compress(), writer)?;
    }

}
