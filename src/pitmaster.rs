use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics_core::pixelcolor::BinaryColor;
use embedded_graphics_core::prelude::DrawTarget;
use heapless::String;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};
use ufmt::uwrite;
use crate::max6675;

const TEMP_AVG_BUFFER_SIZE: usize = 16;
const DISPLAY_WIDTH: usize = 128;
const DISPLAY_HEIGHT: usize = 64;
const GRAPH_HEIGHT: usize = DISPLAY_HEIGHT / 2;
const GRAPH_STEP_TICKS: u8 = 75;
const P_TERM: i32 = 1;
const D_TERM: i32 = 64;
const VALVE_MIN_PWM_DUTY: u32 = 15700;
const VALVE_MAX_PWM_DUTY: u32 = 37000;
const VALVE_DUTY_RANGE: u32 = VALVE_MAX_PWM_DUTY - VALVE_MIN_PWM_DUTY;

pub struct State {
    pub avg_buffer: ConstGenericRingBuffer<u16, TEMP_AVG_BUFFER_SIZE>,
    pub errors: ConstGenericRingBuffer<i32, TEMP_AVG_BUFFER_SIZE>,
    pub temp_history: ConstGenericRingBuffer<u16, DISPLAY_WIDTH>,
    pub valve_history: ConstGenericRingBuffer<u16, DISPLAY_WIDTH>,
    pub valve_pos: u16,
    pub target_temp_raw: u16,
    pub graph_tick_cnt: u8,
}

impl State {
    pub fn new() -> State {
        State {
            avg_buffer: ConstGenericRingBuffer::<u16, TEMP_AVG_BUFFER_SIZE>::new(),
            errors: ConstGenericRingBuffer::<i32, TEMP_AVG_BUFFER_SIZE>::new(),
            temp_history: ConstGenericRingBuffer::<u16, DISPLAY_WIDTH>::new(),
            valve_history: ConstGenericRingBuffer::<u16, DISPLAY_WIDTH>::new(),
            valve_pos: u16::MAX,
            target_temp_raw: max6675::f_to_raw(226),
            graph_tick_cnt: GRAPH_STEP_TICKS,
        }
    }
    pub fn on_temp_read(&mut self, new_temp_raw: u16) {
        // average temperature
        if self.avg_buffer.is_empty() {
            self.avg_buffer.fill(new_temp_raw);
        } else {
            self.avg_buffer.push(new_temp_raw);
        }
        let mut t_avg: i32 = 0; // multiplied by avg buffer size to preserve precision
        for t in self.avg_buffer.iter() {
            t_avg += *t as i32;
        }
        // PID
        let error = (self.target_temp_raw as i32) * (TEMP_AVG_BUFFER_SIZE as i32) - t_avg;
        let error_d = if self.errors.is_empty() {
            0
        } else {
            error - *self.errors.front().unwrap()
        };
        self.errors.push(error);
        let valve_d = (P_TERM * error + D_TERM * error_d) / (TEMP_AVG_BUFFER_SIZE as i32);
        if valve_d < -(self.valve_pos as i32) {
            self.valve_pos = 0;
        } else if valve_d > (u16::MAX - self.valve_pos) as i32 {
            self.valve_pos = u16::MAX;
        } else {
            let valve_pos = self.valve_pos as i32 + valve_d;
            self.valve_pos = valve_pos as u16;
        }
        if self.temp_history.is_empty() || self.graph_tick_cnt >= GRAPH_STEP_TICKS {
            self.temp_history.push(t_avg as u16);
            self.valve_history.push(self.valve_pos);
            self.graph_tick_cnt = 1;
        } else {
            *(self.temp_history.back_mut().unwrap()) = t_avg as u16;
            *(self.valve_history.back_mut().unwrap()) = self.valve_pos;
            self.graph_tick_cnt += 1;
        }
    }
    pub fn valve_pwm_duty(&self) -> u16 {
        (((self.valve_pos as u32) * VALVE_DUTY_RANGE >> 16) + VALVE_MIN_PWM_DUTY) as u16
    }

