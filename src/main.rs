use serde::{Serialize,Deserialize};
use serde_json;
use serial::SerialPort;
use std::fs::File;
use std::io::{self, prelude::*};
use std::time::SystemTime;
use std::io::BufReader;

// Node-REDから実行するのを想定しているので、絶対パスで指定してください
const LOG_FILE_PATH: &str = "/home/user/udco2s.json";
const ALEXA_REMOTE_CONTROL_SH_PATH: &str = "/home/hogehoge/alexa-remote-control/alexa_remote_control.sh";
const YOUR_ALEXA_DEVICE_NAME: &str = "hogehogeさんの Echo Dot";

// 以下は基本的に変更する必要はありません
const DEVICE_PATH: &str = "/dev/ttyACM0";

#[derive(Serialize,Deserialize)]
struct UDCO2SStat {
    co2ppm: i32,
    humidity: f32,
    temperature: f32,
}

#[derive(Serialize,Deserialize)]
struct Log{
    time: i64,
    status: UDCO2SStat
}

impl UDCO2SStat {
    fn new(co2ppm: i32, humidity: f32, temperature: f32) -> Self {
        UDCO2SStat {
            co2ppm,
            humidity,
            temperature,
        }
    }
}

pub struct UDCO2S {
    dev: String,
}

impl UDCO2S {
    pub fn new(dev: &str) -> Self {
        UDCO2S {
            dev: dev.into(),
        }
    }
    
    pub fn start_logging(&self, log_file: &str) -> io::Result<()> {
        let regex = regex::Regex::new(r"CO2=(?P<co2>\d+),HUM=(?P<hum>\d+\.\d+),TMP=(?P<tmp>-?\d+\.\d+)").unwrap();
        
        let mut port = serial::open(&self.dev).unwrap();

        let option_func = &|settings: &mut dyn serial::SerialPortSettings|{
            _ = settings.set_baud_rate(serial::Baud115200);
            settings.set_char_size(serial::Bits8);
            settings.set_parity(serial::ParityNone);
            settings.set_stop_bits(serial::Stop1);
            settings.set_flow_control(serial::FlowNone);
            Ok(())
        };

        _ = port.reconfigure(option_func);
        _ = port.set_timeout(std::time::Duration::from_secs(6));
        
        write!(&mut port, "STA\r\n")?;
        print!("{}", read_until(&mut port, '\n')?); // Print the first line
        
        if let Ok(line) = read_until(&mut port, '\n') {
            if let Some(m) = regex.captures(&line) {
                let time_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;

                let obj=Log{
                    time : time_now,
                    status:UDCO2SStat::new(
                        m["co2"].parse::<i32>().unwrap(),
                        m["hum"].parse::<f32>().unwrap(),
                        m["tmp"].parse::<f32>().unwrap()
                )};

                println!("{}", m["co2"].parse::<i32>().unwrap());
                
                let mut file = File::create(log_file)?;
                file.write_all(serde_json::to_string(&obj).unwrap().as_bytes())?;
            }
        }
        
        write!(&mut port, "STP\r\n")?;

        Ok(())
    }
}

fn read_until(port: &mut dyn serial::SerialPort, del: char) -> io::Result<String> {
    let mut res = String::new();
    loop {
        let mut buf = [0u8; 1];
        match port.read_exact(&mut buf) {
            Ok(_) => {
                let ch = char::from(buf[0]);
                if ch == del {
                    return Ok(res);
                } else {
                    res.push(ch);
                }
            }
            Err(e) => match e.kind() {
                io::ErrorKind::TimedOut => return Ok(res),
                _ => return Err(e.into()),
            },
        }
    }
}

fn main() {
    let sensor = UDCO2S::new(DEVICE_PATH);
    _ = sensor.start_logging(LOG_FILE_PATH);
    let file = File::open(LOG_FILE_PATH).unwrap();
    let reader = BufReader::new(file);
    let deserialized: Log = serde_json::from_reader(reader).unwrap();
    println!("{}",deserialized.status.co2ppm);

    use std::process::Command;
    let co2 = &deserialized.status.co2ppm.to_string();
    Command::new(ALEXA_REMOTE_CONTROL_SH_PATH)
    .args(&["-d", YOUR_ALEXA_DEVICE_NAME , "-e", &format!("speak: {co2}ppmです")])
    .output()
    .expect("failed to start `alexa_remote_control.sh`");
}
