//! # `tudelft-nes-test`
//! This is a helper crate for your NES emulator to run various test ROMs
use crate::all_instrs::{all_instrs_status_code, read_status_string};
use bitflags::bitflags;
use std::error::Error;
use std::thread;
use std::thread::JoinHandle;
use thiserror::Error;
use tudelft_nes_ppu::{run_cpu_headless_for, Cpu, Mirroring};

mod all_instrs;
mod nestest;

use crate::nestest::nestest_status_code;

/// Implement this trait to run our test on our CPU via the [`run_tests`] function.
pub trait TestableCpu: Cpu + Sized + 'static {
    fn get_cpu(rom: &[u8]) -> Result<Self, Box<dyn Error>>;
    fn set_program_counter(&mut self, value: u16);
    fn memory_read(&self, address: u16) -> u8;
}

bitflags! {
    /// Select which tests you want to run
    pub struct TestSelector: u32 {
        /// `NESTEST` is a pretty much all inclusive test suite for a NES CPU. It was designed to test almost every combination of flags, instructions,
        /// and registers. Some of these tests are very difficult.
        /// More information about this test ROM can be found [here](https://github.com/christopherpow/nes-test-roms/blob/master/other/nestest.txt)
        const NESTEST         = 0b00000001;

        /// `ALL_INSTRS` tests all instructions (including unofficial ones).
        /// More function about this test can be found [here](https://github.com/christopherpow/nes-test-roms/tree/master/instr_test-v5)
        const ALL_INSTRS      = 0b00000010;

        /// `OFFICIAL_INSTRS` tests all official nes instructions, a finished emulator should pass this.
        /// More function about this test can be found [here](https://github.com/christopherpow/nes-test-roms/tree/master/instr_test-v5)
        const OFFICIAL_INSTRS = 0b00000100;

        /// `NROM_TEST` is a very simple rom that tests some basic functionality, this is a good starting test to try and pass.
        /// The source for this rom can be found [here](https://gitlab.ewi.tudelft.nl/software-fundamentals/nes-nrom-test/-/blob/main/src/init.s)
        const NROM_TEST       = 0b00001000;

        /// This test selector runs all available tests
        const ALL             = Self::NESTEST.bits | Self::ALL_INSTRS.bits | Self::NROM_TEST.bits;

        /// This test selector runs a default selection of tests: `OFFICIAL_INSTRS` and `NROM_TEST`
        const DEFAULT         = Self::OFFICIAL_INSTRS.bits | Self::NROM_TEST.bits;
    }
}

impl Default for TestSelector {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The main function of this crate, run this with your CPU as generic parameter and a [`TestSelector`] to run the tests
pub fn run_tests<T: TestableCpu>(selector: TestSelector) -> Result<(), String> {
    if selector.contains(TestSelector::ALL_INSTRS) {
        all_instrs::<T>(false)?;
    }

    if selector.contains(TestSelector::OFFICIAL_INSTRS) {
        all_instrs::<T>(true)?;
    }

    if selector.contains(TestSelector::NESTEST) {
        nestest::<T>()?;
    }

    if selector.contains(TestSelector::NROM_TEST) {
        nrom_test::<T>()?;
    }

    Ok(())
}

/// Tests the emulator using "all_instrs.nes" or "official_only.nes":
/// https://github.com/christopherpow/nes-test-roms/tree/master/instr_test-v5
fn all_instrs<T: TestableCpu + 'static>(only_official: bool) -> Result<(), String> {
    let (rom, limit) = if only_official {
        (include_bytes!("roms/official_only.nes"), 350)
    } else {
        (include_bytes!("roms/all_instrs.nes"), 500)
    };

