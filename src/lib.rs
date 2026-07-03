use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

pub const DEFAULT_FREQUENCY: u16 = 300;
pub const DEFAULT_AMPLITUDE: u16 = 200;

const DEFAULT_UNIT_ID: u8 = 1;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

const REG_CONTROL: u16 = 4192;
const REG_FAULTS: u16 = 8194;
const REG_LIGHT1_BRIGHTNESS: u16 = 4099;

const BIT_MOTION_ENABLE: u16 = 1 << 4;
const BIT_LIGHT1_ENABLE: u16 = 1 << 5;
const BIT_CLEAR_FAULTS: u16 = 1 << 15;
const MOTION_BITS_MASK: u16 = 0x000F;

#[derive(Debug)]
pub enum Error {
    InvalidBrightness(u16),
    InvalidFrequency(u16),
    InvalidAmplitude(u16),
    Io(std::io::Error),
    ModbusException { function: u8, code: u8 },
    InvalidResponse(Vec<u8>),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBrightness(value) => {
                write!(f, "invalid brightness {value}; expected 0..=1000")
            }
            Self::InvalidFrequency(value) => {
                write!(f, "invalid frequency {value}; expected 100..=4000")
            }
            Self::InvalidAmplitude(value) => {
                write!(f, "invalid amplitude {value}; expected 0..=1000")
            }
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::ModbusException { function, code } => {
                write!(
                    f,
                    "Modbus exception from function 0x{function:02X}: code 0x{code:02X}"
                )
            }
            Self::InvalidResponse(response) => {
                write!(f, "invalid Modbus response: {response:02X?}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionMode {
    MoveRight,
    MoveLeft,
    MoveForward,
    MoveBackward,
    MoveUpperRight,
    MoveUpperLeft,
    MoveLowerLeft,
    MoveLowerRight,
    Bounce,
    CenterHorizontal,
    CenterVertical,
    CenterDiagonalUp,
    CenterDiagonalDown,
}

impl MotionMode {
    fn motion_code(self) -> u16 {
        match self {
            Self::MoveRight => 1,
            Self::MoveLeft => 2,
            Self::MoveForward => 3,
            Self::MoveBackward => 4,
            Self::MoveUpperRight => 5,
            Self::MoveUpperLeft => 6,
            Self::MoveLowerLeft => 7,
            Self::MoveLowerRight => 8,
            Self::Bounce => 9,
            Self::CenterHorizontal => 10,
            Self::CenterVertical => 11,
            Self::CenterDiagonalUp => 12,
            Self::CenterDiagonalDown => 13,
        }
    }

    fn frequency_register(self) -> u16 {
        match self {
            Self::MoveRight => 4166,
            Self::MoveLeft => 4168,
            Self::MoveForward => 4170,
            Self::MoveBackward => 4172,
            Self::MoveUpperRight => 4174,
            Self::MoveUpperLeft => 4176,
            Self::MoveLowerLeft => 4178,
            Self::MoveLowerRight => 4180,
            Self::Bounce => 4182,
            Self::CenterHorizontal => 4184,
            Self::CenterVertical => 4186,
            Self::CenterDiagonalUp => 4188,
            Self::CenterDiagonalDown => 4190,
        }
    }

    fn amplitude_register(self) -> u16 {
        match self {
            Self::MoveRight => 4167,
            Self::MoveLeft => 4169,
            Self::MoveForward => 4171,
            Self::MoveBackward => 4173,
            Self::MoveUpperRight => 4175,
            Self::MoveUpperLeft => 4177,
            Self::MoveLowerLeft => 4179,
            Self::MoveLowerRight => 4181,
            Self::Bounce => 4183,
            Self::CenterHorizontal => 4185,
            Self::CenterVertical => 4187,
            Self::CenterDiagonalUp => 4189,
            Self::CenterDiagonalDown => 4191,
        }
    }
}

pub struct VibrationTable {
    stream: TcpStream,
    transaction_id: u16,
    unit_id: u8,
    control_word: u16,
}

impl VibrationTable {
    pub fn can_connect(addr: impl ToSocketAddrs) -> bool {
        Self::connect(addr).is_ok()
    }

    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(DEFAULT_TIMEOUT))?;
        stream.set_write_timeout(Some(DEFAULT_TIMEOUT))?;

        Ok(Self {
            stream,
            transaction_id: 1,
            unit_id: DEFAULT_UNIT_ID,
            control_word: 0,
        })
    }

    pub fn start_vibration(&mut self, mode: MotionMode) -> Result<()> {
        self.start_vibration_with_params(mode, DEFAULT_FREQUENCY, DEFAULT_AMPLITUDE)
    }

    pub fn start_vibration_with_params(
        &mut self,
        mode: MotionMode,
        frequency: u16,
        amplitude: u16,
    ) -> Result<()> {
        validate_frequency(frequency)?;
        validate_amplitude(amplitude)?;

        self.write_register(mode.frequency_register(), frequency)?;
        self.write_register(mode.amplitude_register(), amplitude)?;

        self.control_word &= !MOTION_BITS_MASK;
        self.control_word |= mode.motion_code();
        self.control_word |= BIT_MOTION_ENABLE;
        self.write_register(REG_CONTROL, self.control_word)
    }

