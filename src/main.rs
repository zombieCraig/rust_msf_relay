#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate socketcan;
extern crate clap;
extern crate libc;
extern crate time;
extern crate chrono;
extern crate hex;
#[macro_use] extern crate serde_derive;

use rocket::config::Config;
use rocket_contrib::Json;
use rocket::State;
use std::sync::RwLock;
use chrono::prelude::*;
use socketcan::{CANSocket, CANFrame, CANFilter};
use clap::{Arg, App};
use hex::FromHex;

const VERSION: &str = "0.0.1";

static CTL_KERN: libc::c_int = 1;
static KERN_BOOTTIME: libc::c_int = 21;

// Global state information
struct RelayState {
  started_on: i64,
  packets_sent: u32,
  last_packet_sent: Option<i64>,
  available_sockets: Vec<String>
}

impl RelayState {
  pub fn sent_packet(&mut self) {
    self.packets_sent = self.packets_sent.checked_add(1).unwrap();
    self.last_packet_sent = Some(time::now().to_timespec().sec);
  }

}

#[derive(Serialize)]
struct RelayStatus {
  status: String
}

#[derive(Serialize)]
struct SuccessStatus {
  success: bool
}

#[derive(Serialize)]
struct HWSpecialty {
  automotive: bool
}

#[derive(Serialize)]
struct HWCapability {
  can: bool
}

#[derive(Serialize)]
struct Status {
  operational: u8,
  hw_speciality: HWSpecialty,
  hw_capabilities: HWCapability,
  api_version: &'static str,
  fw_version: &'static str,
  hw_version: &'static str,
  device_name: &'static str
}

#[derive(Serialize)]
struct Stats {
  uptime: i64,
  packet_stats: u32,
  last_request: i64,
  voltage: f32
}

#[derive(Serialize)]
struct SystemDatetime {
  system_datetime: i64
}

#[derive(Serialize)]
struct SystemTimezone {
  system_timezone: String
}

#[derive(Serialize)]
struct BusName {
  bus_name: String
}

#[derive(FromForm)]
struct CansendData {
  id: String,
  data: String
}

#[derive(FromForm)]
struct ISOTPData {
  srcid: String,
  dstid: String,
  data: String,
  timeout: Option<u32>,
  maxpkts: Option<u32>,
}

#[derive(Serialize)]
struct Packets {
  success: Option<bool>,
  packets: Vec<CanData>
}

impl Packets {
  pub fn new() -> Packets {
    Packets { success: None, packets: Vec::new(), }
  }

  pub fn add_frame(&mut self, frame: CANFrame) {
    let id = format!("{:X}", frame.id());
    let mut data = Vec::new();
    for byte in frame.data() {
      data.push(format!("{:X}", byte));
    }
    let packet = CanData { id: id, data: data };
    self.packets.push(packet);
  }
}

#[derive(Serialize)]
struct CanData {
  id: String,
  data: Vec<String>
}

fn getboottime() -> libc::timeval {
    let mut mib = [CTL_KERN, KERN_BOOTTIME];
    let mut boottime = libc::timeval {
        tv_sec: 0,
        tv_usec: 0
    };
    let mut size: libc::size_t = std::mem::size_of_val(&boottime) as libc::size_t;
    unsafe {
        libc::sysctl(&mut mib[0], 2,
               &mut boottime as *mut libc::timeval as *mut libc::c_void,
               &mut size, std::ptr::null_mut(), 0);
    }
    boottime
}

#[get("/status")]
fn status() -> Json<Status> {
  Json(Status {
    operational: 1,
    hw_speciality: HWSpecialty { automotive: true },
    hw_capabilities: HWCapability { can: true },
    api_version: "0.0.3",
    fw_version: VERSION,
    hw_version: "0.0.1",
    device_name: "Rust MSFRelay"
  })
}

#[get("/statistics")]
fn statistics(state: State<RwLock<RelayState>>) -> Json<Stats> {
  let state = state.read().unwrap();
  Json(Stats {
    //uptime: time::now().to_timespec().sec - getboottime().tv_sec,
    uptime: time::now().to_timespec().sec - state.started_on,
    packet_stats: state.packets_sent,
    last_request: state.last_packet_sent.unwrap(),
    voltage: 0.0
  })
}

#[get("/settings/datetime")]
fn datetime() -> Json<SystemDatetime> {
  Json(SystemDatetime {
    system_datetime: time::now().to_timespec().sec
  })
}

#[get("/settings/timezone")]
fn timezone() -> Json<SystemTimezone> {
  Json(SystemTimezone {
    system_timezone: Local::now().format("%Z").to_string()
  })
}

#[get("/automotive/supported_buses")]
fn supported_buses(state: State<RelayState>) -> Json<Vec<BusName>> {
  Json(state.available_sockets
	.iter()
	.map(|s| { BusName { bus_name: s.to_string() } })
	.collect::<Vec<_>>())
}

