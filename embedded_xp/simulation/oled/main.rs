use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::pixelcolor::{BinaryColor};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics_simulator::{
    BinaryColorTheme, SimulatorDisplay, SimulatorEvent, Window, OutputSettingsBuilder,
};
use std::thread;
use std::time::Duration;

fn canvas(display: &mut SimulatorDisplay<BinaryColor>) {
    Rectangle::new(Point::new(1, 1), Size::new(126, 62))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)
        .unwrap();
}

fn main() {
    let mut display: SimulatorDisplay<BinaryColor> = SimulatorDisplay::new(Size::new(128, 64));
    let output_settings = OutputSettingsBuilder::new()
        .theme(BinaryColorTheme::OledBlue)
        .build();

    let mut window = Window::new("Example", &output_settings);
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let mut pos = 10;
    let mut grow = true;

    loop {
        canvas(&mut display);
        window.update(&display);

        if pos < 50 && grow {
            pos += 1
        } else if pos > 10 && !grow {
            pos -= 1
        } else {
            grow = !grow
        }
        display.clear(BinaryColor::Off);

        Text::with_alignment("La puta madre\nQue nos pario", Point::new(128 / 2, pos), style, Alignment::Center)
            .draw(&mut display).expect("Error");

        for event in window.events() {
            match event {
                SimulatorEvent::Quit => return,
                _ => {}
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}
