// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Safe Rust wrapper for librime (RIME input method engine).
//!
//! Provides idiomatic Rust access to the full librime API, including:
//! - Lifecycle management (setup, initialize, finalize)
//! - Session-based input processing
//! - Candidate selection and iteration
//! - Schema management
//! - Configuration access (Config)
//! - Deployment and maintenance
//! - Version/directory queries
//! - Levers API (custom settings, switcher settings, user dictionaries)

use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::ptr::null_mut;
use std::sync::{LazyLock, Mutex};

use rime_sys::{
    rime_get_api, rime_struct, RimeApi, RimeCandidateListIterator, RimeCommit, RimeConfig,
    RimeConfigIterator, RimeContext, RimeSchemaList, RimeSessionId, RimeStatus,
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

// Re-export modifier constants
pub use rime_sys::{
    RimeModifier_kAltMask as MODIFIER_ALT, RimeModifier_kControlMask as MODIFIER_CONTROL,
    RimeModifier_kHyperMask as MODIFIER_HYPER, RimeModifier_kLockMask as MODIFIER_LOCK,
    RimeModifier_kMetaMask as MODIFIER_META, RimeModifier_kModifierMask as MODIFIER_MASK,
    RimeModifier_kReleaseMask as MODIFIER_RELEASE, RimeModifier_kShiftMask as MODIFIER_SHIFT,
    RimeModifier_kSuperMask as MODIFIER_SUPER,
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
// SAFETY: `rime_get_api()` must be called after `setup()`. The API pointer
// returned is valid for the lifetime of the process. `RimeApiWrapper` wraps
// a raw pointer but is only accessed through the `rime_api_call!` macro
// which dereferences it in an unsafe block. `Send + Sync` is safe because
// the RIME API is thread-safe.

macro_rules! rime_api_call {
    ($f:ident, $($arg:tt)*) => {
        (***RIME_API).$f.unwrap()($($arg)*)
    };
    ($f:ident) => {
        (***RIME_API).$f.unwrap()()
    };
}

// --- Traits ---

/// RIME engine initialization traits.
///
/// Should be created with [`Traits::new()`], configured with setter methods,
/// then passed to [`setup()`] and [`initialize()`].
pub struct Traits {
    inner: rime_sys::RimeTraits,
    /// Owned C string pointers that need to be freed on drop.
    resources: Vec<*mut c_char>,
    /// Storage for module name CStrings (kept alive for the lifetime of Traits).
    modules_cstrings: Option<Vec<CString>>,
    /// Storage for the modules pointer array (kept alive for the lifetime of Traits).
    modules_array: Option<Box<[*const c_char]>>,
}

macro_rules! setter_fn_impl {
    ($field_name:ident, $fn_name:ident) => {
        impl Traits {
            #[doc = concat!("Set the `", stringify!($field_name), "` field.")]
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
setter_fn_impl!(distribution_name, set_distribution_name);
setter_fn_impl!(distribution_code_name, set_distribution_code_name);
setter_fn_impl!(distribution_version, set_distribution_version);
setter_fn_impl!(log_dir, set_log_dir);
setter_fn_impl!(prebuilt_data_dir, set_prebuilt_data_dir);
setter_fn_impl!(staging_dir, set_staging_dir);

impl Traits {
    /// Create a new, zero-initialized Traits structure.
    pub fn new() -> Self {
        rime_struct!(rime_traits: rime_sys::RimeTraits);
        Self {
            inner: rime_traits,
            resources: Vec::new(),
            modules_cstrings: None,
            modules_array: None,
        }
    }

    /// Set the minimal log level.
    ///
    /// Values: 0 = INFO (default), 1 = WARNING, 2 = ERROR, 3 = FATAL.
    pub fn set_min_log_level(&mut self, level: i32) -> &mut Self {
        self.inner.min_log_level = level as c_int;
        self
    }

    /// Set the list of modules to load before initializing.
    pub fn set_modules(&mut self, modules: &[&str]) -> &mut Self {
        let cstrings: Vec<CString> = modules
            .iter()
            .map(|s| CString::new(*s).expect("CString creation failed"))
            .collect();
        let mut array: Vec<*const c_char> = cstrings.iter().map(|cs| cs.as_ptr()).collect();
        array.push(std::ptr::null()); // null-terminated

        let boxed: Box<[*const c_char]> = array.into_boxed_slice();
        self.inner.modules = boxed.as_ptr() as *mut *const c_char;
        self.modules_cstrings = Some(cstrings);
        self.modules_array = Some(boxed);
        self
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

/// Call this before accessing any other API functions.
pub fn setup(traits: &mut Traits) {
    unsafe {
        rime_api_call!(setup, &mut traits.inner);
    }
}

/// Initialize the RIME engine with the given traits.
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

/// Finalize the RIME engine, releasing all resources.
pub fn finalize() {
    unsafe {
        rime_api_call!(finalize);
    }
}

// --- Notification handler ---

type DynNotificationHandler = dyn Fn(&str, &str) + Send + Sync;

static NOTIFICATION_HANDLER: LazyLock<Mutex<Option<Box<DynNotificationHandler>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Set a handler to receive RIME notifications (schema changes, mode changes, deployment events).
pub fn set_notification_handler<F>(handler: F)
where
    F: Fn(&str, &str) + 'static + Send + Sync,
{
    NOTIFICATION_HANDLER
        .lock()
        .unwrap()
        .replace(Box::new(handler));
}

extern "C" fn notification_handler(
    _obj: *mut c_void,
    _session_id: RimeSessionId,
    message_type: *const c_char,
    message_value: *const c_char,
) {
    unsafe {
        let message_type = CStr::from_ptr(message_type).to_string_lossy();
        let message_value = CStr::from_ptr(message_value).to_string_lossy();

        if message_type.as_ref() == "deploy" {
            let mut deploy_result = DEPLOY_RESULT.lock().unwrap();
            match message_value.as_ref() {
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
            f(message_type.as_ref(), message_value.as_ref());
        }
    }
}

// --- Deployment ---

/// Result of a deployment operation.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DeployResult {
    /// Deployment completed successfully.
    Success,
    /// Deployment failed.
    Failure,
}

static DEPLOY_RESULT: LazyLock<Mutex<Option<DeployResult>>> =
    LazyLock::new(|| Mutex::new(None));

/// Start maintenance (deployment) on workspace changes and wait for completion.
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

/// Check if the engine is currently in maintenance mode.
pub fn is_maintenance_mode() -> bool {
    unsafe { rime_api_call!(is_maintenance_mode) != 0 }
}

/// Initialize for deployment tool.
pub fn deployer_initialize(traits: &mut Traits) {
    unsafe {
        rime_api_call!(deployer_initialize, &mut traits.inner);
    }
}

/// Prebuild all schemas.
pub fn prebuild() -> bool {
    unsafe { rime_api_call!(prebuild) != 0 }
}

/// Deploy the entire workspace.
pub fn deploy() -> bool {
    unsafe { rime_api_call!(deploy) != 0 }
}

/// Deploy a specific schema file.
pub fn deploy_schema(schema_file: &str) -> anyhow::Result<()> {
    let s = CString::new(schema_file)?;
    unsafe {
        if rime_api_call!(deploy_schema, s.as_ptr()) == 0 {
            anyhow::bail!("failed to deploy schema: {schema_file}");
        }
    }
    Ok(())
}

/// Deploy a config file, updating the version key.
pub fn deploy_config_file(file_name: &str, version_key: &str) -> anyhow::Result<()> {
    let f = CString::new(file_name)?;
    let v = CString::new(version_key)?;
    unsafe {
        if rime_api_call!(deploy_config_file, f.as_ptr(), v.as_ptr()) == 0 {
            anyhow::bail!("failed to deploy config file: {file_name}");
        }
    }
    Ok(())
}

/// Sync user data.
pub fn sync_user_data() -> bool {
    unsafe { rime_api_call!(sync_user_data) != 0 }
}

/// Clean up stale sessions.
pub fn cleanup_stale_sessions() {
    unsafe { rime_api_call!(cleanup_stale_sessions) }
}

/// Clean up all sessions.
pub fn cleanup_all_sessions() {
    unsafe { rime_api_call!(cleanup_all_sessions) }
}

// --- Schema management ---

/// Information about a RIME input schema.
#[derive(Debug)]
pub struct SchemaInfo {
    /// Schema identifier (e.g., "luna_pinyin").
    pub schema_id: String,
    /// Display name (e.g., "Luna Pinyin").
    pub name: String,
}

/// Get the list of available schemas.
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

/// A RIME input session. Automatically destroys the session on drop.
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
    // --- Input ---

    /// Process a key event.
    pub fn process_key(&self, key: KeyEvent) {
        unsafe {
            rime_api_call!(process_key, self.session_id, key.key_code, key.modifiers);
        }
    }

    /// Commit the current composition.
    ///
    /// Returns `true` if there was uncommitted text that is now committed.
    pub fn commit_composition(&self) -> bool {
        unsafe { rime_api_call!(commit_composition, self.session_id) != 0 }
    }

    /// Clear the current composition state.
    pub fn clear_composition(&self) {
        unsafe { rime_api_call!(clear_composition, self.session_id) }
    }

    /// Simulate typing a key sequence (e.g., `"chifan"` sends each char as a keypress).
    pub fn simulate_key_sequence(&self, sequence: &str) -> anyhow::Result<()> {
        let s = CString::new(sequence)?;
        unsafe {
            if rime_api_call!(simulate_key_sequence, self.session_id, s.as_ptr()) == 0 {
                anyhow::bail!("failed to simulate key sequence: {sequence}");
            }
        }
        Ok(())
    }

    /// Get the raw input string for the current composition.
    ///
    /// Returns `None` if the session does not exist.
    /// Note: the returned value may become invalid upon subsequent editing.
    pub fn get_input(&self) -> Option<String> {
        let ptr = unsafe { rime_api_call!(get_input, self.session_id) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() })
        }
    }

    /// Get the caret position within the raw input.
    pub fn get_caret_pos(&self) -> usize {
        unsafe { rime_api_call!(get_caret_pos, self.session_id) }
    }

    /// Set the caret position within the raw input.
    pub fn set_caret_pos(&self, pos: usize) {
        unsafe { rime_api_call!(set_caret_pos, self.session_id, pos) }
    }

    /// Set the raw input programmatically.
    pub fn set_input(&self, input: &str) -> anyhow::Result<()> {
        let s = CString::new(input)?;
        unsafe {
            if rime_api_call!(set_input, self.session_id, s.as_ptr()) == 0 {
                anyhow::bail!("failed to set input");
            }
        }
        Ok(())
    }

    // --- Output ---

    /// Get the current context (composition, menu, candidates).
    pub fn context(&self) -> Option<Context> {
        unsafe {
            rime_struct!(context: RimeContext);
            if rime_api_call!(get_context, self.session_id, &mut context) == 0 {
                return None;
            }
            Some(Context { inner: context })
        }
    }

    /// Get the committed text, if any.
    pub fn commit(&self) -> Option<Commit> {
        rime_struct!(commit: RimeCommit);
        unsafe {
            if rime_api_call!(get_commit, self.session_id, &mut commit) == 0 {
                return None;
            }
        }
        Some(Commit { inner: commit })
    }

    /// Get the current session status.
    pub fn status(&self) -> anyhow::Result<Status> {
        rime_struct!(status: RimeStatus);
        unsafe {
            if rime_api_call!(get_status, self.session_id, &mut status) == 0 {
                anyhow::bail!("failed to get session status");
            }
            Ok(Status { inner: status })
        }
    }

    // --- Candidate selection ---

    /// Select candidate at the given index (global across all pages).
    pub fn select_candidate(&self, index: usize) -> bool {
        unsafe { rime_api_call!(select_candidate, self.session_id, index) != 0 }
    }

    /// Select candidate at the given index on the current page.
    pub fn select_candidate_on_current_page(&self, index: usize) -> bool {
        unsafe { rime_api_call!(select_candidate_on_current_page, self.session_id, index) != 0 }
    }

    /// Delete candidate at the given index (global).
    pub fn delete_candidate(&self, index: usize) -> bool {
        unsafe { rime_api_call!(delete_candidate, self.session_id, index) != 0 }
    }

    /// Delete candidate at the given index on the current page.
    pub fn delete_candidate_on_current_page(&self, index: usize) -> bool {
        unsafe { rime_api_call!(delete_candidate_on_current_page, self.session_id, index) != 0 }
    }

    /// Highlight candidate at the given index (global) without committing.
    pub fn highlight_candidate(&self, index: usize) -> bool {
        unsafe { rime_api_call!(highlight_candidate, self.session_id, index) != 0 }
    }

    /// Highlight candidate at the given index on the current page without committing.
    pub fn highlight_candidate_on_current_page(&self, index: usize) -> bool {
        unsafe { rime_api_call!(highlight_candidate_on_current_page, self.session_id, index) != 0 }
    }

    /// Change to the next (`backward=false`) or previous (`backward=true`) candidate page.
    pub fn change_page(&self, backward: bool) -> bool {
        unsafe { rime_api_call!(change_page, self.session_id, backward as c_int) != 0 }
    }

    // --- Candidate iterator ---

    /// Iterate over all candidates using the C iterator API.
    pub fn candidate_iter(&self) -> Option<CandidateIterator> {
        let mut iter: RimeCandidateListIterator = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(candidate_list_begin, self.session_id, &mut iter) == 0 {
                return None;
            }
        }
        Some(CandidateIterator {
            inner: iter,
            exhausted: false,
        })
    }

    /// Get all candidates as a Vec (convenience method).
    pub fn candidates(&self) -> Vec<Candidate> {
        self.candidate_iter()
            .map(|it| it.collect())
            .unwrap_or_default()
    }

    // --- Runtime options ---

    /// Set a runtime option (e.g., `"ascii_mode"`).
    pub fn set_option(&self, option: &str, value: bool) -> anyhow::Result<()> {
        let s = CString::new(option)?;
        unsafe { rime_api_call!(set_option, self.session_id, s.as_ptr(), value as c_int) }
        Ok(())
    }

    /// Get a runtime option value.
    pub fn get_option(&self, option: &str) -> anyhow::Result<bool> {
        let s = CString::new(option)?;
        Ok(unsafe { rime_api_call!(get_option, self.session_id, s.as_ptr()) != 0 })
    }

    /// Set a runtime property.
    pub fn set_property(&self, prop: &str, value: &str) -> anyhow::Result<()> {
        let p = CString::new(prop)?;
        let v = CString::new(value)?;
        unsafe { rime_api_call!(set_property, self.session_id, p.as_ptr(), v.as_ptr()) }
        Ok(())
    }

    /// Get a runtime property value.
    pub fn get_property(&self, prop: &str, buffer_size: usize) -> anyhow::Result<Option<String>> {
        let p = CString::new(prop)?;
        let mut buf = vec![0u8; buffer_size];
        let ok = unsafe {
            rime_api_call!(
                get_property,
                self.session_id,
                p.as_ptr(),
                buf.as_mut_ptr() as *mut c_char,
                buffer_size
            )
        };
        if ok == 0 {
            return Ok(None);
        }
        Ok(Some(
            unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy().to_string() },
        ))
    }

    // --- Schema ---

    /// Get the current schema ID for this session.
    pub fn get_current_schema(&self) -> Option<String> {
        let mut buf = [0u8; 256];
        let ok = unsafe {
            rime_api_call!(
                get_current_schema,
                self.session_id,
                buf.as_mut_ptr() as *mut c_char,
                256
            )
        };
        if ok == 0 {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy().to_string() },
        )
    }

    /// Select a schema by ID for this session.
    pub fn select_schema(&self, id: &str) -> anyhow::Result<()> {
        unsafe {
            let s = CString::new(id)?;
            if rime_api_call!(select_schema, self.session_id, s.as_ptr()) == 0 {
                anyhow::bail!("failed to select schema: {id}");
            }
        }
        Ok(())
    }

    /// Get the display label for a state option.
    pub fn get_state_label(&self, option_name: &str, state: bool) -> anyhow::Result<Option<String>> {
        let s = CString::new(option_name)?;
        let ptr =
            unsafe { rime_api_call!(get_state_label, self.session_id, s.as_ptr(), state as c_int) };
        if ptr.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() }))
        }
    }

    // --- Lifecycle ---

    /// Close and destroy this session.
    pub fn close(&mut self) -> anyhow::Result<()> {
        unsafe {
            if rime_api_call!(destroy_session, self.session_id) == 0 {
                anyhow::bail!("failed to destroy session");
            }
            self.closed = true;
            Ok(())
        }
    }
}

