use esp_idf_hal::{gpio::ADCPin, adc::{Adc, AdcDriver, AdcChannelDriver, Atten0dB}, peripheral::Peripheral};


pub struct ACS172<'d, PIN : ADCPin, ADC : Adc > {
    pub driver : AdcDriver<'d, ADC>,
    pub channel : AdcChannelDriver<'d, PIN, Atten0dB<<PIN as ADCPin>::Adc>>,
}

impl<'d, PIN : ADCPin, ADC : Adc> ACS172<'d, PIN, ADC> {
    pub fn new(pin : impl Peripheral<P = PIN> + 'd,
               adc : impl Peripheral<P = ADC> + 'd) -> anyhow::Result<Self> {
        let conf = Default::default();
        let driver = AdcDriver::new(adc, &conf)?;
        let channel : AdcChannelDriver<_, Atten0dB<_>> = AdcChannelDriver::new(pin)?;
        Ok(Self { driver, channel })
    }
}
