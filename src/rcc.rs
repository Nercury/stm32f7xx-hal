use crate::device::{rcc, FLASH, RCC};

use crate::time::Hertz;

/// Extension trait that constrains the `RCC` peripheral
pub trait RccExt {
    /// Constrains the `RCC` peripheral so it plays nicely with the other abstractions
    fn constrain(self) -> Rcc;
}

impl RccExt for RCC {
    fn constrain(self) -> Rcc {
        Rcc {
            ahb1: AHB1(()),
            apb1: APB1 { _0: () },
            apb2: APB2 { _0: () },
            cfgr: CFGR {
                hclk: None,
                pclk1: None,
                pclk2: None,
                sysclk: None,
            },
        }
    }
}

/// Constrained RCC peripheral
pub struct Rcc {
    /// Advanced High-Performance Bus 1 (AHB1) registers
    pub ahb1: AHB1,

    /// Advanced Peripheral Bus 1 (APB1) registers
    pub apb1: APB1,
    /// Advanced Peripheral Bus 2 (APB2) registers
    pub apb2: APB2,
    pub cfgr: CFGR,
}

/// Advanced High-Performance Bus 1 (AHB1) registers
pub struct AHB1(());

impl AHB1 {
    pub fn enr(&mut self) -> &rcc::AHB1ENR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*RCC::ptr()).ahb1enr }
    }

    pub fn rstr(&mut self) -> &rcc::AHB1RSTR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*RCC::ptr()).ahb1rstr }
    }
}

/// Advanced Peripheral Bus 1 (APB1) registers
pub struct APB1 {
    _0: (),
}

impl APB1 {
    pub(crate) fn enr(&mut self) -> &rcc::APB1ENR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*RCC::ptr()).apb1enr }
    }

    pub(crate) fn rstr(&mut self) -> &rcc::APB1RSTR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*RCC::ptr()).apb1rstr }
    }
}

/// Advanced Peripheral Bus 2 (APB2) registers
pub struct APB2 {
    _0: (),
}

impl APB2 {
    pub fn enr(&mut self) -> &rcc::APB2ENR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*RCC::ptr()).apb2enr }
    }

    pub fn rstr(&mut self) -> &rcc::APB2RSTR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*RCC::ptr()).apb2rstr }
    }
}

const HSI: u32 = 16_000_000; // Hz

pub struct CFGR {
    hclk: Option<u32>,
    pclk1: Option<u32>,
    pclk2: Option<u32>,
    sysclk: Option<u32>,
}

