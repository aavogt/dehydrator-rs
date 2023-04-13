// https://nwdthemes.com/2019/05/09/git-ignore-changes-in-tracked-file/
// alternatively I could set environment variables:
// use std::env;
// const SSID: &str = env!("RUST_ESP32_STD_DEMO_WIFI_SSID");
pub mod secrets {
  pub const ESSID : &str = "my essid";
  pub const PSK : &str = "secret password";
}
