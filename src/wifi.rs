use embedded_svc::wifi::Wifi;
use std::time::Duration;

use esp_idf_svc::{eventloop::{EspEventLoop, System}, wifi::EspWifi, nvs::{EspNvsPartition, NvsDefault}};
use anyhow::{anyhow, bail};
use log::*;
use embedded_svc::ipv4::Ipv4Addr;
use embedded_svc::wifi::{ClientConfiguration, Configuration };
use esp_idf_svc::netif::{EspNetif, EspNetifWait};
use esp_idf_svc::wifi::WifiWait;


// https://github.com/ivmarkov/rust-esp32-std-demo/blob/77150c2bfbbb417c93fee51556bea0aa2ea91e36/src/main.rs#L1379
pub fn connect<'a>(modem : esp_idf_hal::modem::Modem, sysloop : &'a EspEventLoop<System>,
                   nvs : EspNvsPartition<NvsDefault>) -> anyhow::Result<EspWifi<'a>> {

  use crate::secrets::*;

  let mut wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs))?;

  wifi.start()?;

  let ours = wifi.scan()?
      .into_iter()
      .find(|a| a.ssid == ESSID)
      .ok_or(anyhow!("should find access point"))?
      .channel;

  info!( "Found configured access point {} on channel {}", ESSID, ours);

  let conf = ClientConfiguration {
      ssid: ESSID.into(),
      password: PSK.into(),
      channel: Some(ours),
      ..Default::default()
  };

  wifi.set_configuration(&Configuration::Client(conf))?;

  info!("Starting wifi...");
  
  if !WifiWait::new(&sysloop)?
      .wait_with_timeout(Duration::from_secs(20), || wifi.is_started().unwrap())
  {
      bail!("Wifi did not start");
  }

  info!("Connecting wifi...");

  wifi.connect()?;

  if !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &sysloop)?.wait_with_timeout(
      Duration::from_secs(20),
      || {
          wifi.is_connected().unwrap()
              && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
      },
  ) {
      bail!("Wifi did not connect or did not receive a DHCP lease");
  }
  let ip_info = wifi.sta_netif().get_ip_info()?;
  info!("Wifi DHCP info: {:?}", ip_info);
  Ok(wifi)
}