/// Create a new RIME input session.
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

/// A key event with key code and modifier flags.
#[derive(Copy, Clone)]
pub struct KeyEvent {
    /// X11-style key code.
    pub key_code: i32,
    /// Modifier bitmask (see `MODIFIER_*` constants).
    pub modifiers: i32,
}

impl KeyEvent {
    /// Create a new key event with no modifiers.
    pub fn new(key_code: i32) -> Self {
        Self {
            key_code,
            modifiers: 0,
        }
    }

    /// Create a new key event with specific modifiers.
    pub fn with_modifiers(key_code: i32, modifiers: i32) -> Self {
        Self {
            key_code,
            modifiers,
        }
    }

    /// Add shift modifier.
    pub fn shift(mut self) -> Self {
        self.modifiers |= MODIFIER_SHIFT as i32;
        self
    }

    /// Add control modifier.
    pub fn ctrl(mut self) -> Self {
        self.modifiers |= MODIFIER_CONTROL as i32;
        self
    }

    /// Add alt modifier.
    pub fn alt(mut self) -> Self {
        self.modifiers |= MODIFIER_ALT as i32;
        self
    }

    /// Mark as key release event.
    pub fn release(mut self) -> Self {
        self.modifiers |= MODIFIER_RELEASE as i32;
        self
    }
}