    let handle = thread::spawn(move || {
        // TODO: make initial program counter obsolete by modifying nestest
        let mut cpu = T::get_cpu(rom).map_err(|i| TestError::Custom(i.to_string()))?;
        let mut prev = String::new();

        for i in 0..limit {
            if let Err(e1) = run_cpu_headless_for(&mut cpu, Mirroring::Horizontal, 200_000) {
                if let Err(e2) = all_instrs_status_code(&cpu) {
                    return Err(TestError::Custom(format!(
                        "{e1}, possibly due to a test that didn't pass: '{e2}'"
                    )));
                } else {
                    return Err(TestError::Custom(format!("{e1}")));
                }
            }

            let status = read_status_string(&cpu);

            if status.contains("Failed") {
                break;
            }

            let status = status.split('\n').next().unwrap().trim().to_string();
            if !status.is_empty() && status != prev {
                log::info!("{:05}k cycles passed: {}", i * 200, status);
            }
            prev = status;
        }

        let result = run_cpu_headless_for(&mut cpu, Mirroring::Horizontal, 200_000);

        match result {
            Err(e1) => {
                if let Err(e2) = all_instrs_status_code(&cpu) {
                    Err(TestError::Custom(format!(
                        "{e1}, possibly due to a test that didn't pass: '{e2}'"
                    )))
                } else {
                    Err(TestError::Custom(format!("{e1}")))
                }
            }
            Ok(()) => all_instrs_status_code(&cpu),
        }
    });

    process_handle(
        &format!(
            "all instructions{}",
            if only_official {
                " (official only)"
            } else {
                ""
            }
        ),
        handle,
    )
}

/// Runs the nestest rom:
/// https://github.com/christopherpow/nes-test-roms/blob/master/other/nestest.nes
fn nestest<T: TestableCpu + 'static>() -> Result<(), String> {
    let rom = include_bytes!("roms/nestest.nes");

    let handle = thread::spawn(|| {
        // TODO: make initial program counter obsolete by modifying nestest
        let mut cpu = T::get_cpu(rom).map_err(|i| TestError::Custom(i.to_string()))?;
        cpu.set_program_counter(0xC000);
        let result = run_cpu_headless_for(&mut cpu, Mirroring::Horizontal, 1_000_000);

        match result {
            Err(e1) => {
                if let Err(e2) =
                    nestest_status_code(cpu.memory_read(0x0002), cpu.memory_read(0x0003))
                {
                    Err(TestError::Custom(format!(
                        "{e1}, possibly due to a test that didn't pass: '{e2}'"
                    )))
                } else {
                    Err(TestError::Custom(format!("{e1}")))
                }
            }
            Ok(()) => nestest_status_code(cpu.memory_read(0x0002), cpu.memory_read(0x0003)),
        }
    });

    process_handle("nestest", handle)
}

/// runs our own nrom test rom
/// https://gitlab.ewi.tudelft.nl/software-fundamentals/nes-nrom-test
fn nrom_test<T: TestableCpu + 'static>() -> Result<(), String> {
    let rom = include_bytes!("roms/nrom-test.nes");

    let handle = thread::spawn(|| {
        let mut cpu = T::get_cpu(rom).map_err(|i| TestError::Custom(i.to_string()))?;
        run_cpu_headless_for(&mut cpu, Mirroring::Horizontal, 10)
            .map_err(|i| TestError::Custom(i.to_string()))?;

        if cpu.memory_read(0x42) != 0x43 {
            Err(TestError::String(
                "memory location 0x42 is wrong after executing nrom_test".to_owned(),
            ))
        } else if cpu.memory_read(0x43) != 0x6A {
            Err(TestError::String(
                "memory location 0x43 is wrong after executing nrom_test".to_owned(),
            ))
        } else {
            Ok(())
        }
    });

    process_handle("nrom_test", handle)
}

#[derive(Debug, Error)]
enum TestError {
    #[error("{0}")]
    Custom(String),
    #[error("{0}")]
    String(String),
}

fn process_handle(name: &str, handle: JoinHandle<Result<(), TestError>>) -> Result<(), String> {
    match handle.join() {
        // <- waits for the thread to complete or panic
        Ok(Ok(_)) => {
            log::info!("{name} finished succesfully");
            Ok(())
        }
        Ok(Err(e)) => match e {
            TestError::Custom(e) => Err(format!(
                "cpu failed while running test {name} with custom error message {e}"
            )),
            TestError::String(e) => Err(format!("cpu didn't pass test {name}: '{e}'")),
        },
        Err(e) => {
            let err_msg = match (e.downcast_ref::<&str>(), e.downcast_ref::<String>()) {
                (Some(&s), _) => s,
                (_, Some(s)) => s,
                (None, None) => "<No panic info>",
            };

            Err(format!(
                "cpu implementation panicked while running test {name}: {err_msg}"
            ))
        }
    }
}
