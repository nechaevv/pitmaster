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
    use stm32f1xx_hal::gpio::{ExtiPin, Edge};
    use stm32f1xx_hal::timer::Channel;
    use crate::max6675::{TempMAX6675, f_to_raw};
    use crate::pitmaster::State;

    #[shared]
    struct Shared {
        encoder_state: i8,
    }

    #[local]
    struct Local {
        tick_tm: CounterMs<pac::TIM1>,
        servo_pwm: ServoPwm,
        display: Display,
        temp_sensor: TempSensor,
        state: State,
        tick_led: TickLed,
        encoder_clk: EncoderClk,
        encoder_dt: EncoderDt,
    }

    #[init]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        let mut flash = cx.device.FLASH.constrain();
        let rcc = cx.device.RCC.constrain();
        let clocks = rcc.cfgr.freeze(&mut flash.acr);

        let mut delay = cx.core.SYST.delay(&clocks);

        let mut tick_tm = cx.device.TIM1.counter_ms(&clocks);
        tick_tm.start(220.millis()).unwrap(); // 220 ms - min read interval for MAX6675
        tick_tm.listen(Event::Update);

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
        let mut servo_pwm: ServoPwm = cx.device
            .TIM2
            .pwm_hz::<Tim2NoRemap, _, _>(servo_pin, &mut afio.mapr, 250.Hz(), &clocks);
        servo_pwm.enable(Channel::C1);

        let tick_led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let mut encoder_clk = gpioc.pc14.into_floating_input(&mut gpioc.crh);
        let encoder_dt = gpioc.pc15.into_floating_input(&mut gpioc.crh);
        encoder_clk.make_interrupt_source(&mut afio);
        encoder_clk.enable_interrupt(&mut cx.device.EXTI);
        encoder_clk.trigger_on_edge(&mut cx.device.EXTI, Edge::Rising);

        let state = State::new();

        (
            Shared {
                encoder_state: 0i8
            },
            Local {
                tick_tm,
                servo_pwm,
                display,
                temp_sensor,
                state,
                tick_led,
                encoder_clk,
                encoder_dt,
            }
        )
    }

    const ENC_TEMP_INCREMENT: u16 = f_to_raw(50) - f_to_raw(45); //5 F

    #[task(binds = TIM1_UP, priority = 1, local = [tick_tm, servo_pwm, temp_sensor, display, state, tick_led], shared = [encoder_state])]
    fn tick(mut cx: tick::Context) {
        let tick::LocalResources { temp_sensor, servo_pwm, display, tick_tm, tick_led, state, .. } = cx.local;
        tick_tm.clear_interrupt(Event::Update);

        let mut reset_error = false; // Reset error used to eliminate PID reaction on target temperature change
        // Handle encoder action
        cx.shared.encoder_state.lock(|encoder_state| {
            if *encoder_state > 0i8 {
                state.target_temp_raw += ENC_TEMP_INCREMENT;
                *encoder_state = 0i8;
                reset_error = true;
            }
            if *encoder_state < 0i8 {
                state.target_temp_raw -= ENC_TEMP_INCREMENT;
                *encoder_state = 0i8;
                reset_error = true;
            }
        });
        // Read temperature and run PID
        let new_temp_raw = temp_sensor.read_temp_raw().unwrap();
        state.on_temp_read(new_temp_raw, reset_error);
        // Update display
        display.clear();
        state.draw::<Display>(display);
        display.flush().unwrap();

        if state.is_ready() {
            // Update servo
            let max_duty = servo_pwm.get_max_duty() as u32;
            let new_duty = state.valve_pwm_duty() as u32 * max_duty / (u16::MAX as u32);
            servo_pwm.set_duty(Channel::C1, new_duty as u16);
        }

        tick_led.toggle();
    }

    #[task(binds = EXTI15_10, priority = 1, local = [encoder_clk, encoder_dt], shared = [encoder_state])]
    fn encoder_step(mut cx: encoder_step::Context) {
        cx.local.encoder_clk.clear_interrupt_pending_bit();
        let direction = cx.local.encoder_dt.is_low();
        cx.shared.encoder_state.lock(|encoder_state| {
            if direction {
                *encoder_state = 1;
            } else {
                *encoder_state = -1;
            }
        });
    }
}
