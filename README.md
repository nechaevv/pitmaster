# About
Temperature controller for charcoal or wood smoker.  
Controller constantly reads temperature from sensor and adjust chimney damper using attached servo.
# Hardware
* Board - STM32F103C8T6 AKA "Blue Pill"
* Temperature sensor - MAX6675 with type-K thermocouple
* Display - SSD1309
* Servo - any standard RC servo with PWM input and enough force to drive the damper
* Wiring - refer to hw.rs for GPIO pin connections
# Toolchain setup
1. Prerequisites:
    - rustup
    - open-ocd
    - GDB (arm-none-eabi-gdb)
2. Install ARM target:
```bash
rustup target add thumbv7m-none-eabi
```
# Flashing
1. Connect STLINK-V2
2. Run openocd session in another terminal (must be running for GDB to connect)
```bash
# "set CPUID" option needed to flash knockoff board (non-genuine STM32)
openocd -f interface/stlink.cfg -c "set CPUTAPID 0x2ba01477" -f target/stm32f1x.cfg
```
3. Flash and run GDB session:
```bash
cargo run -r
```
4. type "c" to see program running
5. ctrl+c and "q" quits GDB