    pub fn draw<D>(&self, display: &mut D)
        where D: DrawTarget<Color=BinaryColor>,
              <D as DrawTarget>::Error: core::fmt::Debug
    {
        use embedded_graphics::{
            mono_font::ascii::{FONT_4X6, FONT_5X7},
            prelude::*,
            text::Text,
        };
        use ufmt::*;

        if self.temp_history.is_empty() {
            return;
        }

        let lg_text = MonoTextStyle::new(&FONT_5X7, BinaryColor::On);
        let sm_text = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
        let mut sbuf = String::<32>::new();

        uwrite!(sbuf, "T:{}   ", max6675::raw_to_f(*self.temp_history.back().unwrap()/(TEMP_AVG_BUFFER_SIZE as u16))).unwrap();
        Text::new(&sbuf, Point::new(0, 18), lg_text)
            .draw(display).unwrap();
        sbuf.clear();
        uwrite!(sbuf, "V:{}   ", ((self.valve_pos as u32) * 101) >> 16).unwrap();
        Text::new(&sbuf, Point::new(0, 50), lg_text)
            .draw(display).unwrap();
        sbuf.clear();

        let mut min_temp = *self.temp_history.iter().min().unwrap();
        let mut max_temp = *self.temp_history.iter().max().unwrap();
        let mut temp_range = max_temp - min_temp;
        const MIN_TEMP_RANGE: u16 = (GRAPH_HEIGHT * TEMP_AVG_BUFFER_SIZE) as u16;
        if temp_range < MIN_TEMP_RANGE {
            temp_range = MIN_TEMP_RANGE;
            let avg_temp = (max_temp + min_temp) / 2;
            max_temp = avg_temp + (MIN_TEMP_RANGE / 2);
            if avg_temp > MIN_TEMP_RANGE / 2 {
                min_temp = avg_temp - (MIN_TEMP_RANGE / 2);
            } else {
                min_temp = 0;
                temp_range = max_temp;
            }
        }

        uwrite!(sbuf, "{}", max6675::raw_to_f(max_temp/(TEMP_AVG_BUFFER_SIZE as u16))).unwrap();
        Text::new(&sbuf, Point::new(0, 6), sm_text)
            .draw(display).unwrap();
        sbuf.clear();
        uwrite!(sbuf, "{}", max6675::raw_to_f(min_temp/(TEMP_AVG_BUFFER_SIZE as u16))).unwrap();
        Text::new(&sbuf, Point::new(0, 30), sm_text)
            .draw(display).unwrap();
        sbuf.clear();
        uwrite!(sbuf, "TGT:{}   ", max6675::raw_to_f(self.target_temp_raw)).unwrap();
        Text::new(&sbuf, Point::new(96, 30), sm_text)
            .draw(display).unwrap();
        sbuf.clear();

        let target_temp_m = self.target_temp_raw * TEMP_AVG_BUFFER_SIZE as u16;
        if target_temp_m >= min_temp && target_temp_m <= max_temp {
            let display_value = (target_temp_m - min_temp) * (GRAPH_HEIGHT as u16) / temp_range;
            let dotted_line_pixels = (0..DISPLAY_WIDTH).step_by(2)
                .map(|x| Pixel(Point::new(x as i32, GRAPH_HEIGHT as i32 - display_value as i32), BinaryColor::On));
            display.draw_iter(dotted_line_pixels).unwrap();
        }

        let t_graph_start = (DISPLAY_WIDTH - self.temp_history.len()) as i32;
        let t_graph_iter = self.temp_history.iter().enumerate()
            .map(|(i, t)| {
                let display_value = (t - min_temp) * (GRAPH_HEIGHT as u16) / temp_range;
                Pixel(Point::new(i as i32 + t_graph_start, GRAPH_HEIGHT as i32 - display_value as i32), BinaryColor::On)
            });
        display.draw_iter(t_graph_iter).unwrap();

        let v_graph_start = (DISPLAY_WIDTH - self.valve_history.len()) as i32;
        let v_graph_iter = self.valve_history.iter().enumerate()
            .map(|(i, v)| {
                let divisor: i32 = (u16::MAX as i32 + 1) / (GRAPH_HEIGHT as i32);
                Pixel(Point::new(i as i32 + v_graph_start, DISPLAY_HEIGHT as i32 - 1 - (*v as i32 / divisor)), BinaryColor::On)
            });
        display.draw_iter(v_graph_iter).unwrap();

    }
}
