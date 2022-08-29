use std::error::Error;
use std::thread;
use std::thread::JoinHandle;
use thiserror::Error;
use tudelft_nes_ppu::{Cpu, Mirroring};

mod nestest;

use crate::nestest::nestest_status_code;
pub use tudelft_nes_ppu::run_cpu_headless_for;

pub trait TestableCpu: Cpu + Sized + 'static {
    fn get_cpu(rom: &[u8]) -> Result<Self, Box<dyn Error>>;
    fn set_program_counter(&mut self, value: u16);
    fn memory_read(&self, address: u16) -> u8;
}

pub fn run_all_tests<T: TestableCpu>() -> Result<(), String> {
    nestest::<T>()?;

    Ok(())
}

fn nestest<T: TestableCpu + 'static>() -> Result<(), String> {
    let rom = include_bytes!("roms/nestest.nes");

    let handle = thread::spawn(|| {
        // TODO: make initial program counter obsolete by modifying nestest
        let mut cpu = T::get_cpu(rom).map_err(|i| TestError::Custom(i.to_string()))?;
        cpu.set_program_counter(0xC000);
        let result = run_cpu_headless_for(&mut cpu, Mirroring::Horizontal, 1_000_000);

        match result {
            Err(e) => {
                if let Err(e) =
                    nestest_status_code(cpu.memory_read(0x0002), cpu.memory_read(0x0003))
                {
                    Err(TestError::Custom(format!(
                        "{e}, possibly due to a test that didn't pass: '{e}'"
                    )))
                } else {
                    Err(TestError::Custom(format!("{e}")))
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
            println!("{name} finished succesfully");
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
