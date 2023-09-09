pub use display_interface_spi::SPIInterface;
pub use ssd1309::mode::GraphicsMode;
use stm32f1xx_hal::{pac, gpio::*, timer::{Tim2NoRemap, PwmHz, C1, Ch } };
use stm32f1xx_hal::spi::{ Spi, Spi1NoRemap, Spi2NoRemap };

pub type Systick = pac::TIM1;

//Display - SPI1 (SCK - PA5, CIPO - PA6, COPI - PA7), CS - PA4, DC - PA3
pub type Spi1SCKPin  = Pin<'A', 5, Alternate>;
pub type Spi1CIPOPin = Pin<'A', 6, Input>;
pub type Spi1COPIPin = Pin<'A', 7, Alternate>;
type Spi1Impl = Spi<pac::SPI1, Spi1NoRemap, (Spi1SCKPin, Spi1CIPOPin, Spi1COPIPin), u8>;
pub type DisplayCSPin = Pin<'A', 4, Output>;
pub type DisplayDCPin = Pin<'A', 3, Output>;
pub type DisplayInterface = SPIInterface<Spi1Impl, DisplayDCPin, DisplayCSPin>;
pub type Display = GraphicsMode<DisplayInterface>;

//Temp sensor - SPI2 (SCK - PB13, CIPO - PB14, COPI - PB15), CS - PB12
pub type Spi2SCKPin  = Pin<'B', 13, Alternate>;
pub type Spi2CIPOPin = Pin<'B', 14, Input>;
pub type Spi2COPIPin = Pin<'B', 15, Alternate>;
type Spi2Impl = Spi<pac::SPI2, Spi2NoRemap, (Spi2SCKPin, Spi2CIPOPin, Spi2COPIPin), u8>;
pub type TempSensorCSPin = Pin<'B', 12, Output>;
pub type TempSensor = crate::max6675::TempMAX6675<Spi2Impl, TempSensorCSPin>;
//Servo output - PA0
pub type ServoPin = Pin<'A', 0, Alternate>;
pub type ServoPwm = PwmHz<pac::TIM2, Tim2NoRemap, Ch<C1>, ServoPin>;
