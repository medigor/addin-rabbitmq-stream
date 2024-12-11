mod addin_consumer;
mod addin_producer;
mod addin_environment;
mod environments;

use std::{
    ffi::{c_int, c_long, c_void},
    sync::atomic::{AtomicI32, Ordering},
};

use addin1c::{create_component, destroy_component, name, AttachType};

pub static PLATFORM_CAPABILITIES: AtomicI32 = AtomicI32::new(-1);

/// # Safety
/// Component must be non-null.
#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "C" fn GetClassObject(name: *const u16, component: *mut *mut c_void) -> c_long {
    match *name as u8 {
        b'0' => {
            let addin = addin_producer::AddinProducer::new();
            create_component(component, addin)
        }
        b'1' => {
            let addin = addin_consumer::AddinConsumer::new();
            create_component(component, addin)
        }
        b'2' => {
            let addin = addin_environment::AddinEnvironment::new();
            create_component(component, addin)
        }
        _ => 0,
    }
}

/// # Safety
/// Component must be returned from `GetClassObject`, the function must be called once for each component.
#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "C" fn DestroyObject(component: *mut *mut c_void) -> c_long {
    destroy_component(component)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn GetClassNames() -> *const u16 {
    name!("0|1|2").as_ptr()
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn SetPlatformCapabilities(capabilities: c_int) -> c_int {
    PLATFORM_CAPABILITIES.store(capabilities, Ordering::Relaxed);
    3
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn GetAttachType() -> AttachType {
    AttachType::Any
}