#[get("/automotive/<bus_name>/cansend?<candata>")]
fn get_cansend(state: State<RwLock<RelayState>>, bus_name: String, candata: CansendData) -> Json<SuccessStatus> {
  let mut success = SuccessStatus { success: false };
  let id = u32::from_str_radix(&candata.id, 16).unwrap();
  success.success = cansend(bus_name, id, candata.data);
  if success.success {
    let mut state = state.write().unwrap();
    state.sent_packet();
  }
  Json(success)
}

#[get("/automotive/<bus_name>/isotpsend_and_wait?<isotp_data>")]
fn isotpsend_and_wait(state: State<RwLock<RelayState>>, bus_name: String, isotp_data: ISOTPData) -> Json<Packets> {
  let mut packets = Packets::new();
  packets.success = Some(false);
  let soc = match CANSocket::open(&bus_name) {
    Ok(s) => s,
    Err(_e) => return Json(packets),
  };
  let srcid = u32::from_str_radix(&isotp_data.srcid, 16).unwrap();
  let dstid = u32::from_str_radix(&isotp_data.dstid, 16).unwrap();
  let frame_data = match Vec::from_hex(&isotp_data.data) {
    Ok(d) => d,
    Err(_e) => return Json(packets),
  };
  let filter = CANFilter::new(dstid, 0x7FF).unwrap();
  if soc.set_filter(&[filter]).is_err() {
    return Json(packets)
  };
  let timeout = match isotp_data.timeout {
    None => 1500,
    Some(t) => t
  };
  let maxpkts = match isotp_data.maxpkts {
    None => 3,
    Some(p) => p
  };
  let frame = match CANFrame::new(srcid, &frame_data, false, false) {
    Ok(f) => f,
    Err(_e) => return Json(packets),
  };
  let result = soc.write_frame_insist(&frame);
  if result.is_err() {
    return Json(packets)
  } else {
    let mut state = state.write().unwrap();
    state.sent_packet();
  };
  let started = time::now().to_timespec().sec;
  let mut done = false;
  let mut current_count = 0;
  while !done {
    let pkt = soc.read_frame().unwrap();
    current_count += 1;
    packets.add_frame(pkt);
    if current_count >= maxpkts || (time::now().to_timespec().sec - started) >= timeout as i64 {
      done = true;
    }
  }
  if soc.filter_accept_all().is_err() {
    return Json(packets)
  };
  packets.success = Some(true);
  Json(packets) 
}

#[error(404)]
fn not_supported(_req: &rocket::Request) -> Json<RelayStatus> {
  Json(RelayStatus {
    status: "not supported".to_string()
  })
}

fn cansend(bus_name: String, id: u32, data: String) -> bool {
  let soc = match CANSocket::open(&bus_name) {
    Ok(s) => s,
    Err(_e) => return false,
  };
  let frame_data = match Vec::from_hex(&data) {
    Ok(d) => d,
    Err(_e) => return false,
  };
  let frame = match CANFrame::new(id, &frame_data, false, false) {
    Ok(f) => f,
    Err(_e) => return false,
  };
  let result = soc.write_frame_insist(&frame);
  result.is_ok()
}

fn main() {
    // Grab command line arguments
    let matches = App::new("MSF Relay")
	.version(VERSION)
	.author("Craig Smith <agent.craig@gmail.com>")
	.about("Rust implementation of the Metasploit Hardware Bridge")
	.arg(Arg::with_name("port")
		.short("p")
		.long("port")
		.value_name("PORT")
		.help("Sets the web server port")
		.takes_value(true))
	.arg(Arg::with_name("sockets")
		.multiple(true)
		.help("List of CAN sockets, can0, vcan0, etc")
		.required(true))
	.get_matches();

    // Grab the CAN sockets
    let socket_strs: Vec<&str> = matches.values_of("sockets").unwrap().collect();
    println!("Using sockets: {}", socket_strs.join(" "));

    // Create a String version of sockets
    let sockets = socket_strs
	.iter()
	.map(|s| { s.to_string() })
	.collect::<Vec<_>>();

    // Define the state to track
    let state = RelayState {
      started_on: time::now().to_timespec().sec,
      packets_sent: 0,
      last_packet_sent: None,
      available_sockets: sockets
    };

    let mut port = 8080;
    if let Some(p) = matches.value_of("port") {
      port = p.parse().unwrap();
    }
    // TODO: Figure out how to actually use this configuration at launch...
    let _config = Config::build(rocket::config::Environment::Development).port(port).unwrap();

    // Launch the web server
    rocket::ignite()
	.manage(RwLock::new(state))
	.mount("/", routes![status, statistics, datetime, timezone, supported_buses, get_cansend, isotpsend_and_wait])
	.catch(errors![not_supported])
	.launch();
}
