#![no_main]
#![no_std]
#![deny(warnings)]
#![deny(unsafe_code)]

mod pitmaster;
mod max6675;
mod hw;

use panic_halt as _;

#[rtic::app(device = pac)]
mod app {
    use crate::hw::*;
    use embedded_hal::spi::MODE_0;
    use stm32f1xx_hal::{prelude::*, timer::{CounterMs, Event}, spi::Spi, timer::Tim2NoRemap, pac};
    use stm32f1xx_hal::timer::Channel;
    use crate::max6675::TempMAX6675;
    use crate::pitmaster::State;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        systick: CounterMs<Systick>,
        servo_pwm: ServoPwm,
        display: Display,
        temp_sensor: TempSensor,
        state: State,
        tick_led: TickLed,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let mut flash = cx.device.FLASH.constrain();
        let rcc = cx.device.RCC.constrain();
        let clocks = rcc.cfgr.freeze(&mut flash.acr);

        let mut delay = cx.device.TIM3.delay_us(&clocks);
        let mut systick = cx.device.TIM1.counter_ms(&clocks);
        systick.start(1.secs()).unwrap();
        systick.listen(Event::Update);

        let mut afio = cx.device.AFIO.constrain();
        let mut gpioa = cx.device.GPIOA.split();
        let mut gpiob = cx.device.GPIOB.split();
        let mut gpioc = cx.device.GPIOC.split();

        // SPI1
        let sck1 = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
        let cipo1 = gpioa.pa6;
        let copi1 = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

        let spi1 = Spi::spi1(
            cx.device.SPI1,
            (sck1, cipo1, copi1),
            &mut afio.mapr,
            MODE_0,
            1.MHz(),
            clocks,
        );

        // Display (SSD1309)
        let display_cs = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
        let display_dc = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
        let mut display_rs = gpioa.pa2.into_push_pull_output(&mut gpioa.crl);
        let display_interface = display_interface_spi::SPIInterface::new(spi1,
                                                                         display_dc, display_cs);
        let mut display: ssd1309::mode::GraphicsMode<_> = ssd1309::Builder::new()
            .connect(display_interface)
            .into();
        display.reset(&mut display_rs, &mut delay).unwrap();
        display.init().unwrap();
        display.flush().unwrap();
        // Temp sensor (MAX6675)
        // SPI2
        let sck2 = gpiob.pb13.into_alternate_push_pull(&mut gpiob.crh);
        let cipo2 = gpiob.pb14;
        let copi2 = gpiob.pb15.into_alternate_push_pull(&mut gpiob.crh);

        let spi2 = Spi::spi2(
            cx.device.SPI2,
            (sck2, cipo2, copi2),
            MODE_0,
            1.MHz(),
            clocks,
        );
        let temp_cs = gpiob.pb12.into_push_pull_output(&mut gpiob.crh);
        let temp_sensor: TempSensor = TempMAX6675::new(spi2, temp_cs);

                        // Servo PWM
                        let servo_pin: ServoPin = gpioa.pa0.into_alternate_push_pull(&mut gpioa.crl);
                        let servo_pwm: ServoPwm = cx.device
                            .TIM2
                            .pwm_hz::<Tim2NoRemap, _, _>(servo_pin, &mut afio.mapr, 250.Hz(), &clocks);

        let tick_led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let state = State::new();

        (
            Shared {},
            Local {
                systick,
                servo_pwm,
                display,
                temp_sensor,
                state,
                tick_led,
            }
        )
    }

    #[task(binds = TIM1_UP, priority = 1, local = [systick, servo_pwm, temp_sensor, display, state, tick_led])]
    fn tick(cx: tick::Context) {
        let tick::LocalResources { temp_sensor, state, servo_pwm, display, systick, tick_led, .. } = cx.local;
        let new_temp_raw = temp_sensor.read_temp_raw().unwrap();
        state.on_temp_read(new_temp_raw);
        servo_pwm.set_duty(Channel::C1, state.valve_pwm_duty());
        display.clear();
        state.draw_graphs::<Display>(display);
        display.flush().unwrap();
        tick_led.toggle();
        systick.clear_interrupt(Event::Update);
    }
}
