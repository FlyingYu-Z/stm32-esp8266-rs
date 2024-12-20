# stm32-esp8266-rs
A driver lib for ESP01 (ESP8266).

add dependency to your Cargo.toml
```
stm32-esp8266-rs = { git = "https://github.com/FlyingYu-Z/stm32-esp8266-rs.git", branch = "main"}
```

import:

```rust
use stm32_esp8266_rs::{CipStatus, ESP8266};
```

# Example
```rust
    let mut delay = syst.delay(&clocks);
    let mut counter4: Counter<TIM4, 1000> = dp.TIM4.counter_ms(&clocks);
    // USART2
    let tx = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
    let rx = gpioa.pa3;
    let serial = Serial::new(
        dp.USART2, (tx, rx), 
        &mut afio.mapr, 
        serial::Config::default()
            .baudrate(115200.bps())
            .stopbits(serial::StopBits::STOP1)
            .wordlength_8bits()
            .parity_none(), 
        &clocks);
    let (mut tx, mut rx) = serial.split();
    let mut esp8266 = ESP8266::new(
        tx, rx, 
        gpioa.pa4.into_push_pull_output(&mut gpioa.crl), 
        &mut counter4
    );
    esp8266.power_on();
    delay.delay_ms(2000_u16);

    while esp8266.test().is_err() {
        hprintln!("retrying test");
        delay.delay_ms(2000_u16);
    }
    while esp8266.restart().is_err() {
        hprintln!("retrying restart");
        delay.delay_ms(2000_u16);
    }
    delay.delay_ms(5000_u16);
    while esp8266.set_mode(1).is_err() {
        hprintln!("retrying set_mode");
        delay.delay_ms(2000_u16);
    }

    loop {
        let cip_status = esp8266.cip_status();
        if cip_status.is_err() {
            hprintln!("wifi module error");
            continue;
        }

        match cip_status.unwrap() {
            CipStatus::WifiUninitialized => {
                hprintln!("wifi module uninitialized");
                continue;
            }
            CipStatus::WifiConnectFailed | CipStatus::WifiDisconnected => {
                while esp8266.join_ap("Your SSID", "Your password").is_err() {
                    delay.delay_ms(2000_u16);
                    hprintln!("retrying connect wifi");
                }
                hprintln!("successfully connected wifi");
            }
            CipStatus::ServerConnected => {
                let receive = esp8266.cip_receive();
                if receive.is_ok() {
                    // do something
                }
            }
            CipStatus::WifiConnected | CipStatus::ServerDisconnected => {
                while esp8266.connect_server("TCP", "bemfa.com", 8344).is_err() {
                    delay.delay_ms(2000_u16);
                    hprintln!("retrying connect server");
                }
                delay.delay_ms(1000_u16);
                send_data(&mut esp8266, &wrap_data(1, false, ""));
            }
        }

    }

```