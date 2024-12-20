use core::fmt::Write;
use core::str::FromStr;
use core::{cell::RefCell, str};
use cortex_m_semihosting::hprintln;
use heapless::{String, Vec};
use stm32f1xx_hal::prelude::_fugit_ExtU32;
use stm32f1xx_hal::{
    gpio::{Output, PushPull, PA4},
    serial::{Instance, Rx, Tx},
    timer::Counter,
};

#[derive(Debug)]
pub enum Error {
    Failure,
    NoResponse,
}

#[derive(Debug)]
pub enum CipStatus {
    WifiUninitialized,  //0
    WifiDisconnected,   //1
    WifiConnected,      //2
    ServerConnected,    //3
    ServerDisconnected, //4
    WifiConnectFailed,  //5
}

const ESP_RX_BUFF_SIZE: usize = 256;
const ESP_TX_BUFF_SIZE: usize = 256;

pub const MAX_STRING_SIZE: usize = 1024;
const RECEIVE_PIECE_LEN: usize = 8;

// ESP8266 结构体定义
pub struct ESP8266<'a, USART, TIM, const FREQ: u32> {
    tx: Tx<USART>,
    rx: Rx<USART>,
    power_pin: PA4<Output<PushPull>>,
    timer: &'a mut Counter<TIM, FREQ>,
}

