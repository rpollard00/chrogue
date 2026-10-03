//! The C interface of the core, for a client that loads the core into its own process: the
//! desktop client through the FFI of LuaJIT, and the WebAssembly build of LÖVE as a linked
//! library. `include/chrogue_core.h` is the contract. The requests and the responses are the
//! lines of `core/PROTOCOL.md`; this crate only moves them, as `chrogue-core` does for a socket.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use chrogue_game::{FileStorage, MemoryStorage, Session, Storage};
use serde_json::Value;

/// A core in the process of its client.
pub struct ChrogueCore {
    session: Session,
    /// The last response. The client reads it until its next call.
    response: CString,
}

/// The options of `chrogue_open`.
#[derive(Debug, PartialEq)]
pub struct Options {
    pub save_dir: Option<String>,
    pub seed: Option<u64>,
    pub debug: bool,
}

impl Options {
    /// Reads the JSON object of `chrogue_open`. A field that the core does not know is ignored, as
    /// in a request.
    pub fn parse(text: &str) -> Result<Options, String> {
        let value: Value = serde_json::from_str(text).map_err(|e| format!("The options are not JSON: {e}"))?;
        let Value::Object(fields) = value else { return Err("The options must be a JSON object".into()) };
        let save_dir = match fields.get("save_dir") {
            None | Some(Value::Null) => None,
            Some(Value::String(dir)) if !dir.is_empty() => Some(dir.clone()),
            Some(other) => return Err(format!("\"save_dir\" must be a path, not {other}")),
        };
        // A client in Lua has no whole number of 64 bits, thus the seed can also be a string of digits.
        let seed = match fields.get("seed") {
            None | Some(Value::Null) => None,
            Some(Value::Number(n)) => Some(n.as_u64().ok_or(format!("\"seed\" must be a whole number, not {n}"))?),
            Some(Value::String(digits)) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                Some(digits.parse().map_err(|_| format!("\"seed\" is too large: {digits}"))?)
            }
            Some(other) => return Err(format!("\"seed\" must be a whole number, not {other}")),
        };
        let debug = match fields.get("debug") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(debug)) => *debug,
            Some(other) => return Err(format!("\"debug\" must be true or false, not {other}")),
        };
        Ok(Options { save_dir, seed, debug })
    }
}

impl ChrogueCore {
    pub fn open(options: Options) -> Result<ChrogueCore, String> {
        let storage: Box<dyn Storage> = match options.save_dir {
            Some(dir) => Box::new(FileStorage::open(dir).map_err(|e| e.to_string())?),
            None => Box::new(MemoryStorage::default()),
        };
        let seed = options
            .seed
            .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64));
        Ok(ChrogueCore { session: Session::with_debug(storage, seed, options.debug), response: CString::default() })
    }
}

/// The response to a call that has no core, and to a request that made the core panic. It has no `view`.
const NO_CORE: &CStr =
    c"{\"ok\":false,\"id\":null,\"error\":{\"code\":\"internal\",\"message\":\"The core has no session\"}}";
const FAILED: &str = r#"{"ok":false,"id":null,"error":{"code":"internal","message":"The core failed"}}"#;

thread_local! {
    static OPEN_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// Text for C. A response of the core is JSON, thus it has no 0 byte; a message of the system could have one.
fn c_text(text: String) -> CString {
    CString::new(text.replace('\0', " ")).unwrap_or_default()
}

/// The text of a C string, or None for NULL. Bytes that are not UTF-8 become the replacement character, as in
/// the socket of `chrogue-core`.
///
/// # Safety
/// `text` is NULL or a string with a 0 at its end.
unsafe fn text_of(text: *const c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned())
}

/// See `chrogue_core.h`.
///
/// # Safety
/// `options` is NULL or a string with a 0 at its end.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chrogue_open(options: *const c_char) -> *mut ChrogueCore {
    let text = unsafe { text_of(options) }.unwrap_or_else(|| "{}".into());
    let opened = catch_unwind(|| Options::parse(&text).and_then(ChrogueCore::open))
        .unwrap_or_else(|_| Err("The core failed while it opened".into()));
    match opened {
        Ok(core) => Box::into_raw(Box::new(core)),
        Err(reason) => {
            OPEN_ERROR.with(|error| *error.borrow_mut() = c_text(reason));
            ptr::null_mut()
        }
    }
}

/// See `chrogue_core.h`.
#[unsafe(no_mangle)]
pub extern "C" fn chrogue_open_error() -> *const c_char {
    OPEN_ERROR.with(|error| error.borrow().as_ptr())
}

/// See `chrogue_core.h`.
///
/// # Safety
/// `core` is NULL or a core of `chrogue_open` that is not closed, and no other thread uses it. `request` is NULL
/// or a string with a 0 at its end.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chrogue_command(core: *mut ChrogueCore, request: *const c_char) -> *const c_char {
    let Some(core) = (unsafe { core.as_mut() }) else { return NO_CORE.as_ptr() };
    let request = unsafe { text_of(request) }.unwrap_or_default();
    // `Session::command` does not panic. If it does, the client still gets a line that it can read.
    let response = catch_unwind(AssertUnwindSafe(|| core.session.command(&request))).unwrap_or_else(|_| FAILED.into());
    core.response = c_text(response);
    core.response.as_ptr()
}

/// See `chrogue_core.h`.
///
/// # Safety
/// `core` is NULL or a core of `chrogue_open` that is not closed. The client does not use it after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chrogue_close(core: *mut ChrogueCore) {
    if !core.is_null() {
        drop(unsafe { Box::from_raw(core) });
    }
}
