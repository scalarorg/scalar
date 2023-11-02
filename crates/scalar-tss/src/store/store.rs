use types::{message_out::keygen_result::KeygenResultData, KeygenOutput};

#[derive(Clone, Default)]
pub struct TssStore {
    key_data: Option<KeygenOutput>,
}

impl TssStore {
    pub fn new() -> Self {
        Self { key_data: None }
    }
    pub fn set_key(&mut self, key_data: KeygenOutput) {
        self.key_data = Some(key_data);
    }
    pub fn get_key(&self) -> Option<KeygenOutput> {
        self.key_data.clone()
    }
}