// --- Context / Composition / Menu / Candidate ---

/// Input context for a session, containing composition and candidate menu.
pub struct Context {
    inner: RimeContext,
}

impl Context {
    /// Get the composition (preedit text and cursor info).
    pub fn composition(&self) -> Composition {
        let c = self.inner.composition;
        Composition {
            length: c.length as usize,
            cursor_pos: c.cursor_pos as usize,
            sel_start: c.sel_start as usize,
            sel_end: c.sel_end as usize,
            preedit: to_c_str_nullable(c.preedit).map(|s| s.to_string()),
        }
    }

    /// Get the candidate menu.
    pub fn menu(&self) -> Menu {
        let m = self.inner.menu;
        let candidates = unsafe {
            let mut candidates = Vec::new();
            for i in 0..m.num_candidates as usize {
                let candidate = &*m.candidates.add(i);
                candidates.push(Candidate {
                    text: to_c_str(candidate.text).to_string(),
                    comment: to_c_str_nullable(candidate.comment).map(|s| s.to_string()),
                });
            }
            candidates
        };
        Menu {
            page_size: m.page_size as usize,
            page_no: m.page_no as usize,
            is_last_page: m.is_last_page != 0,
            highlighted_candidate_index: m.highlighted_candidate_index as usize,
            candidates,
            select_keys: to_c_str_nullable(m.select_keys).map(|s| s.to_string()),
        }
    }

