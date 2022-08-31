use crate::{TestError, TestableCpu};

pub(crate) fn all_instrs_status_code(cpu: &impl TestableCpu) -> Result<(), TestError> {
    let status = cpu.memory_read(0x6000);
    let m1 = cpu.memory_read(0x6001);
    let m2 = cpu.memory_read(0x6002);
    let m3 = cpu.memory_read(0x6003);

    if m1 != 0xde || m2 != 0xb0 || m3 != 0x61 {
        return Err(TestError::String(format!(
            "invalid magic sequence: {m1:x}{m2:x}{m3:x}. the test output was corrupted"
        )));
    }

    if status == 0 {
        Ok(())
    } else {
        Err(TestError::String(format!(
            "exited with status {status}:\n {}",
            read_status_string(cpu)
        )))
    }
}

pub(crate) fn read_status_string(cpu: &impl TestableCpu) -> String {
    let mut res = String::new();
    for i in 0x6004..=0x7000 {
        let b = cpu.memory_read(i);
        if b == 0 {
            break;
        }

        res.push(char::from_u32(u32::from(b)).unwrap_or('�'))
    }

    res
}
