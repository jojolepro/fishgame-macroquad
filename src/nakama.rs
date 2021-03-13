//! Nakama client
//! Right now supports only web and only very few nakama calls
//! Eventually going to be replaced with nakama crate

use nakama_sdk::*;
use std::sync::{Arc, Mutex};
use lazy_static::*;

pub struct MatchData {
    pub data: Vec<u8>,
    pub opcode: i32,
    pub user_id: String,
}

#[allow(dead_code)]
pub enum Event {
    Join(String),
    Leave(String),
}

lazy_static! {
    static ref CLIENT: Arc<Mutex<Option<NakamaClient>>> = Arc::new(Mutex::new(None));
    static ref RT_CLIENT: Arc<Mutex<Option<NakamaRealtimeClient>>> = Arc::new(Mutex::new(None));
}

#[cfg(target_arch = "wasm32")]
mod nakama {
    use super::{Event, MatchData};
    use sapp_jsutils::JsObject;

    extern "C" {
        fn nakama_connect(key: JsObject, server: JsObject, port: u32, protocol: JsObject);
        fn nakama_is_connected() -> bool;
        fn nakama_self_id() -> JsObject;
        fn nakama_send(opcode: i32, data: JsObject);
        fn nakama_try_recv() -> JsObject;
        fn nakama_events() -> JsObject;
    }

    #[no_mangle]
    pub extern "C" fn quad_nakama_crate_version() -> u32 {
        (0 << 24) + (1 << 16) + 1
    }

    pub fn connect(key: &str, server: &str, port: u32, protocol: &str) {
        unsafe {
            nakama_connect(
                JsObject::string(key),
                JsObject::string(server),
                port,
                JsObject::string(protocol),
            );
        }
    }

    pub fn connected() -> bool {
        unsafe { nakama_is_connected() }
    }

    pub fn self_id() -> String {
        let mut id = String::new();
        let js_obj = unsafe { nakama_self_id() };
        js_obj.to_string(&mut id);

        id
    }

    pub fn send(opcode: i32, data: &[u8]) {
        unsafe { nakama_send(opcode, JsObject::buffer(data)) }
    }

    pub fn send_bin<T: nanoserde::SerBin>(opcode: i32, data: &T) {
        use nanoserde::SerBin;

        send(opcode, &SerBin::serialize_bin(data));
    }

    pub fn try_recv() -> Option<MatchData> {
        let js_obj = unsafe { nakama_try_recv() };
        if js_obj.is_nil() == false {
            let mut buf = vec![];
            let mut user_id = String::new();

            let opcode = js_obj.field_u32("opcode") as i32;
            js_obj.field("data").to_byte_buffer(&mut buf);
            js_obj.field("user_id").to_string(&mut user_id);

            return Some(MatchData {
                opcode,
                user_id,
                data: buf,
            });
        }
        None
    }

    pub fn events() -> Option<Event> {
        let js_obj = unsafe { nakama_events() };
        if js_obj.is_nil() == false {
            let mut user_id = String::new();

            js_obj.field("user_id").to_string(&mut user_id);
            let event_type = js_obj.field_u32("event");

            match event_type {
                1 => return Some(Event::Join(user_id)),
                2 => return Some(Event::Leave(user_id)),
                _ => panic!("Unknown nakama event type"),
            }
        }
        None
    }
}

// just enough of stubs to run the game on PC, but no real networking involved
#[cfg(not(target_arch = "wasm32"))]
mod nakama {
    use nakama_sdk::*;
    use super::{Event, MatchData, CLIENT, RT_CLIENT};

    pub fn connect(key: &str, server: &str, port: u32, _protocol: &str) {
        println!("Connecting");
        *CLIENT.lock().unwrap() = Some(NakamaClient::new(key, server, port as u16, false));
        tick();
        tick();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        tick();
        // TODO check if we need the client to be logged in.
        auth_email(&mut CLIENT.lock().unwrap().as_mut().unwrap(), "email@example.com", "3bc8f72e95a9aaa", "mycustomusername");
        tick();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        tick();
        println!("Verifying client is connected");
        assert!(LAST_AUTH.lock().unwrap().is_some());

        println!("Creating Realtime Client...");
        *RT_CLIENT.lock().unwrap() = Some(NakamaRealtimeClient::new(CLIENT.lock().unwrap().as_mut().unwrap(), port as u16));
        tick();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        tick();
        RT_CLIENT.lock().unwrap().as_mut().unwrap().connect();
        tick();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        tick();
        println!("Ensure RtClient connected.");
        assert!(RT_CLIENT.lock().unwrap().as_mut().unwrap().is_connected());

        RT_CLIENT.lock().unwrap().as_mut().unwrap().match_make();
        tick();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        tick();
        println!("Ensuring we are in a match.");
        //assert!(MATCH.lock().unwrap().is_some());
    }

    pub fn self_id() -> String {
        "self".to_string()
    }

    pub fn send_bin<T: nanoserde::SerBin>(opcode: i32, data: &T) {
        println!("Send bin data");
        if let Some(game) = nakama_sdk::MATCH.lock().unwrap().as_mut() {
            if let Some(rt_client) = RT_CLIENT.lock().unwrap().as_mut() {
                println!("Beep boop");
                println!("Serializing data");
                let data = data.serialize_bin();
                println!("Sending data");
                game.send_data(rt_client, opcode as i64, data);
            }
        }
    }

    pub fn tick() {
        if let Some(client) = CLIENT.lock().unwrap().as_mut() {
            client.tick();
        }
        if let Some(client) = RT_CLIENT.lock().unwrap().as_mut() {
            client.tick();
        }
    }

    pub fn try_recv() -> Option<MatchData> {
        tick();
        let mut events = RECEIVED_DATA.lock().unwrap();
        if events.len() > 0 {
            println!("Received bin data");
            let first = events.pop().unwrap();
            return Some(MatchData {
                data: first.1,
                opcode: first.0 as i32,
                user_id: self_id(), // TODO
            });
        }
        None
    }

    pub fn events() -> Option<Event> {
        let mut lock = nakama_sdk::RECEIVED_DATA.lock().unwrap();
        if lock.len() > 0 {
            println!("Received event");
            let ev = lock.pop().unwrap();
            let id = String::from("tmp_user_id");
            match ev.0 {
                1 => return Some(Event::Join(id)),
                2 => return Some(Event::Leave(id)),
                _ => panic!("Unknown nakama event type"),
            }
        }
        None
    }
}

pub use nakama::*;
