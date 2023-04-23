use esp_idf_hal::{gpio::{OutputPin, PinDriver, AnyOutputPin, self}, delay::FreeRtos};


trait Step = FnMut(Dir) -> anyhow::Result<()>;

pub struct Stepper<'d> {
    pub min : i32,
    pub max : i32,
    pub pos : i32,
    step : HalfStep<'d>,
    delay_ms : u32,
}

impl<'d> Stepper<'d> {
    fn delay(&self) {
        FreeRtos::delay_ms(self.delay_ms);
    }

    /// with delay
    pub fn fwd(&mut self) -> anyhow::Result<bool> {
        self.pos += 1;
        if self.pos < self.max {
            self.delay();
            self.step.activate(Dir::CC)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// with delay
    pub fn rev(&mut self) -> anyhow::Result<bool> {
        self.pos -= 1;
        if self.pos > self.min {
            self.delay();
            self.step.activate(Dir::CW)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn off(&mut self) -> anyhow::Result<()> {
        self.delay();
        self.step.activate(Dir::Off)
    }

    pub fn set_fraction(&mut self, f : f32) -> anyhow::Result<()> {
        let pos = (self.min as f32) + (f * ((self.max - self.min) as f32));
        self.set_pos(pos as i32)
    }

    pub fn set_pos(&mut self, pos : i32) -> anyhow::Result<()> {
        if pos < self.min || pos > self.max {
            anyhow::bail!("position out of range");
        }
        while self.pos < pos {
            self.fwd()?;
        }
        while self.pos > pos {
            self.rev()?;
        }
        self.delay();
        Ok(())
    }
}

pub enum Dir { CC, CW, Off }



/// calibrate the stepper relative to the magnet
/// it is also possible to calibrate it relative to measured temperatures,
/// but the limits are unknown...
/// find the max and min positions
/// it is only required that the sensor orientations
/// be consistent: when near the magnet
/// they must either all have high or low levels it does
/// not matter which in particular.
pub fn calibrate<'d>(step : HalfStep<'d>,
                         at_min : impl Fn() -> bool,
                         at_mid : impl Fn() -> bool,
                         at_max : impl Fn() -> bool) -> anyhow::Result<Stepper<'d>> {

    let b1 = at_min();
    let b2 = at_mid();
    let b3 = at_max();
    let majority = (b1 as u8) + (b2 as u8) + (b3 as u8);
    let correct = |b : bool| {
        match majority {
            0 | 1 => b ,
            _ => !b,
    }};

    let mut s = Stepper {
        min : 0,
        max : 0,
        pos : 0,
        step,
        delay_ms : 20,
    };

    loop {
        s.pos += 1;
        s.delay();
        s.step.activate(Dir::CC)?;
        if correct(at_max()) {
            s.max = s.pos;
            break;
        }
        if correct(at_min()) {
            s.min = s.pos;
            break;
        }
    }

    loop {
        s.pos -= 1;
        s.delay();
        s.step.activate(Dir::CW)?;
        if correct(at_min()) {
            s.min = s.pos;
            break;
        };
        if correct(at_max()) {
            s.max = s.pos;
            break;
        }
    }


    Ok(s)
}


pub struct HalfStep<'d> {
    pps : [PinDriver<'d, AnyOutputPin, gpio::Output>; 4],
    /// For each half step i, the lowest 4 bits of pulse[i] specify
    /// the level of the 4 coils energized by pps.
    /// Copied fromt the table at <https://vanhunteradams.com/Pico/Steppers/Lorenz.html>
    pulse : [u8; 8],
}

/// The 28byj-48 is controlled with 4 gpio pins. The returned lambda
/// executes a single half-step in the direction given by the bool. 512 calls should be a full rotation
/// according to <https://projecthub.arduino.cc/debanshudas23/1620bd1e-3463-4fb0-9c1c-53d03bb1a433>
/// so a full rotation would be 512*20ms = 10.24s. This is fast enough since the dial is only 1/3
/// of a rotation.
impl<'d> HalfStep<'d> {
    pub fn init(a : impl OutputPin, b : impl OutputPin, c : impl OutputPin, d : impl OutputPin)
            -> anyhow::Result<Self> {
        let pps : [PinDriver<'_, AnyOutputPin, gpio::Output>; 4] =
            [ PinDriver::output(a.downgrade_output())?,
              PinDriver::output(b.downgrade_output())?,
              PinDriver::output(c.downgrade_output())?,
              PinDriver::output(d.downgrade_output())? ];
        let pulse = [ 0x9,0x8,0xc,0x4,0x6,0x2,0x3,0x1 ];
        Ok(HalfStep { pps, pulse })
    }

    pub fn activate(&mut self, dir : Dir) -> anyhow::Result<()> {
        static mut i : i8 = 0;
        use Dir::*;
        unsafe {
            match dir {
                CC => i+=1,
                CW => i-=1,
                Off => (),
            };
            i = i.rem_euclid(8);
            let p : u8 = self.pulse[i as usize];
            for j in 0..4 {
                // unless the previous Dir::Off, then only one of the
                // 4 calls the level to a different level. That is, only
                // one bit differs between successive elements of pulse.
                // Removing the redundancy is unlikely to matter because
                // the mechanical and thermal time constants are much
                // larger than the time wasted here.
                self.pps[j].set_level(match (&dir, (p>>j)&1) {
                    (Off, _) | (_, 0) if true => gpio::Level::Low,
                    _ => gpio::Level::High,
                })?;
            };
        };
        anyhow::Ok(())
    }
}

