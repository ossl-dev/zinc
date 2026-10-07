use napi::Env;
use napi_derive::napi;
use std::sync::Arc;
use zinc_core::SharedRegion;

#[napi]
pub struct ZincRegion(Arc<SharedRegion>);

#[napi]
impl ZincRegion {
    #[napi(factory)]
    pub fn create(name: String, capacity: u32) -> napi::Result<Self> {
        SharedRegion::create(&name, capacity as usize)
            .map(|region| Self(Arc::new(region)))
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi(factory)]
    pub fn open(name: String) -> napi::Result<Self> {
        SharedRegion::open(&name)
            .map(|region| Self(Arc::new(region)))
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn as_buffer(&self, env: Env) -> napi::Result<napi::JsBuffer> {
        let ptr = self.0.as_ptr();
        let len = self.0.capacity();
        unsafe {
            env.create_buffer_with_borrowed_data(ptr, len, Arc::clone(&self.0), |region, _| {
                drop(region)
            })
        }
        .map(|b| b.into_raw())
    }

    #[napi]
    pub fn notify(&self) {
        self.0.notify();
    }

    #[napi]
    pub fn wait(&self, timeout_ms: u32) -> napi::Result<bool> {
        match self.0.wait(timeout_ms) {
            Ok(()) => Ok(true),
            Err(zinc_core::ZincError::TimedOut) => Ok(false),
            Err(error) => Err(napi::Error::from_reason(error.to_string())),
        }
    }

    #[napi]
    pub fn try_wait(&self) -> bool {
        self.0.try_wait()
    }
}
