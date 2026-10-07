use napi::Env;
use napi_derive::napi;
use std::sync::Arc;
use zinc_core::SharedRegion;

#[napi]
pub struct ZincRegion(Arc<SharedRegion>);

#[napi]
impl ZincRegion {
    #[napi(factory)]
    pub fn create(name: String, capacity: f64) -> napi::Result<Self> {
        SharedRegion::create(&name, u32_argument(capacity, "capacity")? as usize)
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
    pub fn wait(&self, timeout_ms: f64) -> napi::Result<bool> {
        match self.0.wait(u32_argument(timeout_ms, "timeout")?) {
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

fn u32_argument(value: f64, name: &str) -> napi::Result<u32> {
    if !value.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(napi::Error::from_reason(format!("invalid {name}")));
    }
    Ok(value as u32)
}
