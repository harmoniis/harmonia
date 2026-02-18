use harmonia_vault::{get_secret_for_symbol, set_secret_for_symbol};
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-whatsapp/0.1.0\0";
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn last_error() -> &'static RwLock<String> { LAST_ERROR.get_or_init(|| RwLock::new(String::new())) }
fn set_error(msg: impl Into<String>) { if let Ok(mut s)=last_error().write(){*s=msg.into();} }
fn clear_error(){ if let Ok(mut s)=last_error().write(){s.clear();} }
fn cstr_to_string(ptr:*const c_char)->Result<String,String>{ if ptr.is_null(){return Err("null pointer".into())}; let c=unsafe{CStr::from_ptr(ptr)}; Ok(c.to_string_lossy().into_owned()) }
fn to_c_string(v:String)->*mut c_char{ CString::new(v).map(|s|s.into_raw()).unwrap_or(std::ptr::null_mut()) }

#[no_mangle] pub extern "C" fn harmonia_whatsapp_version()->*const c_char{ VERSION.as_ptr().cast() }
#[no_mangle] pub extern "C" fn harmonia_whatsapp_healthcheck()->i32{1}

#[no_mangle]
pub extern "C" fn harmonia_whatsapp_store_linked_device(device_id:*const c_char, creds:*const c_char)->i32{
    let device_id = match cstr_to_string(device_id){Ok(v)=>v,Err(e)=>{set_error(e);return -1;}};
    let creds = match cstr_to_string(creds){Ok(v)=>v,Err(e)=>{set_error(e);return -1;}};
    if let Err(e)=set_secret_for_symbol("whatsapp_device_id", &device_id){ set_error(e); return -1; }
    if let Err(e)=set_secret_for_symbol("whatsapp_device_creds", &creds){ set_error(e); return -1; }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_whatsapp_send_text(to:*const c_char, text:*const c_char)->i32{
    let to = match cstr_to_string(to){Ok(v)=>v,Err(e)=>{set_error(e);return -1;}};
    let text = match cstr_to_string(text){Ok(v)=>v,Err(e)=>{set_error(e);return -1;}};
    let token = match get_secret_for_symbol("whatsapp_api_key"){Some(v)=>v,None=>{set_error("missing secret: whatsapp_api_key");return -1;}};
    let device_id = get_secret_for_symbol("whatsapp_device_id").unwrap_or_default();
    let endpoint = env::var("HARMONIA_WHATSAPP_API_URL").unwrap_or_else(|_| "http://127.0.0.1:3000/api/sendText".into());
    let payload = format!("{{\"to\":\"{}\",\"text\":\"{}\",\"deviceId\":\"{}\"}}", json_escape(&to), json_escape(&text), json_escape(&device_id));
    let out = Command::new("curl").arg("-sS").arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-H").arg(format!("Authorization: Bearer {token}"))
        .arg("-d").arg(payload)
        .arg(endpoint).output();
    match out {
        Ok(o) if o.status.success() => { clear_error(); 0 }
        Ok(o) => { set_error(format!("whatsapp send failed: {}", String::from_utf8_lossy(&o.stderr))); -1 }
        Err(e) => { set_error(format!("curl exec failed: {e}")); -1 }
    }
}

fn json_escape(s:&str)->String{ s.replace('\\',"\\\\").replace('"',"\\\"").replace('\n',"\\n").replace('\r',"\\r") }

#[no_mangle] pub extern "C" fn harmonia_whatsapp_last_error()->*mut c_char{ let m=last_error().read().map(|v|v.clone()).unwrap_or_else(|_|"whatsapp lock poisoned".into()); to_c_string(m)}
#[no_mangle] pub extern "C" fn harmonia_whatsapp_free_string(ptr:*mut c_char){ if ptr.is_null(){return;} unsafe{drop(CString::from_raw(ptr));}}