    /// Get the commit text preview (text that will be committed).
    pub fn commit_text_preview(&self) -> Option<String> {
        to_c_str_nullable(self.inner.commit_text_preview).map(|s| s.to_string())
    }

    /// Get the select labels for candidates (e.g., `"1."`, `"2."`, etc.).
    pub fn select_labels(&self) -> Vec<String> {
        let num = self.inner.menu.num_candidates as usize;
        if self.inner.select_labels.is_null() || num == 0 {
            return Vec::new();
        }
        unsafe {
            std::slice::from_raw_parts(self.inner.select_labels, num)
                .iter()
                .filter_map(|&ptr| {
                    if ptr.is_null() {
                        None
                    } else {
                        Some(CStr::from_ptr(ptr).to_string_lossy().to_string())
                    }
                })
                .collect()
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

/// Composition state of the input.
#[derive(Debug)]
pub struct Composition {
    /// Length of the preedit string.
    pub length: usize,
    /// Caret position within the raw input.
    pub cursor_pos: usize,
    /// Start of the selected segment.
    pub sel_start: usize,
    /// End of the selected segment.
    pub sel_end: usize,
    /// Preedit display text.
    pub preedit: Option<String>,
}

/// Candidate menu for the current page.
#[derive(Debug)]
pub struct Menu {
    /// Number of candidates per page.
    pub page_size: usize,
    /// Current page number (0-indexed).
    pub page_no: usize,
    /// Whether this is the last page of candidates.
    pub is_last_page: bool,
    /// Index of the highlighted candidate.
    pub highlighted_candidate_index: usize,
    /// Candidate list for the current page.
    pub candidates: Vec<Candidate>,
    /// Selection key labels (e.g., `"1234567890"`).
    pub select_keys: Option<String>,
}

/// A candidate in the candidate list.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Candidate text.
    pub text: String,
    /// Optional comment (e.g., completion hint).
    pub comment: Option<String>,
}

// --- Commit ---

/// Committed text from the input method.
pub struct Commit {
    inner: RimeCommit,
}

impl Commit {
    /// Get the committed text.
    pub fn text(&self) -> String {
        to_c_str(self.inner.text).into_owned()
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

/// Session status information.
pub struct Status {
    inner: RimeStatus,
}

impl Status {
    /// Get the current schema ID.
    pub fn schema_id(&self) -> String {
        to_c_str(self.inner.schema_id).into_owned()
    }

    /// Get the current schema display name.
    pub fn schema_name(&self) -> String {
        to_c_str(self.inner.schema_name).into_owned()
    }

    /// Whether the input method is disabled.
    pub fn is_disabled(&self) -> bool {
        self.inner.is_disabled != 0
    }

    /// Whether the user is currently composing (has preedit text).
    pub fn is_composing(&self) -> bool {
        self.inner.is_composing != 0
    }

    /// Whether ASCII mode is active (direct Latin input).
    pub fn is_ascii_mode(&self) -> bool {
        self.inner.is_ascii_mode != 0
    }

    /// Whether full-shape mode is active.
    pub fn is_full_shape(&self) -> bool {
        self.inner.is_full_shape != 0
    }

    /// Whether simplified Chinese output is active.
    pub fn is_simplified(&self) -> bool {
        self.inner.is_simplified != 0
    }

    /// Whether traditional Chinese output is active.
    pub fn is_traditional(&self) -> bool {
        self.inner.is_traditional != 0
    }

    /// Whether ASCII punctuation mode is active.
    pub fn is_ascii_punct(&self) -> bool {
        self.inner.is_ascii_punct != 0
    }
}

impl Drop for Status {
    fn drop(&mut self) {
        unsafe {
            let _ = rime_api_call!(free_status, &mut self.inner);
        }
    }
}

// --- CandidateIterator ---

/// Iterator over all candidates using the C iterator API.
pub struct CandidateIterator {
    inner: RimeCandidateListIterator,
    exhausted: bool,
}

impl Iterator for CandidateIterator {
    type Item = Candidate;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }
        let ok = unsafe { rime_api_call!(candidate_list_next, &mut self.inner) };
        if ok == 0 {
            self.exhausted = true;
            return None;
        }
        let c = &self.inner.candidate;
        Some(Candidate {
            text: unsafe { CStr::from_ptr(c.text).to_string_lossy().to_string() },
            comment: unsafe {
                if c.comment.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(c.comment).to_string_lossy().to_string())
                }
            },
        })
    }
}

