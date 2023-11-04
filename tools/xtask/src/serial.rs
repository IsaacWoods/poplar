use serialport::SerialPort;
use std::{path::Path, time::Duration};

pub struct Serial {
    port: Box<dyn SerialPort>,
}

impl Serial {
    pub fn new(device: &Path, baud: u32) -> Self {
        let port =
            serialport::new(device.to_str().unwrap(), baud).timeout(Duration::from_secs(10)).open().unwrap();
        Self { port }
    }

    pub fn listen(mut self) -> ! {
        loop {
            let mut buffer = [0u8; 256];
            let read_buffer = {
                let bytes_read = self.port.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    continue;
                }

                &mut buffer[0..bytes_read]
            };
            print!("{}", std::str::from_utf8(read_buffer).unwrap());
        }
    }
}
