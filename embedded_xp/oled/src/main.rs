#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_rp::{bind_interrupts, config::Config, gpio::{Input, Pull}, i2c::{self, Async, I2c, InterruptHandler as I2CInterruptHandler}, peripherals::{I2C1, USB}, usb::{Driver, InterruptHandler}};
use embassy_time::{Duration, Timer};
use embedded_graphics::{Drawable, geometry::{Point, Size}, mono_font::{MonoTextStyle, iso_8859_5::FONT_6X10}, pixelcolor::BinaryColor, prelude::Primitive, primitives::{PrimitiveStyle, Rectangle}, text::Text};
use ssd1306::{I2CDisplayInterface, Ssd1306, mode::{BufferedGraphicsMode, DisplayConfig}, prelude::I2CInterface, rotation::DisplayRotation, size::DisplaySize128x64};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    I2C1_IRQ => I2CInterruptHandler<I2C1>;
});

const OLED_SIZE: (u32, u32) = (128, 64);


type OledDisplay = Ssd1306<
    I2CInterface<I2c<'static, I2C1, Async>>,
    DisplaySize128x64,
    BufferedGraphicsMode<DisplaySize128x64>,
>;

static DISPLAY: Mutex<ThreadModeRawMutex, Option<OledDisplay>> = Mutex::new(None);
static COUNTER: Mutex<ThreadModeRawMutex, u8> = Mutex::new(0);


async fn canvas(display: &mut OledDisplay) {
    Rectangle::new(Point::new(1, 1), Size::new(OLED_SIZE.0 - 2, OLED_SIZE.1 - 2))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)
        .unwrap();
    display.flush().expect("cant flush");
}

async fn write(text: &str, pos: Point) {
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let mut is_increasing = true;
    let mut cur_pos: i32 = pos.x;
    loop {
        {
            let mut guard = DISPLAY.lock().await;
            if let Some(ref mut display) = *guard {
                if cur_pos <= OLED_SIZE.0 as i32 - (text.len() * FONT_6X10.character_size.width as usize  + 4) as i32 && is_increasing {
                    cur_pos += 1
                } else if cur_pos >= pos.x as i32 && !is_increasing {
                    cur_pos -= 1
                } else {
                    is_increasing = !is_increasing;
                    let mut counter = COUNTER.lock().await;
                    *counter = counter.saturating_add(1);
                }
                display.clear_buffer();
                Text::new(text, Point::new(cur_pos as i32, pos.y), style).draw(display).unwrap();
                display.flush().unwrap();
                {
                    let mut counter = COUNTER.lock().await;
                    if *counter == 3 {
                        display.set_display_on(false).expect("Can't put display to sleep");
                        *counter = 0;
                    }
                }
            }
        }
        Timer::after_micros(100).await;
    }
}

#[embassy_executor::task]
async fn setup() {
    {
        let mut guard = DISPLAY.lock().await;
        if let Some(ref mut display) = *guard {
            display.init().expect("could not initialize");
        }
    }
    write("Test", Point::new(5, 10)).await;
}


#[embassy_executor::task]
async fn logger(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let config = Config::default();
    let p = embassy_rp::init(config);
    let usb_driver = Driver::new(p.USB, Irqs);
    let mut wake = Input::new(p.PIN_0, Pull::Down);

    let i2c = I2c::new_async(
        p.I2C1,
        p.PIN_27,
        p.PIN_26,
        Irqs,
        i2c::Config::default(),
    );

    let interface = I2CDisplayInterface::new(i2c);

    let display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    {
        let mut display_guard = DISPLAY.lock().await;

        *display_guard = Some(display);
    }

    spawner.spawn(logger(usb_driver).unwrap());
    spawner.spawn(setup().unwrap());

    loop {
        wake.wait_for_high().await;
        log::info!("High!");
        wake.wait_for_low().await;
        log::info!("Low");
        {
            let counter = COUNTER.lock().await;
            log::info!("Counter: {}", *counter);
        }
        let mut guard = DISPLAY.lock().await;
        log::info!("Acquired Lock!");
        {
            if let Some(ref mut display) = *guard {
                log::info!("Setting display on");
                display.set_display_on(true).expect("Can't set display on.");
            }
        }
        Timer::after(Duration::from_secs(1)).await;
    }
}
