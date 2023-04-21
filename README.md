# Dehydrator Automation

This project is for me to learn esp32 with rust. Also I wanted food dehydrator that shuts itself off when finished. The final dryness is hard to define, so I measure a total weight as well as humidity. I am also interested in dropping the temperature as drying proceeds. The temperature profile should minimize cost and maintain quality: higher temperatures may use less electricity: raising the temperature about 10°C doubles the amount of water vapor per mass of air, so half as much dry air has to be heated to a slightly higher temperature to carry the same amount of water vapor out of the dehydrator.

To this end I have selected the following components:

  - Excalibur 2400 food dehydrator
  - esp32c3 dev board (luatos ESP32C3-CORE in this case)
  - IR led transmitter module
  - IR plug (HENGMING HM-01K3)
  - 2xSHT31D humidity temperature sensors for inside and outside
  - 28BYJ-48 stepper motor with ULN2003 board
  - 5kg scale beam with HX711 board
  - ACS712 current sensor

# TODO

 - [ ] vero board layout
         - [ ] reassign pins (13 pins < 15 or 17 available)
 - [x] sht31: 2 pins for both
 - [x] nvs
   - [x] compress, serialize and store in nvs
   - [x] streamingly load, decompress, deserialize, turn into csv
   - [ ] test?
 - [ ] stepper: 4 + 3 pins
   - [x] half step / stop
   - [x] hall sensor logic
   - [x] `set_fraction`
   - [x] apply `set_fraction` to `config.step_fracs` at the right times in a separate thread
   - [ ] mechanical design
     - [ ] support for the motor (lego? meccano? kinex? also consider below:)
     - [ ] support for the hall sensors: tape to the dial
     - [ ] tape small magnet (bicycle spedometer) to a fixed point on the guide below the dial
   - [ ] possibly calibrate to measured temperatures? IE. we set the dial to 65 but get 63 so then a slow PI controller can get it up to 65
 - [ ] weigh scale: 2 pins
   - [x] `hx711_spi`
   - [ ] calibration
   - [ ] mounting/board supposedly needed to ensure it's accurate. Not sure about balancing? The front half of the dehydrator could be supported. This measurement of vibration from the fan. And then there are no problems with respect to keeping the weight over the centre / having the thing tip over unintentionally
 - [ ] ACS712 current meter: 1 pin
   - [x] raw adc
   - [ ] calibration?Voltage = (RawValue / 1024.0) * 5000; // Gets you mV int mVperAmp = 100; // use 100 for 20A Module and 66 for 30A Module
   - [ ] where to put it? It could go inside the dehydrator where the plug goes into. But then it's harder to calibrate. There is probably not enough room inside the HENGMING HM-01K3
 - [x] IR shutoff: 1 pin
   - [x] call from http
   - [ ] needs testing possibly the signal should be something different/standard that the remote can learn
 - [x] wifi
 - [x] serve www/{app.js,index.html} for local development serve it with `pnpm dev --open chromium-browser`
   - [x] submit/receive/manipulate T(t) profile, `w_cutoff`, `n_wavelets`
   - [x] request historical csv
   - [ ] plot historical csv with wavelet smoothing applied and overlay the cutoff?
   - [ ] current conditions
   - [ ] chart.js from a cdn or perhaps it will be small enough to be served from the esp32c3. Another option is to make it an android app.
