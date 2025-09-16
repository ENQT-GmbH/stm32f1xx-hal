#![no_main]
#![no_std]

use core::cell::RefCell;

use panic_halt as _;

use cortex_m::interrupt::Mutex;
use stm32f1::stm32f103::ADC1;
use stm32f1xx_hal::{self as hal, adc::Adc};

use cortex_m_rt::entry;

use crate::hal::{
    gpio::PinExt,
    pac::{interrupt, Peripherals},
    prelude::*,
    rcc,
};
use cortex_m_semihosting::hprintln;

static G_ADC: Mutex<RefCell<Option<Adc<ADC1>>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    // Acquire peripherals
    let p = Peripherals::take().unwrap();
    let mut flash = p.FLASH.constrain();
    let mut rcc = p.RCC.freeze(
        rcc::Config::hsi()
            .sysclk(56.MHz()) //note: F105 and F107 line devices can only reach 36 MHz from HSI
            .pclk1(28.MHz())
            .adcclk(14.MHz()),
        &mut flash.acr,
    );

    /*
    // Alternative configuration using dividers and multipliers directly
    let rcc = p.RCC.freeze(
        rcc::RawConfig {
            hse: Some(8_000_000),
            pllmul: Some(7),
            hpre: rcc::HPre::Div1,
            ppre1: rcc::PPre::Div2,
            ppre2: rcc::PPre::Div1,
            usbpre: rcc::UsbPre::Div1_5,
            adcpre: rcc::AdcPre::Div2,
            ..Default::default()
        },
        &mut flash.acr,
    );*/
    hprintln!("sysclk freq: {}", rcc.clocks.sysclk());
    hprintln!("adc freq: {}", rcc.clocks.adcclk());

    // Setup ADC
    let mut adc = p.ADC1.adc(&mut rcc);
    adc.unmask_irq();
    //setup a pin to be read
    let mut gpioa = p.GPIOA.split(&mut rcc);
    let adc_pin = gpioa.pa0.into_analog(&mut gpioa.crl);
    adc.set_regular_sequence(&[adc_pin.pin_id()]);
    //Start continuous conversion. Sample rate depends on adcclk and sample time
    adc.set_continuous_mode(true);

    //switch on interrupt while making sure it doesn't fire before adc move is compleat
    //move adc to global variable to access it inside the interrupt
    cortex_m::interrupt::free(|cs| {
        adc.enable_eoc_interrupt();
        *G_ADC.borrow(cs).borrow_mut() = Some(adc)
    });
    loop {
        //busy loop, wfi() would block swd
    }
}

#[interrupt]
fn ADC1_2() {
    static mut ADC: Option<Adc<ADC1>> = None;
    //Move the ADC from the global static to the local one so no more locking is needed
    let adc = ADC.get_or_insert_with(|| {
        cortex_m::interrupt::free(|cs| G_ADC.borrow(cs).replace(None).unwrap())
    });
    //If multiple interrupts sources were in use on the adc or use more then one adc check what happened and reset flags for handled interrupt sources
    //If multiple channels were monitored eoc event and a counter could be used to keep track of where in the sequence it is
    hprintln!(
        "measurement result {}",
        adc.read_latest_regular_conversion_result()
    );
}