impl<'a, USART, TIM, const FREQ: u32> ESP8266<'a, USART, TIM, FREQ>
where
    USART: stm32f1xx_hal::serial::Instance,
    TIM: stm32f1xx_hal::timer::Instance,
{
    pub fn new(mut tx: Tx<USART>, mut rx: Rx<USART>, power_pin: PA4<Output<PushPull>>, timer: &'a mut Counter<TIM, FREQ>) -> Self {
        rx.listen();
        Self { tx, rx, power_pin, timer }
    }

    pub fn power_on(&mut self) {
        self.power_pin.set_high();
    }

    pub fn power_off(&mut self) {
        self.power_pin.set_low();
    }

    pub fn send(&mut self, command: &str) -> bool {
        let mut base: String<4096> = String::new();
        base.push_str(command).unwrap();
        base.push_str("\r\n").unwrap();
        let command = base.as_str();
        self.tx.write_str(command).unwrap();
        true
    }

    pub fn test(&mut self) -> Result<bool, Error> {
        self.send("AT");
        let receive = self.recv_string("OK")?;
        Ok(receive == "OK")
    }

    pub fn restart(&mut self) -> Result<bool, Error> {
        self.send("AT+RST");
        let receive = self.recv_string_with_flag("ready", 5000_u32)?;
        let receive = Self::remove_first_line(receive.trim_end());
        hprintln!("restart receive:{}", receive);
        Ok(&receive[..2] == "OK")
    }

    pub fn set_mode(&mut self, mode: u8) -> Result<bool, Error> {
        let mut command = String::<ESP_TX_BUFF_SIZE>::new();
        write!(command, "AT+CWMODE={}", mode).ok();
        self.send(&command);
        let receive = self.recv_string_with_flag("OK", 3000_u32)?;
        hprintln!("set_mode receive:{}", receive);
        Ok(receive == "OK")
    }

    pub fn set_cip_mode(&mut self, mode: u8) -> Result<bool, Error> {
        let mut command = String::<ESP_TX_BUFF_SIZE>::new();
        write!(command, "AT+CIPMODE={}", mode).ok();
        self.send(&command);
        let receive = self.recv_string_with_flag("OK", 3000_u32)?;
        hprintln!("set_mode receive:{}", receive);
        Ok(receive == "OK")
    }

    pub fn connect_server(&mut self, mode: &str, ip: &str, port: u16) -> Result<bool, Error> {
        let mut command = String::<ESP_TX_BUFF_SIZE>::new();
        write!(command, "AT+CIPSTART=\"{}\",\"{}\",{}", mode, ip, port).ok();
        self.send(&command);
        let receive = self.recv_string_with_flag("OK", 5000_u32)?;
        let receive = Self::remove_first_line(receive.trim_end());
        Ok(receive.ends_with("OK"))
    }

    pub fn cip_send(&mut self, data: &str) -> Result<String<MAX_STRING_SIZE>, Error> {
        let len = data.len() + 2;
        let mut command = String::<ESP_TX_BUFF_SIZE>::new();
        write!(command, "AT+CIPSEND={}", len).ok();
        self.send(&command);
        let receive = self.recv_string_with_flag(">", 3000_u32)?;
        if !receive.ends_with(">") {
            return Err(Error::Failure);
        }
        // self.timer.start(1000.millis()).unwrap();
        // self.timer.wait().unwrap_or_default();
        self.send(data);
        let receive = self.recv_string_with_timeout(2000_u32)?;
        for line in receive.lines() {
            if line.starts_with("+IPD") {
                if let Some(pos) = line.find(':') {
                    let data_str = &line[pos + 1..line.len()];
                    hprintln!("receive:{}", data_str);
                    return Ok(String::from_str(data_str).unwrap());
                }
            }
        }
        Err(Error::NoResponse)
    }

    pub fn cip_receive(&mut self) -> Result<String<MAX_STRING_SIZE>, Error> {
        let receive = self.recv_string_with_timeout(1000_u32)?;
        for line in receive.lines() {
            if line.starts_with("+IPD") {
                if let Some(pos) = line.find(':') {
                    let data_str = &line[pos + 1..line.len()];
                    hprintln!("receive:{}", data_str);
                    return Ok(String::from_str(data_str).unwrap());
                }
            }
        }
        Err(Error::NoResponse)
    }

    pub fn cip_status(&mut self) -> Result<CipStatus, Error> {
        self.send("AT+CIPSTATUS");
        let receive = self.recv_string_with_flag("OK", 3000_u32)?;
        for line in receive.lines() {
            if line.starts_with("STATUS:") {
                if let Some(pos) = line.find(':') {
                    let data_str = &line[pos + 1..line.len()];
                    //hprintln!("CIPSTATUS:{}", data_str);
                    let cip_status = match data_str {
                        "0" => CipStatus::WifiUninitialized,
                        "1" => CipStatus::WifiDisconnected,
                        "2" => CipStatus::WifiConnected,
                        "3" => CipStatus::ServerConnected,
                        "4" => CipStatus::ServerDisconnected,
                        "5" => CipStatus::WifiConnectFailed,
                        _ => panic!("error")
                    };
                    return Ok(cip_status);
                }
            }
        }
        Err(Error::NoResponse)
    }

    pub fn set_auto_join_ap(&mut self, mode: u8) -> Result<bool, Error> {
        let mut command = String::<ESP_TX_BUFF_SIZE>::new();
        write!(command, "AT+CWAUTOCONN={}", mode).ok();
        self.send(&command);
        let receive = self.recv_string_with_flag("OK", 3000_u32)?;
        Ok(receive == "OK")
    }

    pub fn join_ap(&mut self, ssid: &str, password: &str) -> Result<bool, Error> {
        let mut command = String::<ESP_TX_BUFF_SIZE>::new();
        write!(command, "AT+CWJAP=\"{}\",\"{}\"", ssid, password).ok();
        self.send(&command);
        let receive = self.recv_string_with_flag("OK", 5000_u32)?;
        Ok(receive.ends_with("OK"))
    }

    pub fn recv_string(&mut self, success_flag: &str) -> Result<String<1024>, Error> {
        return self.recv_string_with_flag(success_flag, 1000_u32);
    }

    pub fn recv_string_with_timeout(&mut self, timeout: u32) -> Result<String<MAX_STRING_SIZE>, Error> {
        return self.recv_string_with_flag("", timeout);
    }

    pub fn recv_string_with_flag(&mut self, success_flag: &str, timeout: u32) -> Result<String<MAX_STRING_SIZE>, Error> {
        let with_flag = !success_flag.is_empty();
        let mut piece: [u8; RECEIVE_PIECE_LEN] = [b'\0'; RECEIVE_PIECE_LEN];
        let piece_last_index = RECEIVE_PIECE_LEN - 1;
        let timer = &mut self.timer;
        let mut result: String<MAX_STRING_SIZE> = String::new();
        timer.start(timeout.millis()).unwrap();
        let mut receive_started = false;
        loop {
            if let Ok(byte) = self.rx.read() {
                let c = byte as char;
                result.push(c).unwrap();
                for i in 0..piece_last_index {
                    piece[i] = piece[i + 1];
                }
                piece[piece_last_index] = byte;
                receive_started = true;
            } else {
                if receive_started {
                    if let Ok(piece_str) = str::from_utf8(&piece) {
                        if with_flag && piece_str.ends_with(success_flag) {
                            timer.cancel().unwrap();
                            break;
                        }
                        if piece_str.ends_with("Error") {
                            timer.cancel().unwrap();
                            return Err(Error::Failure);
                        }
                    }
                }
                if timer.wait().is_ok() {
                    if with_flag {
                        return Err(Error::NoResponse);
                    } else {
                        break;
                    }
                }
            }
        }
        let result_str = result.as_str();
        let result_str: &str = Self::remove_first_line(result_str.trim_end());
        Ok(String::from_str(result_str).unwrap())
    }

    fn remove_first_line(input: &str) -> &str {
        if let Some(pos) = input.find('\n') {
            let result = &input[pos + 1..];
            result.trim()
        } else {
            ""
        }
    }
}
