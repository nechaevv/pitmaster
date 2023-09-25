use embedded_hal as hal;
use hal::digital::v2::OutputPin;

#[derive(Clone, Debug)]
pub enum Max6675Error {
    SPIError,
    SensorError
}

pub struct TempMAX6675<SPI, CS> {
    spi: SPI,
    cs: CS
}

impl<SPI, CS> TempMAX6675<SPI, CS>
    where SPI: hal::blocking::spi::Transfer<u8>,
          CS: OutputPin {

    pub fn new(spi: SPI, cs: CS) -> Self {
        Self { spi, cs }
    }

    pub fn read_temp_raw(&mut self) -> Result<u16, Max6675Error> {
        let mut t_buf: [u8; 2] = [0u8,0u8];
        self.cs.set_low().map_err(|_| Max6675Error::SPIError)?;
        self.spi.transfer(&mut t_buf).map_err(|_| Max6675Error::SPIError)?;
        self.cs.set_high().map_err(|_| Max6675Error::SPIError)?;
        if (t_buf[1] & 0x4) > 0 {
            Err(Max6675Error::SensorError)
        } else {
            let t_u16 = ((t_buf[0] as u16) << 8) | (t_buf[1] as u16);
            Ok(t_u16 >> 3)
        }

    }

}

pub const fn raw_to_f(temp: u16) -> i16 {
    ((temp as i16) * 9 / 20) + 32
}

pub const fn f_to_raw(temp_f: i16) -> u16 {
    ((temp_f - 32) * 20 / 9) as u16
}
/*
pub fn raw_to_c(temp: u16) -> i16 {
    (temp as i16) / 4
}

pub fn c_to_raw(temp_c: i16) -> u16 {
    (temp_c * 4) as u16
}
*/