    pub fn stop_vibration(&mut self) -> Result<()> {
        self.control_word &= !BIT_MOTION_ENABLE;
        self.control_word &= !MOTION_BITS_MASK;
        self.write_register(REG_CONTROL, self.control_word)
    }

    pub fn light_on(&mut self, brightness: u16) -> Result<()> {
        validate_brightness(brightness)?;

        self.write_register(REG_LIGHT1_BRIGHTNESS, brightness)?;
        self.control_word |= BIT_LIGHT1_ENABLE;
        self.write_register(REG_CONTROL, self.control_word)
    }

    pub fn light_off(&mut self) -> Result<()> {
        self.control_word &= !BIT_LIGHT1_ENABLE;
        self.write_register(REG_CONTROL, self.control_word)
    }

    pub fn read_faults(&mut self) -> Result<u16> {
        self.read_input_register(REG_FAULTS)
    }

    pub fn has_fault(&mut self) -> Result<bool> {
        Ok(self.read_faults()? != 0)
    }

    pub fn clear_faults(&mut self) -> Result<()> {
        self.write_register(REG_CONTROL, self.control_word | BIT_CLEAR_FAULTS)
    }

    fn read_input_register(&mut self, register: u16) -> Result<u16> {
        let payload = [(register >> 8) as u8, register as u8, 0x00, 0x01];
        let response = self.request(0x04, &payload)?;

        if response.len() < 11 || response[7] != 0x04 || response[8] != 2 {
            return Err(Error::InvalidResponse(response));
        }

        Ok(u16::from_be_bytes([response[9], response[10]]))
    }

    fn write_register(&mut self, register: u16, value: u16) -> Result<()> {
        let payload = [
            (register >> 8) as u8,
            register as u8,
            (value >> 8) as u8,
            value as u8,
        ];
        let response = self.request(0x06, &payload)?;

        if response.len() < 12 || response[7] != 0x06 {
            return Err(Error::InvalidResponse(response));
        }

        Ok(())
    }

    fn request(&mut self, function: u8, payload: &[u8]) -> Result<Vec<u8>> {
        let pdu_len = 1 + payload.len();
        let mbap_len = pdu_len + 1;
        let transaction_id = self.next_transaction();

        let mut frame = Vec::with_capacity(7 + pdu_len);
        frame.extend_from_slice(&transaction_id.to_be_bytes());
        frame.extend_from_slice(&0u16.to_be_bytes());
        frame.extend_from_slice(&(mbap_len as u16).to_be_bytes());
        frame.push(self.unit_id);
        frame.push(function);
        frame.extend_from_slice(payload);

        self.stream.write_all(&frame)?;

        let mut header = [0u8; 7];
        self.stream.read_exact(&mut header)?;

        let [
            transaction_hi,
            transaction_lo,
            protocol_hi,
            protocol_lo,
            length_hi,
            length_lo,
            _unit_id,
        ] = header;
        let response_transaction_id = u16::from_be_bytes([transaction_hi, transaction_lo]);
        let protocol_id = u16::from_be_bytes([protocol_hi, protocol_lo]);
        let response_len = u16::from_be_bytes([length_hi, length_lo]) as usize;

        if response_transaction_id != transaction_id || protocol_id != 0 || response_len == 0 {
            return Err(Error::InvalidResponse(header.to_vec()));
        }

        let mut body = vec![0u8; response_len - 1];
        self.stream.read_exact(&mut body)?;

        let mut response = header.to_vec();
        response.extend_from_slice(&body);

        if response.len() >= 9 && response[7] == function | 0x80 {
            return Err(Error::ModbusException {
                function: response[7],
                code: response[8],
            });
        }

        Ok(response)
    }

    fn next_transaction(&mut self) -> u16 {
        let current = self.transaction_id;
        self.transaction_id = self.transaction_id.wrapping_add(1);
        current
    }
}

fn validate_frequency(value: u16) -> Result<()> {
    if (100..=4000).contains(&value) {
        Ok(())
    } else {
        Err(Error::InvalidFrequency(value))
    }
}

fn validate_amplitude(value: u16) -> Result<()> {
    if value <= 1000 {
        Ok(())
    } else {
        Err(Error::InvalidAmplitude(value))
    }
}

fn validate_brightness(value: u16) -> Result<()> {
    if value <= 1000 {
        Ok(())
    } else {
        Err(Error::InvalidBrightness(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::thread::sleep;
    use std::time::Duration;

    const TEST_DEVICE_ADDR: &str = "192.168.3.7:8887";
    const TEST_DURATION: Duration = Duration::from_secs(2);

    static DEVICE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn connection_check() {
        let _guard = DEVICE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        assert!(VibrationTable::can_connect(TEST_DEVICE_ADDR));
    }

    #[test]
    fn light_control() {
        let _guard = DEVICE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let mut table = VibrationTable::connect(TEST_DEVICE_ADDR).unwrap();

        table.light_on(800).unwrap();
        sleep(TEST_DURATION);
        table.light_off().unwrap();
    }

    #[test]
    fn center_vibration() {
        let _guard = DEVICE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let mut table = VibrationTable::connect(TEST_DEVICE_ADDR).unwrap();

        table.start_vibration(MotionMode::CenterHorizontal).unwrap();
        sleep(TEST_DURATION);
        table.stop_vibration().unwrap();
    }
}