impl Drop for CandidateIterator {
    fn drop(&mut self) {
        unsafe { rime_api_call!(candidate_list_end, &mut self.inner) }
    }
}

// --- Config ---

/// A RIME configuration handle. Automatically closes on drop.
pub struct Config {
    inner: RimeConfig,
}

impl Config {
    /// Open a schema's configuration by schema ID.
    pub fn schema_open(schema_id: &str) -> anyhow::Result<Self> {
        let s = CString::new(schema_id)?;
        let mut config: RimeConfig = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(schema_open, s.as_ptr(), &mut config) == 0 {
                anyhow::bail!("failed to open schema config: {schema_id}");
            }
        }
        Ok(Self { inner: config })
    }

    /// Open a configuration by config ID (e.g., `"default"`).
    pub fn open(config_id: &str) -> anyhow::Result<Self> {
        let s = CString::new(config_id)?;
        let mut config: RimeConfig = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(config_open, s.as_ptr(), &mut config) == 0 {
                anyhow::bail!("failed to open config: {config_id}");
            }
        }
        Ok(Self { inner: config })
    }

    /// Open a user configuration by config ID.
    pub fn user_config_open(config_id: &str) -> anyhow::Result<Self> {
        let s = CString::new(config_id)?;
        let mut config: RimeConfig = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(user_config_open, s.as_ptr(), &mut config) == 0 {
                anyhow::bail!("failed to open user config: {config_id}");
            }
        }
        Ok(Self { inner: config })
    }

    /// Create an empty configuration object.
    pub fn new() -> anyhow::Result<Self> {
        let mut config: RimeConfig = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(config_init, &mut config) == 0 {
                anyhow::bail!("failed to init config");
            }
        }
        Ok(Self { inner: config })
    }

    /// Load configuration from a YAML string.
    pub fn from_yaml(yaml: &str) -> anyhow::Result<Self> {
        let config = Self::new()?;
        let s = CString::new(yaml)?;
        unsafe {
            if rime_api_call!(config_load_string, config.ptr(), s.as_ptr()) == 0 {
                anyhow::bail!("failed to load config from yaml");
            }
        }
        Ok(config)
    }

    /// Get a raw pointer to the inner `RimeConfig`.
    ///
    /// The C API uses `*mut RimeConfig` for all operations even when read-only,
    /// because `RimeConfig` is just an opaque handle.
    fn ptr(&self) -> *mut RimeConfig {
        // SAFETY: `RimeConfig` is an opaque handle (just a void pointer inside).
        // The C API uses `*mut RimeConfig` for all operations including reads,
        // because `RimeConfig` is a value type used as a lookup key, not mutated
        // through this pointer. Casting `*const → *mut` is safe because no actual
        // mutation of the `RimeConfig` struct memory occurs through this pointer.
        &self.inner as *const RimeConfig as *mut RimeConfig
    }

    // --- Getters ---

    /// Get a boolean config value.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        let k = CString::new(key).ok()?;
        let mut value: c_int = 0;
        let ok = unsafe { rime_api_call!(config_get_bool, self.ptr(), k.as_ptr(), &mut value) };
        if ok == 0 {
            None
        } else {
            Some(value != 0)
        }
    }

    /// Get an integer config value.
    pub fn get_int(&self, key: &str) -> Option<i32> {
        let k = CString::new(key).ok()?;
        let mut value: c_int = 0;
        let ok = unsafe { rime_api_call!(config_get_int, self.ptr(), k.as_ptr(), &mut value) };
        if ok == 0 {
            None
        } else {
            Some(value)
        }
    }

    /// Get a double config value.
    pub fn get_double(&self, key: &str) -> Option<f64> {
        let k = CString::new(key).ok()?;
        let mut value: c_double = 0.0;
        let ok = unsafe { rime_api_call!(config_get_double, self.ptr(), k.as_ptr(), &mut value) };
        if ok == 0 {
            None
        } else {
            Some(value)
        }
    }

    /// Get a string config value.
    pub fn get_string(&self, key: &str) -> Option<String> {
        let k = CString::new(key).ok()?;
        unsafe {
            let ptr = rime_api_call!(config_get_cstring, self.ptr(), k.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().to_string())
            }
        }
    }

    // --- Setters ---

    /// Set a boolean config value. Returns `true` on success.
    pub fn set_bool(&self, key: &str, value: bool) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_set_bool, self.ptr(), k.as_ptr(), value as c_int) != 0 })
    }

    /// Set an integer config value. Returns `true` on success.
    pub fn set_int(&self, key: &str, value: i32) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_set_int, self.ptr(), k.as_ptr(), value as c_int) != 0 })
    }

    /// Set a double config value. Returns `true` on success.
    pub fn set_double(&self, key: &str, value: f64) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_set_double, self.ptr(), k.as_ptr(), value) != 0 })
    }

    /// Set a string config value. Returns `true` on success.
    pub fn set_string(&self, key: &str, value: &str) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        let v = CString::new(value)?;
        Ok(unsafe { rime_api_call!(config_set_string, self.ptr(), k.as_ptr(), v.as_ptr()) != 0 })
    }

    // --- Structure operations ---

    /// Get the size of a list config value.
    pub fn list_size(&self, key: &str) -> anyhow::Result<usize> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_list_size, self.ptr(), k.as_ptr()) })
    }

    /// Create a list at the given key.
    pub fn create_list(&self, key: &str) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_create_list, self.ptr(), k.as_ptr()) != 0 })
    }

    /// Create a map at the given key.
    pub fn create_map(&self, key: &str) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_create_map, self.ptr(), k.as_ptr()) != 0 })
    }

    /// Clear a config value at the given key.
    pub fn clear(&self, key: &str) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(unsafe { rime_api_call!(config_clear, self.ptr(), k.as_ptr()) != 0 })
    }

    /// Update the config signature with the given signer.
    pub fn update_signature(&self, signer: &str) -> anyhow::Result<bool> {
        let s = CString::new(signer)?;
        Ok(unsafe { rime_api_call!(config_update_signature, self.ptr(), s.as_ptr()) != 0 })
    }

    // --- Iterators ---

    /// Iterate over entries in a list config value.
    pub fn iter_list(&self, key: &str) -> Option<ConfigIterator> {
        let k = CString::new(key).ok()?;
        let mut iter: RimeConfigIterator = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(config_begin_list, &mut iter, self.ptr(), k.as_ptr()) == 0 {
                return None;
            }
        }
        Some(ConfigIterator { inner: iter })
    }

    /// Iterate over entries in a map config value.
    pub fn iter_map(&self, key: &str) -> Option<ConfigIterator> {
        let k = CString::new(key).ok()?;
        let mut iter: RimeConfigIterator = unsafe { std::mem::zeroed() };
        unsafe {
            if rime_api_call!(config_begin_map, &mut iter, self.ptr(), k.as_ptr()) == 0 {
                return None;
            }
        }
        Some(ConfigIterator { inner: iter })
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe { rime_api_call!(config_close, self.ptr()) };
    }
}

