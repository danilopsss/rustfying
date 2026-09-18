#![no_std]
#![no_main]

use embassy_executor::Spawner;
use core::fmt::Write;
use embassy_rp::{bind_interrupts, config::Config as RpConfig, i2c::{Async, Config as I2CConfig, I2c, InterruptHandler as I2CInterruptHandler}, peripherals::{I2C1, UART0, USB}, uart::{Config, InterruptHandler as UartInterruptHandler, Uart}, usb::{Driver, InterruptHandler as UsbInterruptHandler}};
use embassy_time::{Duration, Timer};
use embedded_graphics::{Drawable, geometry::Point, mono_font::{MonoTextStyle, iso_8859_2::FONT_5X8}, pixelcolor::BinaryColor, text::Text};
use heapless::{String};
use ssd1306::{I2CDisplayInterface, Ssd1306, mode::{BufferedGraphicsMode, DisplayConfig}, prelude::I2CInterface, rotation::DisplayRotation, size::DisplaySize128x64};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    I2C1_IRQ => I2CInterruptHandler<I2C1>;
    UART0_IRQ => UartInterruptHandler<UART0>;
});

const STYLE: MonoTextStyle<'static, BinaryColor> = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);

fn gpgga_to_decimals(measure: &f64, direction: &str) -> f64 {
    let set_dir = match direction {
        "S" | "W" => -1.0,
        "N" | "E" => 1.0,
        &_ => 0.0
    };
    let extract_degrees = (*measure - (*measure % 100.0)) / 100.0;
    let resolved_minutes = (*measure % 100.0) / 60.0;
    (extract_degrees + resolved_minutes) * set_dir
}

struct Gpgga {
    utc: i64,
    latitude: f64,
    longitude: f64,
    satelites: u16
}

impl Gpgga {
    fn new(raw_gpgga: &str) -> Option<Self> {
        let mut messages = raw_gpgga.split(",");
        messages.next();
        let utc = messages.next()?.parse::<f64>().ok().unwrap_or(0.0) as i64;
        let lat_raw = messages.next()?;
        let lat_dir = messages.next()?;
        if lat_raw.is_empty() || lat_dir.is_empty() {
            return None;
        }
        let latitude = gpgga_to_decimals(&lat_raw.parse::<f64>().ok()?, lat_dir);
        let lon_raw = messages.next()?;
        let lon_dir = messages.next()?;
        let longitude = gpgga_to_decimals(&lon_raw.parse::<f64>().ok()?, lon_dir);
        let satelites = messages.next().unwrap_or("0").parse::<u16>().expect("Can't parse satelites") as u16;

        Some(Self {
            utc,
            latitude,
            longitude,
            satelites
        })
    }
}

async fn write_to_display(
    display: &mut Ssd1306<I2CInterface<I2c<'_, I2C1, Async>>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
    msg: &[u8; 128]
) {
    let splitted = core::str::from_utf8(msg).unwrap_or("").split_inclusive('$');
    display.clear_buffer();
    let mut fixed = false;
    for splitted_text in splitted {
        log::info!("{:?}", splitted_text);
        if splitted_text.starts_with("GPGGA") {
            if let Some(coordinates) = Gpgga::new(splitted_text) {
                fixed = true;
                let mut msg: String<64> = String::new();
                write!(msg, "UTC:                   {}", coordinates.utc).ok();
                Text::new(&msg, Point::new(10, 10), STYLE).draw(display).unwrap();
                msg.clear();
                write!(msg, "Latitude:              {}", coordinates.latitude).ok();
                Text::new(&msg, Point::new(10, 20), STYLE).draw(display).unwrap();
                msg.clear();
                write!(msg, "Longitude:             {}", coordinates.longitude).ok();
                Text::new(&msg, Point::new(10, 30), STYLE).draw(display).unwrap();
                msg.clear();
                write!(msg, "Number of Satelites:   {}", coordinates.satelites).ok();
                Text::new(&msg, Point::new(10, 40), STYLE).draw(display).unwrap();
            }
        }
    }
    if !fixed {
        Text::new("NO GPS FIX", Point::new(24, 36), STYLE).draw(display).unwrap();
    }
    display.flush().expect("display flush failed");
}

#[embassy_executor::task]
async fn logger(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let config = RpConfig::default();
    let mut uart_config = Config::default();
    uart_config.baudrate = 9600;
    let p = embassy_rp::init(config);
    let usb_driver = Driver::new(p.USB, Irqs);

    let i2c = I2c::new_async(
        p.I2C1,
        p.PIN_27,
        p.PIN_26,
        Irqs,
        I2CConfig::default(),
    );
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate180)
        .into_buffered_graphics_mode();

    display.init().expect("could not initialize");

    let uart = Uart::new(p.UART0, p.PIN_0, p.PIN_1, Irqs, p.DMA_CH0, p.DMA_CH1, uart_config);
    let (_, mut rx) = uart.split();

    spawner.spawn(logger(usb_driver).unwrap());
    let mut buffer = [0u8; 128];
    loop {
        match rx.read(&mut buffer).await {
            Ok(()) => {
                write_to_display(&mut display, &buffer).await;
            },
            Err(e) => {
                log::warn!("Cant read. Error -> {:?}", e);
                Text::new("Waiting GPS Data", Point::new(24, 36), STYLE).draw(&mut display).unwrap();
                display.flush().expect("display flush failed");
                Timer::after(Duration::from_secs(1)).await;
            }
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}
