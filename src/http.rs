use std::thread;

use esp_idf_svc::http::{
    Method,
    client::{Configuration, EspHttpConnection},
};

use crate::error::{Error, Result};

/// Stack size, in bytes, of the worker thread each request runs on.
///
/// A TLS handshake that validates against the certificate bundle needs far more
/// stack than the ESP-IDF main task is given, so requests get their own thread.
const STACK_SIZE: usize = 16 * 1024;

/// Makes a HTTP `GET` request and returns the response body as a byte array.
///
/// The request runs on a dedicated thread with a [`STACK_SIZE`] byte stack, so
/// the caller's stack does not have to be big enough for the TLS handshake.
///
/// # Arguments
///
/// - `url`: The URL to make the request to.
///
/// # Returns
///
/// - `Ok`: The response body as a byte array.
/// - `Err`: An error if the request fails.
///
/// # Errors
///
/// - `Esp`: An error from the HTTP client.
/// - `HttpStatus`: A non 2xx response status.
/// - `Io`: The worker thread could not be spawned.
/// - `WorkerPanicked`: The worker thread panicked.
pub fn get(url: &str) -> Result<Vec<u8>> {
    thread::scope(|scope| {
        let handle = thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, || get_inner(url))?;

        handle.join().map_err(|_| Error::WorkerPanicked)?
    })
}

/// Performs the request itself, see [`get`].
fn get_inner(url: &str) -> Result<Vec<u8>> {
    log::info!("[GET {url}] Setting up");
    let mut conn = EspHttpConnection::new(&Configuration {
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    })?;

    log::info!("[GET {url}] Initate request");
    conn.initiate_request(Method::Get, url, &[])?;
    log::info!("[GET {url}] Initate response");
    conn.initiate_response()?;

    let status = conn.status();
    log::info!("[GET {url}] Response: {status}");
    if !(200..300).contains(&status) {
        return Err(Error::HttpStatus(url.to_string(), status));
    }

    let mut body = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let n = conn.read(&mut buf)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(buf.get(..n).unwrap_or_default());
    }

    Ok(body)
}
