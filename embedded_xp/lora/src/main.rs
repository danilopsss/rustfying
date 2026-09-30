#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, config::Config as RpConfig, gpio::{Flex, Level, Output}, peripherals::{SPI0, USB}, pio::Pin, spi::{Blocking, Config as SPIConfig, Phase, Polarity, Spi}, usb::{Driver, InterruptHandler}};
use embassy_time::{block_for, Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const FREQ: u32 = 433_000_000;

#[embassy_executor::task]
async fn logger(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

fn reset_trigger(rst: &mut Output<'static>) {
    // Docs say that we need to low the reset pin for at least 10ms
    // then release it.
    rst.set_low();
    block_for(Duration::from_millis(10));
    rst.set_high();
    block_for(Duration::from_millis(10));
}

fn transact(
    spi: &mut Spi<'static, SPI0, Blocking>,
    nss: &mut Flex<'static>,
    command: &mut [u8])
{
    nss.set_low();
    log::info!("Sent: {:02x?}", command);
    spi.blocking_transfer_in_place(command).unwrap();
    nss.set_high();
    log::info!("Received: {:02x?}", command);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(RpConfig::default());
    let usb_driver = Driver::new(p.USB, Irqs);

    let mut nss = Flex::new(p.PIN_1);
    nss.set_as_output();
    nss.set_high();
    let mut rst = Output::new(p.PIN_8, Level::Low);

    let mut spi_cfg = SPIConfig::default();
    spi_cfg.polarity = Polarity::IdleLow;
    spi_cfg.phase = Phase::CaptureOnFirstTransition;
    let mut spi = Spi::new_blocking(p.SPI0, p.PIN_2, p.PIN_3, p.PIN_4, spi_cfg);

    spawner.spawn(logger(usb_driver).unwrap());

    Timer::after(Duration::from_secs(5)).await;

    reset_trigger(&mut rst);

    transact(&mut spi, &mut nss, &mut [0x01, 0x81]);
    block_for(Duration::from_millis(10));


    let mut _payload = [0u8; 255];

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
