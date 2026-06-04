// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: MIT

//! Safe Rust wrapper for librime (RIME input method engine).
//!
//! Only covers the APIs used by rsime. Based on the `rime_get_api()` handler pattern.

use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::null_mut;
use std::sync::{LazyLock, Mutex};

use rime_sys::{
    rime_get_api, rime_struct, RimeApi, RimeCommit, RimeContext,
    RimeSchemaList, RimeSessionId, RimeStatus,
};

// Re-export commonly used keycodes
pub use rime_sys::{
    RimeKeyCode_XK_BackSpace as KEY_BACKSPACE, RimeKeyCode_XK_Delete as KEY_DELETE,
    RimeKeyCode_XK_Down as KEY_DOWN, RimeKeyCode_XK_End as KEY_END,
    RimeKeyCode_XK_Escape as KEY_ESCAPE, RimeKeyCode_XK_Home as KEY_HOME,
    RimeKeyCode_XK_Left as KEY_LEFT, RimeKeyCode_XK_Page_Down as KEY_PAGEDOWN,
    RimeKeyCode_XK_Page_Up as KEY_PAGEUP, RimeKeyCode_XK_Return as KEY_RETURN,
    RimeKeyCode_XK_Right as KEY_RIGHT, RimeKeyCode_XK_Tab as KEY_TAB,
    RimeKeyCode_XK_Up as KEY_UP, RimeKeyCode_XK_space as KEY_SPACE,
};

// --- Internal: global API pointer ---

struct RimeApiWrapper(*mut RimeApi);

impl Deref for RimeApiWrapper {
    type Target = *mut RimeApi;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<*mut RimeApi> for RimeApiWrapper {
    fn from(value: *mut RimeApi) -> Self {
        Self(value)
    }
}

unsafe impl Send for RimeApiWrapper {}
unsafe impl Sync for RimeApiWrapper {}

static RIME_API: LazyLock<RimeApiWrapper> = LazyLock::new(|| unsafe { rime_get_api().into() });

macro_rules! rime_api_call {
    ($f:tt, $($arg:tt)*) => {
        (***RIME_API).$f.unwrap()($($arg)*)
    };
    ($f:tt) => {
        (***RIME_API).$f.unwrap()()
    };
}

// --- Traits ---

pub struct Traits {
    inner: rime_sys::RimeTraits,
    resources: Vec<*mut c_char>,
}

macro_rules! setter_fn_impl {
    ($field_name:ident, $fn_name:ident) => {
        impl Traits {
            pub fn $fn_name(&mut self, value: &str) -> &mut Self {
                let c_string = CString::new(value).expect("CString creation failed");
                let ptr = c_string.into_raw();
                self.inner.$field_name = ptr;
                self.resources.push(ptr);
                self
            }
        }
    };
}

setter_fn_impl!(shared_data_dir, set_shared_data_dir);
setter_fn_impl!(user_data_dir, set_user_data_dir);
setter_fn_impl!(app_name, set_app_name);

impl Traits {
    pub fn new() -> Self {
        rime_struct!(rime_traits: rime_sys::RimeTraits);
        Self {
            inner: rime_traits,
            resources: Vec::new(),
        }
    }
}

impl Default for Traits {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Traits {
    fn drop(&mut self) {
        for ptr in &self.resources {
            unsafe {
                drop(CString::from_raw(*ptr));
            }
        }
    }
}

// --- Global lifecycle functions ---

pub fn setup(traits: &mut Traits) {
    unsafe {
        rime_api_call!(setup, &mut traits.inner);
    }
}

pub fn initialize(traits: &mut Traits) {
    unsafe {
        rime_api_call!(initialize, &mut traits.inner);
        rime_api_call!(
            set_notification_handler,
            Some(notification_handler),
            null_mut()
        );
    }
}

pub fn finalize() {
    unsafe {
        rime_api_call!(finalize);
    }
}

// --- Deployment ---

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DeployResult {
    Success,
    Failure,
}

static DEPLOY_RESULT: LazyLock<Mutex<Option<DeployResult>>> =
    LazyLock::new(|| Mutex::new(None));

extern "C" fn notification_handler(
    _obj: *mut c_void,
    _session_id: RimeSessionId,
    message_type: *const c_char,
    message_value: *const c_char,
) {
    unsafe {
        let message_type = CStr::from_ptr(message_type).to_str().unwrap();
        let message_value = CStr::from_ptr(message_value).to_str().unwrap();

        if message_type == "deploy" {
            let mut deploy_result = DEPLOY_RESULT.lock().unwrap();
            match message_value {
                "success" => {
                    deploy_result.replace(DeployResult::Success);
                }
                "failure" => {
                    deploy_result.replace(DeployResult::Failure);
                }
                _ => {}
            }
        }

        let on_message_handler = NOTIFICATION_HANDLER.lock().unwrap();
        if let Some(f) = on_message_handler.as_ref() {
            f(message_type, message_value);
        }
    }
}

// --- Notification handler ---

type DynNotificationHandler = dyn Fn(&str, &str) + Send + Sync;

static NOTIFICATION_HANDLER: LazyLock<Mutex<Option<Box<DynNotificationHandler>>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn set_notification_handler<F>(handler: F)
where
    F: Fn(&str, &str) + 'static + Send + Sync,
{
    NOTIFICATION_HANDLER
        .lock()
        .unwrap()
        .replace(Box::new(handler));
}

// --- Deploy functions ---

pub fn deploy_on_changed() -> DeployResult {
    *DEPLOY_RESULT.lock().unwrap() = None;

    let started = unsafe { rime_api_call!(start_maintenance, false as c_int) };
    if started == 0 {
        return DeployResult::Failure;
    }

    unsafe {
        rime_api_call!(join_maintenance_thread);
    }

    match *DEPLOY_RESULT.lock().unwrap() {
        Some(DeployResult::Success) => DeployResult::Success,
        _ => DeployResult::Failure,
    }
}

// --- Schema management ---

#[derive(Debug)]
pub struct SchemaInfo {
    pub schema_id: String,
    pub name: String,
}

pub fn get_schema_list() -> Vec<SchemaInfo> {
    let mut list: RimeSchemaList = unsafe { std::mem::zeroed() };
    let ok = unsafe { rime_api_call!(get_schema_list, &mut list) != 0 };
    if !ok || list.size == 0 {
        return Vec::new();
    }
    let schemas = unsafe {
        std::slice::from_raw_parts(list.list, list.size)
            .iter()
            .map(|item| SchemaInfo {
                schema_id: CStr::from_ptr(item.schema_id).to_string_lossy().to_string(),
                name: CStr::from_ptr(item.name).to_string_lossy().to_string(),
            })
            .collect()
    };
    unsafe {
        rime_api_call!(free_schema_list, &mut list);
    }
    schemas
}

// --- Session ---

pub struct Session {
    session_id: RimeSessionId,
    closed: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.close();
        }
    }
}