/// Iterator over config entries (list or map).
pub struct ConfigIterator {
    inner: RimeConfigIterator,
}

/// A key-path entry from a config iterator.
#[derive(Debug)]
pub struct ConfigEntry {
    /// Entry key.
    pub key: String,
    /// Full path to the entry.
    pub path: String,
}

impl Iterator for ConfigIterator {
    type Item = ConfigEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let ok = unsafe { rime_api_call!(config_next, &mut self.inner) };
        if ok == 0 {
            return None;
        }
        Some(ConfigEntry {
            key: unsafe { CStr::from_ptr(self.inner.key).to_string_lossy().to_string() },
            path: unsafe { CStr::from_ptr(self.inner.path).to_string_lossy().to_string() },
        })
    }
}

impl Drop for ConfigIterator {
    fn drop(&mut self) {
        unsafe { rime_api_call!(config_end, &mut self.inner) }
    }
}

// --- Version/directory queries ---

/// Get the librime version string.
pub fn get_version() -> String {
    let ptr = unsafe { rime_api_call!(get_version) };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() }
}

/// Get the user ID.
pub fn get_user_id() -> String {
    let ptr = unsafe { rime_api_call!(get_user_id) };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() }
}

macro_rules! dir_query_fn {
    ($fn_name:ident, $api_field:ident) => {
        #[doc = concat!("Query the ", stringify!($api_field), " path.")]
        pub fn $fn_name() -> String {
            let mut buf = [0u8; 1024];
            unsafe { rime_api_call!($api_field, buf.as_mut_ptr() as *mut c_char, 1024) };
            unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy().to_string() }
        }
    };
}

dir_query_fn!(get_shared_data_dir, get_shared_data_dir_s);
dir_query_fn!(get_user_data_dir, get_user_data_dir_s);
dir_query_fn!(get_prebuilt_data_dir, get_prebuilt_data_dir_s);
dir_query_fn!(get_staging_dir, get_staging_dir_s);
dir_query_fn!(get_sync_dir, get_sync_dir_s);

// --- Levers API ---

/// Opaque handle for custom settings (levers API).
pub struct CustomSettings(*mut rime_sys::RimeCustomSettings);

/// Opaque handle for switcher settings (levers API).
pub struct SwitcherSettings(*mut rime_sys::RimeSwitcherSettings);

