use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref DATA_INDICES_MAP: Mutex<HashMap<&'static str, usize>> = Mutex::new(HashMap::new());
}

pub fn set_named_value(name: &'static str, value: usize) {
    let mut map = DATA_INDICES_MAP.lock().unwrap();
    map.insert(name, value);
}

pub fn get_and_increment_named_value(name: &str) -> usize {
    let mut map = DATA_INDICES_MAP.lock().unwrap();
    let index = map.get_mut(name).unwrap();
    *index = *index + 1;
    *index - 1
}