//! The audited subset of mpv/client.h API 2.x used by this experiment.
use libloading::Library;
use std::ffi::{c_char, c_int, c_ulong, c_void};

#[repr(C)]
pub struct RawEvent {
    pub id: c_int,
    pub error: c_int,
    pub userdata: u64,
    pub data: *mut c_void,
}
#[repr(C)]
pub struct Property {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}
#[repr(C)]
pub struct EndFile {
    pub reason: c_int,
    pub error: c_int,
    pub entry_id: i64,
    pub insert_id: i64,
    pub insert_count: c_int,
}

pub struct Api {
    // Must stay loaded until every handle has been destroyed.
    _library: Library,
    pub version: unsafe extern "C" fn() -> c_ulong,
    pub create: unsafe extern "C" fn() -> *mut c_void,
    pub destroy: unsafe extern "C" fn(*mut c_void),
    pub initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int,
    pub get: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int,
    pub set: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int,
    pub string: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub free: unsafe extern "C" fn(*mut c_void),
    pub observe: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    pub event: unsafe extern "C" fn(*mut c_void, f64) -> *const RawEvent,
}

impl Api {
    pub fn load() -> Result<Self, ()> {
        let names: &[&str] = if cfg!(target_os = "windows") {
            &["mpv-2.dll", "libmpv-2.dll"]
        } else if cfg!(target_os = "macos") {
            &["libmpv.2.dylib"]
        } else {
            &["libmpv.so.2"]
        };
        let library = names
            .iter()
            .find_map(|name| {
                // Loading a system library executes its initializers. This experiment
                // accepts only named SDK libraries from the OS loader's search path.
                unsafe { Library::new(*name).ok() }
            })
            .ok_or(())?;
        // Symbols are typed exactly as the upstream C header. Copying pointers is
        // safe here because Api owns the Library for their entire lifetime.
        unsafe {
            Ok(Self {
                version: *library.get(b"mpv_client_api_version\0").map_err(|_| ())?,
                create: *library.get(b"mpv_create\0").map_err(|_| ())?,
                destroy: *library.get(b"mpv_terminate_destroy\0").map_err(|_| ())?,
                initialize: *library.get(b"mpv_initialize\0").map_err(|_| ())?,
                option: *library.get(b"mpv_set_option_string\0").map_err(|_| ())?,
                command: *library.get(b"mpv_command\0").map_err(|_| ())?,
                get: *library.get(b"mpv_get_property\0").map_err(|_| ())?,
                set: *library.get(b"mpv_set_property\0").map_err(|_| ())?,
                string: *library.get(b"mpv_get_property_string\0").map_err(|_| ())?,
                free: *library.get(b"mpv_free\0").map_err(|_| ())?,
                observe: *library.get(b"mpv_observe_property\0").map_err(|_| ())?,
                event: *library.get(b"mpv_wait_event\0").map_err(|_| ())?,
                _library: library,
            })
        }
    }
}