/// Opaque handle for user dictionary iterator (levers API).
pub struct UserDictIterator(rime_sys::RimeUserDictIterator);

/// Internal: obtain the levers API vtable via `find_module("levers")`.
///
/// # Safety
///
/// Must be called after RIME is initialized. The returned pointer is valid for
/// the lifetime of the process. The cast from `*mut RimeCustomApi` to
/// `*mut RimeLeversApi` is sound because the levers module's `get_api()` returns
/// a pointer to a statically allocated `RimeLeversApi` cast to the base type
/// `RimeCustomApi` — this is the standard RIME plugin API pattern.
fn get_levers_api() -> Option<*mut rime_sys::RimeLeversApi> {
    let module_name = CString::new("levers").ok()?;
    let module = unsafe { rime_api_call!(find_module, module_name.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let get_api = unsafe { (*module).get_api? };
    let api = unsafe { get_api() };
    if api.is_null() {
        return None;
    }
    Some(api as *mut rime_sys::RimeLeversApi)
}

macro_rules! rime_levers_call {
    ($f:ident) => {{
        let api = crate::rime::get_levers_api().expect("levers module not available");
        unsafe { (*api).$f.unwrap()() }
    }};
    ($f:ident, $($arg:tt)*) => {{
        let api = crate::rime::get_levers_api().expect("levers module not available");
        unsafe { (*api).$f.unwrap()($($arg)*) }
    }};
}

// --- CustomSettings ---

impl CustomSettings {
    /// Create a new custom settings instance.
    ///
    /// `config_id` is the configuration file name (without `.yaml` extension).
    /// `generator_id` identifies the tool creating/modifying the settings.
    pub fn new(config_id: &str, generator_id: &str) -> anyhow::Result<Self> {
        let c = CString::new(config_id)?;
        let g = CString::new(generator_id)?;
        let ptr = rime_levers_call!(custom_settings_init, c.as_ptr(), g.as_ptr());
        if ptr.is_null() {
            anyhow::bail!("failed to create custom settings for: {config_id}");
        }
        Ok(Self(ptr))
    }

    /// Load settings from disk. Returns `true` if successful.
    pub fn load(&self) -> bool {
        rime_levers_call!(load_settings, self.0) != 0
    }

    /// Save settings to disk. Returns `true` if successful.
    pub fn save(&self) -> bool {
        rime_levers_call!(save_settings, self.0) != 0
    }

    /// Customize a boolean value. Returns `true` if successful.
    pub fn customize_bool(&self, key: &str, value: bool) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(rime_levers_call!(customize_bool, self.0, k.as_ptr(), value as c_int) != 0)
    }

    /// Customize an integer value. Returns `true` if successful.
    pub fn customize_int(&self, key: &str, value: i32) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(rime_levers_call!(customize_int, self.0, k.as_ptr(), value as c_int) != 0)
    }

    /// Customize a double value. Returns `true` if successful.
    pub fn customize_double(&self, key: &str, value: f64) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        Ok(rime_levers_call!(customize_double, self.0, k.as_ptr(), value) != 0)
    }

    /// Customize a string value. Returns `true` if successful.
    pub fn customize_string(&self, key: &str, value: &str) -> anyhow::Result<bool> {
        let k = CString::new(key)?;
        let v = CString::new(value)?;
        Ok(rime_levers_call!(customize_string, self.0, k.as_ptr(), v.as_ptr()) != 0)
    }

    /// Check if this is the first time these settings have been loaded.
    pub fn is_first_run(&self) -> bool {
        rime_levers_call!(is_first_run, self.0) != 0
    }

    /// Check if the settings have been modified since last save.
    pub fn is_modified(&self) -> bool {
        rime_levers_call!(settings_is_modified, self.0) != 0
    }

    /// Get access to the underlying Config object for these settings.
    pub fn config(&self) -> Option<ConfigBorrow<'_>> {
        let mut config: RimeConfig = unsafe { std::mem::zeroed() };
        let ok = rime_levers_call!(settings_get_config, self.0, &mut config);
        if ok == 0 {
            return None;
        }
        Some(ConfigBorrow { inner: config, _marker: std::marker::PhantomData })
    }
}

impl Drop for CustomSettings {
    fn drop(&mut self) {
        if !self.0.is_null() {
            rime_levers_call!(custom_settings_destroy, self.0);
        }
    }
}

/// A borrowed reference to a Config obtained from CustomSettings.
/// Does not close the config on drop (the settings own it).
pub struct ConfigBorrow<'a> {
    inner: RimeConfig,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl ConfigBorrow<'_> {
    fn ptr(&self) -> *mut RimeConfig {
        // SAFETY: Same rationale as Config::ptr() — RimeConfig is opaque.
        &self.inner as *const RimeConfig as *mut RimeConfig
    }

    /// Get a string config value.
    pub fn get_string(&self, key: &str) -> Option<String> {
        let k = CString::new(key).ok()?;
        unsafe {
            let ptr = rime_api_call!(config_get_cstring, self.ptr(), k.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().to_string())
            }
        }
    }

    /// Get an integer config value.
    pub fn get_int(&self, key: &str) -> Option<i32> {
        let k = CString::new(key).ok()?;
        let mut value: c_int = 0;
        let ok = unsafe { rime_api_call!(config_get_int, self.ptr(), k.as_ptr(), &mut value) };
        if ok == 0 { None } else { Some(value) }
    }

    /// Get a boolean config value.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        let k = CString::new(key).ok()?;
        let mut value: c_int = 0;
        let ok = unsafe { rime_api_call!(config_get_bool, self.ptr(), k.as_ptr(), &mut value) };
        if ok == 0 { None } else { Some(value != 0) }
    }
}

