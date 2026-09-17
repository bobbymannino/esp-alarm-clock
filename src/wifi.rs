use std::net::Ipv4Addr;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::WifiModemPeripheral,
    nvs::{EspDefaultNvsPartition, EspNvsPartition},
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};

use crate::error::{Error, Result};

/// Maximum length, in bytes, of an SSID.
pub const MAX_SSID_LEN: usize = 32;
/// Maximum length, in bytes, of a WPA3 pre shared key.
pub const MAX_PASSWORD_LEN: usize = 64;
/// Shortest WPA3 pre shared key the standard allows.
pub const MIN_PASSWORD_LEN: usize = 8;

/// A WPA3 personal station (client) radio.
///
/// The radio is started on creation and stopped when dropped.
pub struct Wifi<'d> {
    wifi: BlockingWifi<EspWifi<'d>>,
}

impl<'d> Wifi<'d> {
    /// Creates a new [`Wifi`] and starts the radio in station mode.
    pub fn new<M: WifiModemPeripheral + 'd>(modem: M, nvs: EspDefaultNvsPartition) -> Result<Self> {
        let sysloop = EspSystemEventLoop::take()?;

        let wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs))?;
        let mut wifi = BlockingWifi::wrap(wifi, sysloop)?;

        wifi.start()?;

        Ok(Self { wifi })
    }

    /// Connects to the given WPA 2/3 personal network and waits for an IP address
    /// to be handed out by DHCP.
    ///
    /// # Arguments
    ///
    /// * `ssid` - Name of the network, at most [`MAX_SSID_LEN`] bytes.
    /// * `password` - Pre shared key, [`MIN_PASSWORD_LEN`] to [`MAX_PASSWORD_LEN`] bytes.
    pub fn connect(&mut self, ssid: &str, password: &str) -> Result<Ipv4Addr> {
        if ssid.is_empty() {
            return Err(Error::InvalidSsid);
        }
        if password.len() < MIN_PASSWORD_LEN {
            return Err(Error::InvalidPassword);
        }

        let Ok(ssid_con) = ssid.try_into() else {
            return Err(Error::InvalidSsid);
        };
        let Ok(password) = password.try_into() else {
            return Err(Error::InvalidPassword);
        };

        self.wifi.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: ssid_con,
            password,
            auth_method: AuthMethod::WPA2WPA3Personal,
            ..Default::default()
        }))?;

        log::info!("Attempting to connect to {ssid}");
        self.wifi.connect()?;
        self.wifi.wait_netif_up()?;

        self.ip()
    }

    /// Disconnects from the network, leaving the radio started so that
    /// [`Wifi::connect`] can be called again.
    pub fn disconnect(&mut self) -> Result<()> {
        self.wifi.disconnect()?;

        Ok(())
    }

    /// Whether the radio is associated with an access point.
    ///
    /// Being connected does not mean an IP address has been assigned yet; use
    /// [`Wifi::is_up`] for that.
    pub fn is_connected(&self) -> Result<bool> {
        Ok(self.wifi.is_connected()?)
    }

    /// Whether the network interface is up, i.e. it has an IP address.
    pub fn is_up(&self) -> Result<bool> {
        Ok(self.wifi.is_up()?)
    }

    /// The IPv4 address currently assigned to the station interface.
    pub fn ip(&self) -> Result<Ipv4Addr> {
        let ip_info = self.wifi.wifi().sta_netif().get_ip_info()?;

        Ok(ip_info.ip)
    }
}
