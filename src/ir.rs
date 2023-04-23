use std::{sync::{Mutex, Arc}, ops::DerefMut};

use esp_idf_hal::{gpio::{AnyOutputPin, OutputPin}, rmt::{CHANNEL0, FixedLengthSignal, TxRmtDriver, RmtTransmitConfig, Pulse, PinState, PulseTicks, RmtChannel}, peripheral::Peripheral};

pub struct IrShutdown<'d> (Arc<Mutex<IrShutdownState>>);

/// boiletplate to be able to use `ir_shutdown();`
impl<'d> FnOnce<()> for IrShutdown<'d> {
    type Output = ();
    extern "rust-call" fn call_once(self, _args: ()) {
        self.0.lock().unwrap().send_signal();
    }
}

impl<'d> Fn<()> for IrShutdown<'d> {
    extern "rust-call" fn call(&self, _args: ()) {
        self.0.lock().unwrap().send_signal();
    }
}

impl<'d> FnMut<()> for IrShutdown<'d> {
    extern "rust-call" fn call_mut(&mut self, _args: ()) {
        self.0.lock().unwrap().send_signal();
    }
}

struct IrShutdownState {
   tx : TxRmtDriver<'static>,
   signal : FixedLengthSignal<2>,
}

impl IrShutdownState {
    fn send_signal(&mut self) {
        self.tx.start_blocking(&self.signal).unwrap();
    }
}


// or follow the NEC or another protocol?
impl<'d> IrShutdown<'d> {
    pub fn new(ir_pin : impl Peripheral<P = impl OutputPin> + 'd,
               channel : impl Peripheral<P = impl RmtChannel> + 'd ) 
        -> anyhow::Result<IrShutdown<'d>>
    {
            let config = RmtTransmitConfig::new().clock_divider(1);
            let tx = TxRmtDriver::new(channel, ir_pin, &config)?;
            let low = Pulse::new(PinState::Low, PulseTicks::new(10)?);
            let high = Pulse::new(PinState::High, PulseTicks::new(10)?);
            let mut signal = FixedLengthSignal::<2>::new();
            signal.set(0, &(low, high))?;
            signal.set(1, &(high, low))?;
            Ok(IrShutdown(Arc::new(Mutex::new(IrShutdownState { tx, signal }))))
    }
}