// --- SwitcherSettings ---

impl SwitcherSettings {
    /// Create a new switcher settings instance.
    pub fn new() -> anyhow::Result<Self> {
        let ptr = rime_levers_call!(switcher_settings_init);
        if ptr.is_null() {
            anyhow::bail!("failed to create switcher settings");
        }
        Ok(Self(ptr))
    }

    /// Get the list of available (installed) schemas.
    pub fn get_available_schemas(&self) -> Vec<SchemaInfo> {
        let mut list: RimeSchemaList = unsafe { std::mem::zeroed() };
        let ok = rime_levers_call!(get_available_schema_list, self.0, &mut list);
        if ok == 0 || list.size == 0 {
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
        rime_levers_call!(schema_list_destroy, &mut list);
        schemas
    }

    /// Get the list of currently selected (active) schemas.
    pub fn get_selected_schemas(&self) -> Vec<SchemaInfo> {
        let mut list: RimeSchemaList = unsafe { std::mem::zeroed() };
        let ok = rime_levers_call!(get_selected_schema_list, self.0, &mut list);
        if ok == 0 || list.size == 0 {
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
        rime_levers_call!(schema_list_destroy, &mut list);
        schemas
    }

    /// Select which schemas to activate.
    pub fn select_schemas(&self, schema_ids: &[&str]) -> anyhow::Result<bool> {
        let cstrings: Vec<CString> = schema_ids
            .iter()
            .map(|s| CString::new(*s))
            .collect::<std::result::Result<_, _>>()?;
        let mut ptrs: Vec<*const c_char> = cstrings.iter().map(|cs| cs.as_ptr()).collect();
        Ok(rime_levers_call!(
            select_schemas,
            self.0,
            ptrs.as_mut_ptr(),
            cstrings.len() as c_int
        ) != 0)
    }

    /// Get the hotkey configuration string.
    pub fn get_hotkeys(&self) -> Option<String> {
        let ptr = rime_levers_call!(get_hotkeys, self.0);
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() })
        }
    }

    /// Set the hotkey configuration string.
    pub fn set_hotkeys(&self, hotkeys: &str) -> anyhow::Result<bool> {
        let s = CString::new(hotkeys)?;
        Ok(rime_levers_call!(set_hotkeys, self.0, s.as_ptr()) != 0)
    }
}

impl Drop for SwitcherSettings {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `SwitcherSettings` extends `CustomSettings` in the C++
            // implementation. The levers API does not provide a dedicated
            // `switcher_settings_destroy`; `custom_settings_destroy` correctly
            // handles both types through C++ virtual destruction.
            rime_levers_call!(custom_settings_destroy, self.0 as *mut rime_sys::RimeCustomSettings);
        }
    }
}

// --- User dictionary operations ---

/// Iterate over user dictionary names.
pub fn user_dict_iter() -> Option<UserDictIterator> {
    let mut iter: rime_sys::RimeUserDictIterator = unsafe { std::mem::zeroed() };
    let ok = rime_levers_call!(user_dict_iterator_init, &mut iter);
    if ok == 0 {
        return None;
    }
    Some(UserDictIterator(iter))
}

impl Iterator for UserDictIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = rime_levers_call!(next_user_dict, &mut self.0);
        if ptr.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() })
    }
}

impl Drop for UserDictIterator {
    fn drop(&mut self) {
        rime_levers_call!(user_dict_iterator_destroy, &mut self.0);
    }
}

/// Backup a user dictionary to a snapshot file.
pub fn backup_user_dict(dict_name: &str) -> bool {
    let s = CString::new(dict_name).expect("invalid dict name");
    rime_levers_call!(backup_user_dict, s.as_ptr()) != 0
}

/// Restore a user dictionary from a snapshot file.
pub fn restore_user_dict(snapshot_file: &str) -> bool {
    let s = CString::new(snapshot_file).expect("invalid snapshot file");
    rime_levers_call!(restore_user_dict, s.as_ptr()) != 0
}

/// Export a user dictionary to a text file.
pub fn export_user_dict(dict_name: &str, text_file: &str) -> i32 {
    let d = CString::new(dict_name).expect("invalid dict name");
    let t = CString::new(text_file).expect("invalid text file");
    rime_levers_call!(export_user_dict, d.as_ptr(), t.as_ptr())
}

/// Import a user dictionary from a text file.
pub fn import_user_dict(dict_name: &str, text_file: &str) -> i32 {
    let d = CString::new(dict_name).expect("invalid dict name");
    let t = CString::new(text_file).expect("invalid text file");
    rime_levers_call!(import_user_dict, d.as_ptr(), t.as_ptr())
}

// --- Helpers ---

/// Convert a non-null C string pointer to `&str`.
///
/// # Panics
///
/// Panics if `ptr` is null. Use `to_c_str_nullable` for pointers that may be null.
fn to_c_str<'a>(ptr: *mut c_char) -> std::borrow::Cow<'a, str> {
    assert!(!ptr.is_null(), "to_c_str: null pointer");
    unsafe { CStr::from_ptr(ptr).to_string_lossy() }
}

/// Convert a possibly-null C string pointer to `Option<&str>`.
fn to_c_str_nullable<'a>(ptr: *mut c_char) -> Option<std::borrow::Cow<'a, str>> {
    if ptr.is_null() {
        return None;
    }
    Some(to_c_str(ptr))
}
