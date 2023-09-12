# Dehydrator Automation

This project is for me to learn esp32 with rust. Also I wanted food dehydrator that shuts itself off when finished. The final dryness is hard to define, so I measure a total weight as well as humidity. I am also interested in dropping the temperature as drying proceeds. The temperature profile should minimize cost and maintain quality: higher temperatures may use less electricity: raising the temperature about 10°C doubles the maximum amount of water vapor per mass of air, so potentially half as much dry air has to be heated to a slightly higher temperature water to remove the same amount of water.

To this end I have selected the following components:

  - Excalibur 2400 food dehydrator
  - esp32c3 dev board (luatos ESP32C3-CORE in this case)
  - IR led transmitter module
  - IR plug (HENGMING HM-01K3)
  - 2xSHT31D humidity temperature sensors for inside and outside
  - 28BYJ-48 stepper motor with ULN2003 board
  - 5kg scale beam with HX711 board
  - ACS712 current sensor. A less invasive SCT-013, or [PZEM-004T](https://tasmota.github.io/docs/PZEM-0XX) might be preferable.


Allow GPIO11: `espefuse.py -p /dev/ttyACM0 burn_efuse VDD_SPI_AS_GPIO 1` and type BURN [^burn]

[^burn] https://github.com/chenxuuu/luatos-wiki/discussions/11#discussioncomment-3021045 see also https://www.esp32.com/viewtopic.php?t=25906

# TODO

 - [ ] wifi improvements
  - [ ] <https://github.com/esp-rs/espressif-trainings/tree/main/common/lib/wifi>
  - [ ] stop hardcoding credentials <https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/network/esp_dpp.html>. This can't be displayed on the 128x32 oled because 20 lines of UPPER HALF BLOCK, LOWER HALF BLOCK and space needs 41 pixels. Continue reading <https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/provisioning/provisioning.html>
 - [ ] gsl filter before cutoff
 - [x] `typescript_type_def`
   - [x] app.js -> app.ts but it doesn't quite typecheck
 - [ ] www sveltekit svelte-chartjs? adapter-static
 - [ ] vero board layout
         - [ ] reassign pins (13 pins < 15 or 17 available), adc pins are 0 through 5
 - [x] sht31: 2 pins for both
 - [x] nvs
   - [x] compress, serialize and store measurements in nvs
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
     - [ ] leaning against hall effect and instead doing temperature feedback and allowing the motor to lock at the endpoints. This is more complicated because I have to make a discrete time controller and set the parameters. Maybe the web interface can just set a vector that gets multiplied and added to the past. That is, I have e1 e2 e3 e4 which are the errors in temperature at the given times. Then I do not need to address the question of controller design when I write the embedded code. Perhaps the initial can still be open-loop: that I request a 3/4 turn towards one end (probably low) and then go up a number of steps that I know (from the angle of the dial and assuming all steps are effective). Slippage seems to be unidentifiable. Error only leads to clockwise or counterclockwise rotation and not knowledge about whether or not a rotation changed anything. It seems like it cannot be discovered for sure: what if a change in the mechanical bang-bang controller is due to a disturbance? Assume there are no disturbances. Then you're at a limit when a taking a step makes no difference in the relative length of the on/off periods. I know when it's on or off from the current sensor. Another way is to set the temperature somewhere in the middle. Identify the bang-bang period. Move the dial with a small amplitude (clipped sinewave?) such that we are moving when there is a switch on or off. What happens to the controller? I need to understand the hysteresis what does the bimetalic strip do with respect to the T_on and T_offlx.
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
   - [x] GET/POST calibrations to `/calib`
        - [ ] option to submit the whole calibration rather than just the y value corresponding to the current x?
        - [ ] display calibration line and not just two x,y pairs
   - [ ] plot historical csv with wavelet smoothing applied and overlay the cutoff? `esp_idf_sys::{httpd_req_get_url_query_len,httpd_req_get_url_query_str,httpd_query_key_value};`

   - [ ] current conditions
   - [ ] chart.js from a cdn or perhaps it will be small enough to be served from the esp32c3. Another option is to make it an android app.
   - [ ] current data. The js would be making requests? More natural would be for the js to subscribe and for the main.rs main loop to push the data. This is not http however. MQTT needs a server in the middle. Or the http request is made for data and this stays open?