impl Session {
    pub fn process_key(&self, key: KeyEvent) {
        unsafe {
            rime_api_call!(process_key, self.session_id, key.key_code, key.modifiers);
        }
    }

    pub fn context(&self) -> Option<Context> {
        unsafe {
            rime_struct!(context: RimeContext);
            if rime_api_call!(get_context, self.session_id, &mut context) == 0 {
                return None;
            }
            Some(Context { inner: context })
        }
    }

    pub fn commit(&self) -> Option<Commit> {
        rime_struct!(commit: RimeCommit);
        unsafe {
            if rime_api_call!(get_commit, self.session_id, &mut commit) == 0 {
                return None;
            }
        }
        Some(Commit { inner: commit })
    }

    pub fn status(&self) -> anyhow::Result<Status> {
        rime_struct!(status: RimeStatus);
        unsafe {
            if rime_api_call!(get_status, self.session_id, &mut status) == 0 {
                anyhow::bail!("failed to get session status");
            }
            Ok(Status { inner: status })
        }
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        unsafe {
            if rime_api_call!(destroy_session, self.session_id) == 0 {
                anyhow::bail!("failed to destroy session");
            }
            self.closed = true;
            Ok(())
        }
    }

    pub fn select_schema(&self, id: &str) -> anyhow::Result<()> {
        unsafe {
            let s = CString::new(id)?;
            if rime_api_call!(select_schema, self.session_id, s.as_ptr()) == 0 {
                anyhow::bail!("failed to select schema: {id}");
            }
        }
        Ok(())
    }
}

pub fn create_session() -> anyhow::Result<Session> {
    let session_id = unsafe { rime_api_call!(create_session) };
    let session = Session {
        session_id,
        closed: false,
    };
    if unsafe { rime_api_call!(find_session, session.session_id) == 0 } {
        anyhow::bail!("failed to create rime session");
    }
    Ok(session)
}

// --- KeyEvent ---

#[derive(Copy, Clone)]
pub struct KeyEvent {
    pub key_code: i32,
    pub modifiers: i32,
}

// --- Context / Composition / Menu / Candidate ---

pub struct Context {
    inner: RimeContext,
}

impl Context {
    pub fn composition(&self) -> Composition<'_> {
        let composition = self.inner.composition;
        Composition {
            length: composition.length as usize,
            preedit: to_c_str_nullable(composition.preedit),
        }
    }

    pub fn menu(&self) -> Menu<'_> {
        let menu = self.inner.menu;
        Menu {
            highlighted_candidate_index: menu.highlighted_candidate_index as usize,
            candidates: unsafe {
                let mut candidates = Vec::new();
                for i in 0..menu.num_candidates as usize {
                    let candidate = &*menu.candidates.add(i);
                    candidates.push(Candidate {
                        text: to_c_str(candidate.text),
                        comment: to_c_str_nullable(candidate.comment),
                    });
                }
                candidates
            },
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            rime_api_call!(free_context, &mut self.inner);
        }
    }
}

#[derive(Debug)]
pub struct Composition<'a> {
    pub length: usize,
    pub preedit: Option<&'a str>,
}

#[derive(Debug)]
pub struct Menu<'a> {
    pub highlighted_candidate_index: usize,
    pub candidates: Vec<Candidate<'a>>,
}

#[derive(Debug)]
pub struct Candidate<'a> {
    pub text: &'a str,
    pub comment: Option<&'a str>,
}

// --- Commit ---

pub struct Commit {
    inner: RimeCommit,
}

impl Commit {
    pub fn text(&self) -> &'_ str {
        to_c_str(self.inner.text)
    }
}

impl Drop for Commit {
    fn drop(&mut self) {
        unsafe {
            rime_api_call!(free_commit, &mut self.inner);
        }
    }
}

// --- Status ---

pub struct Status {
    inner: RimeStatus,
}

impl Status {
    pub fn schema_id(&self) -> &'_ str {
        to_c_str(self.inner.schema_id)
    }
}

impl Drop for Status {
    fn drop(&mut self) {
        unsafe {
            let _ = rime_api_call!(free_status, &mut self.inner);
        }
    }
}

// --- Helpers ---

fn to_c_str<'a>(ptr: *mut c_char) -> &'a str {
    unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
}

fn to_c_str_nullable<'a>(ptr: *mut c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    Some(to_c_str(ptr))
}
