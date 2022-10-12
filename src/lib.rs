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

pub trait TestableCpu: Cpu + Sized + 'static {
    type GetCpuError: Error;

    fn get_cpu(rom: &[u8]) -> Result<Self, Self::GetCpuError>;
    fn set_program_counter(&mut self, value: u16);
    fn memory_read(&self, address: u16) -> u8;
}

bitflags! {
    /// Select which tests you want to run
    pub struct TestSelector: u32 {
        const NESTEST         = 0b00000001;
        const ALL_INSTRS      = 0b00000010;
        const OFFICIAL_INSTRS = 0b00000100;
        const ALL             = Self::NESTEST.bits | Self::ALL_INSTRS.bits;
        const DEFAULT         = Self::OFFICIAL_INSTRS.bits;
    }
}

impl Default for TestSelector {
    fn default() -> Self {
        Self::DEFAULT
    }
}

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