impl CFGR {
    pub fn hclk<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.hclk = Some(freq.into().0);
        self
    }

    pub fn pclk1<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.pclk1 = Some(freq.into().0);
        self
    }

    pub fn pclk2<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.pclk2 = Some(freq.into().0);
        self
    }

    pub fn sysclk<F>(mut self, freq: F) -> Self
    where
        F: Into<Hertz>,
    {
        self.sysclk = Some(freq.into().0);
        self
    }

    pub fn freeze(self) -> Clocks {
        let flash = unsafe { &(*FLASH::ptr()) };
        let rcc = unsafe { &*RCC::ptr() };

        let sysclk = self.sysclk.unwrap_or(HSI);
        let hclk = self.hclk.unwrap_or(HSI);

        assert!(sysclk >= HSI);
        assert!(hclk <= sysclk);

        if sysclk == HSI && hclk == sysclk {
            // use HSI as source and run everything at the same speed
            rcc.cfgr.modify(|_, w| unsafe {
                w.ppre2().bits(0).ppre1().bits(0).hpre().bits(0).sw().hsi()
            });

            Clocks {
                hclk: Hertz(hclk),
                pclk1: Hertz(hclk),
                pclk2: Hertz(hclk),
                sysclk: Hertz(sysclk),
            }
        } else if sysclk == HSI && hclk < sysclk {
            let hpre_bits = match sysclk / hclk {
                0 => unreachable!(),
                1 => 0b0111,
                2 => 0b1000,
                3..=5 => 0b1001,
                6..=11 => 0b1010,
                12..=39 => 0b1011,
                40..=95 => 0b1100,
                96..=191 => 0b1101,
                192..=383 => 0b1110,
                _ => 0b1111,
            };

            // Use HSI as source and run everything at the same speed
            rcc.cfgr.modify(|_, w| unsafe {
                w.ppre2()
                    .bits(0)
                    .ppre1()
                    .bits(0)
                    .hpre()
                    .bits(hpre_bits)
                    .sw()
                    .hsi()
            });

            Clocks {
                hclk: Hertz(hclk),
                pclk1: Hertz(hclk),
                pclk2: Hertz(hclk),
                sysclk: Hertz(sysclk),
            }
        } else {
            assert!(sysclk <= 216_000_000 && sysclk >= 24_000_000);

            // We're not diving down the hclk so it'll be the same as sysclk
            let hclk = sysclk;

            let (pllm, plln, pllp) = if sysclk >= 96_000_000 {
                // Input divisor from HSI clock, must result in less than 2MHz
                let pllm = 16;

                // Main scaler, must result in >= 192MHz and <= 432MHz, min 50, max 432
                let plln = (sysclk / 1_000_000) * 2;

                // Sysclk output divisor, must result in >= 24MHz and <= 216MHz
                // needs to be the equivalent of 2, 4, 6 or 8
                let pllp = 0;

                (pllm, plln, pllp)
            } else if sysclk <= 54_000_000 {
                // Input divisor from HSI clock, must result in less than 2MHz
                let pllm = 16;

                // Main scaler, must result in >= 192MHz and <= 432MHz, min 50, max 432
                let plln = (sysclk / 1_000_000) * 8;

                // Sysclk output divisor, must result in >= 24MHz and <= 216MHz
                // needs to be the equivalent of 2, 4, 6 or 8
                let pllp = 0b11;

                (pllm, plln, pllp)
            } else {
                // Input divisor from HSI clock, must result in less than 2MHz
                let pllm = 16;

                // Main scaler, must result in >= 192MHz and <= 432MHz, min 50, max 432
                let plln = (sysclk / 1_000_000) * 4;

                // Sysclk output divisor, must result in >= 24MHz and <= 216MHz
                // needs to be the equivalent of 2, 4, 6 or 8
                let pllp = 0b1;

                (pllm, plln, pllp)
            };

            let ppre2_bits = if sysclk > 108_000_000 { 0b100 } else { 0 };
            let ppre1_bits = if sysclk > 108_000_000 {
                0b101
            } else if sysclk > 54_000_000 {
                0b100
            } else {
                0
            };

            // Calculate real divisor
            let ppre1 = 1 << (ppre1_bits - 0b011);
            let ppre2 = 1 << (ppre2_bits - 0b011);

            // Calculate new bus clocks
            let pclk1 = hclk / ppre1;
            let pclk2 = hclk / ppre2;

            // Adjust flash wait states
            flash.acr.write(|w| {
                w.latency().bits(if sysclk <= 30_000_000 {
                    0b0000
                } else if sysclk <= 60_000_000 {
                    0b0001
                } else if sysclk <= 90_000_000 {
                    0b0010
                } else if sysclk <= 120_000_000 {
                    0b0011
                } else if sysclk <= 150_000_000 {
                    0b0100
                } else if sysclk <= 180_000_000 {
                    0b0101
                } else if sysclk <= 210_000_000 {
                    0b0110
                } else {
                    0b0111
                })
            });

            // use PLL as source
            rcc.pllcfgr.write(|w| unsafe {
                w.pllm()
                    .bits(pllm)
                    .plln()
                    .bits(plln as u16)
                    .pllp()
                    .bits(pllp)
            });

            // Enable PLL
            rcc.cr.write(|w| w.pllon().set_bit());

            // Wait for PLL to stabilise
            while rcc.cr.read().pllrdy().bit_is_clear() {}

            // Set scaling factors and switch clock to PLL
            rcc.cfgr.modify(|_, w| unsafe {
                w.ppre2()
                    .bits(ppre2_bits)
                    .ppre1()
                    .bits(ppre1_bits)
                    .hpre()
                    .bits(0)
                    .sw()
                    .pll()
            });

            Clocks {
                hclk: Hertz(hclk),
                pclk1: Hertz(pclk1),
                pclk2: Hertz(pclk2),
                sysclk: Hertz(sysclk),
            }
        }
    }
}

/// Frozen clock frequencies
///
/// The existence of this value indicates that the clock configuration can no longer be changed
#[derive(Clone, Copy)]
pub struct Clocks {
    hclk: Hertz,
    pclk1: Hertz,
    pclk2: Hertz,
    sysclk: Hertz,
}

impl Clocks {
    /// Returns the frequency of the AHB1
    pub fn hclk(&self) -> Hertz {
        self.hclk
    }

    /// Returns the frequency of the APB1
    pub fn pclk1(&self) -> Hertz {
        self.pclk1
    }

    /// Returns the frequency of the APB2
    pub fn pclk2(&self) -> Hertz {
        self.pclk2
    }

    /// Returns the system (core) frequency
    pub fn sysclk(&self) -> Hertz {
        self.sysclk
    }
}
