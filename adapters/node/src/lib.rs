use napi_derive::napi;
use zinc_core::SharedRegion;

#[napi]
pub struct ZincRegion(SharedRegion);

#[napi]
impl ZincRegion {
    #[napi(factory)]
    pub fn create(name: String, capacity: u32) -> napi::Result<Self> {
        SharedRegion::create(&name, capacity as usize)
            .map(Self)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi(factory)]
    pub fn open(name: String) -> napi::Result<Self> {
        SharedRegion::open(&name)
            .map(Self)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn as_buffer(&self, env: napi::Env) -> napi::Result<napi::JsBuffer> {
        let ptr = self.0.as_ptr();
        let len = self.0.capacity();
        unsafe {
            env.create_buffer_with_borrowed_data(ptr, len, ptr as usize, |_, _| {})
        }
        .map(|b| b.into_raw())
    }

    #[napi]
    pub fn notify(&self) {
        self.0.notify();
    }

    #[napi]
    pub fn wait(&self, timeout_ms: u32) -> bool {
        self.0.wait(timeout_ms).is_ok()
    }
}
