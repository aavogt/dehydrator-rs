use std::sync::{Mutex, Arc};

use esp_idf_hal::{gpio::AnyOutputPin, rmt::{CHANNEL0, FixedLengthSignal, TxRmtDriver, RmtTransmitConfig, Pulse, PinState, PulseTicks}};

pub struct IrShutdown (Arc<Mutex<IrShutdownState>>);

/// boiletplate to be able to use `ir_shutdown();`
impl FnOnce<()> for IrShutdown {
    type Output = ();
    extern "rust-call" fn call_once(self, _args: ()) {
        self.0.lock().unwrap().send_signal();
    }
}

impl Fn<()> for IrShutdown {
    extern "rust-call" fn call(&self, _args: ()) {
        self.0.lock().unwrap().send_signal();
    }
}

impl FnMut<()> for IrShutdown {
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
impl IrShutdown {
    /// TODO CHANNEL0 shouldn't be hardcoded. It could either be a parameter,
    /// or I need an AnyChannel like AnyOutputPin
    pub fn new<'d>(ir_pin : AnyOutputPin, channel : CHANNEL0) 
        -> anyhow::Result<IrShutdown>
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
