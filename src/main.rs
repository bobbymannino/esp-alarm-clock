fn main() {
    // Runtime patches that the ESP-IDF linker needs; must be first.
    esp_idf_svc::sys::link_patches();
    // Routes the `log` crate through ESP-IDF's logger.
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");
}
