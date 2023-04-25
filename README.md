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
   - [x] calibrations
   - [ ] test?
 - [ ] stepper: 4 out + 3 in pins
   - [x] half step / stop
   - [x] hall sensor logic
     - [ ] pick supply voltage (3.3 vs 5) and resistors to avoid needing ADC and damage
   - [x] `set_fraction`
   - [x] apply `set_fraction` to `config.step_fracs` at the right times in a separate thread
   - [ ] mechanical design
     - [ ] support for the motor (lego? meccano? kinex? also consider below:)
     - [ ] support for the hall sensors: tape to the dial
     - [ ] tape small magnet (bicycle spedometer) to a fixed point on the guide below the dial
   - [ ] possibly calibrate to measured temperatures? IE. we set the dial to 65 but get 63 so then a slow PI controller can get it up to 65
 - [ ] weigh scale: 2 pins
   - [x] `hx711_spi`
   - [x] calibration, tare store in nvs
   - [ ] mounting/board supposedly needed to ensure it's accurate. Not sure about balancing? The front half of the dehydrator could be supported. This will reduce the effect of vibration from the fan on the measurement. And then there are no problems with respect to keeping the weight over the centre / having the thing tip over unintentionally
 - [ ] ACS712 current meter: 1 pin
   - [x] raw adc
   - [ ] note calibration https://github.com/esp-rs/esp-hal/issues/326
   - [ ] where to put it? It could go inside the dehydrator where the plug goes into. But then it's harder to calibrate. There is probably not enough room inside the HENGMING HM-01K3
 - [x] IR shutoff: 1 pin
   - [x] call from http
   - [ ] needs testing possibly the signal should be something different/standard that the remote can learn
 - [x] wifi
 - [x] serve www/{app.js,index.html} for local development serve it with `pnpm dev --open chromium-browser`
   - [x] submit/receive/manipulate T(t) profile, `w_cutoff`, `n_wavelets`
   - [x] request historical csv
   - [ ] request calibrations
   - [ ] plot historical csv with wavelet smoothing applied and overlay the cutoff? `esp_idf_sys::{httpd_req_get_url_query_len,httpd_req_get_url_query_str,httpd_query_key_value};`

   - [ ] current conditions
   - [ ] chart.js from a cdn or perhaps it will be small enough to be served from the esp32c3. Another option is to make it an android app.
   - [ ] current data. The js would be making requests? More natural would be for the js to subscribe and for the main.rs main loop to push the data. This is not http however. MQTT needs a server in the middle. Or the http request is made for data and this stays open?
