#![no_std]
#![no_main]

use embassy_executor;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    gpio::Flex,
    peripherals::USB,
    usb::{Driver, InterruptHandler},
};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embassy_usb_logger;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn logger(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

struct DhtResponse {
    temp: f64,
    humidity: u8,
}

async fn dht_read(pin: &mut Flex<'static>) -> DhtResponse {
    pin.set_as_output();
    pin.set_low();
    Timer::after(Duration::from_millis(18)).await;

    pin.set_as_input();

    with_timeout(Duration::from_micros(45), pin.wait_for_low())
        .await
        .expect("Timed out [45us]");
    with_timeout(Duration::from_micros(85), pin.wait_for_high())
        .await
        .expect("Timed out [85us]");
    with_timeout(Duration::from_micros(85), pin.wait_for_low())
        .await
        .expect("Timed out [85us]");
    with_timeout(Duration::from_micros(55), pin.wait_for_high())
        .await
        .expect("Timed out [85us]");

    let mut value = [0u8; 5];

    for bit in 0..40 {
        pin.wait_for_high().await;
        let start = Instant::now();
        pin.wait_for_low().await;
        let elapsed = start.elapsed().as_micros();

        match elapsed {
            23..=28 => {
                value[bit / 8] = value[bit / 8] << 1;
            }
            65..=75 => {
                value[bit / 8] = value[bit / 8] << 1;
                value[bit / 8] |= 1;
            }
            _ => {}
        }
    }
    let [humidity, _, temp, temp_dec, _] = value;

    DhtResponse {
        temp: temp as f64 + temp_dec as f64 / 10 as f64,
        humidity: humidity,
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());
    let usb_driver = Driver::new(p.USB, Irqs);
    let mut sensor_pin = Flex::new(p.PIN_0);
    spawner.spawn(logger(usb_driver).unwrap());
    log::info!("Starting in 10s");
    Timer::after(Duration::from_secs(10)).await;
    loop {
        let measure = dht_read(&mut sensor_pin).await;
        log::info!(
            "Temperature: {}Cº\r\nHumidity: {}%",
            measure.temp,
            measure.humidity
        );
        Timer::after(Duration::from_secs(5)).await;
    }
}
