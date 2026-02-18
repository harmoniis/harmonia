use harmonia_vault::get_secret_for_symbol;
use std::env; use std::ffi::{CStr,CString}; use std::os::raw::c_char; use std::process::Command; use std::sync::{OnceLock,RwLock};
const VERSION:&[u8]=b"harmonia-search-brave/0.1.0\0"; static LAST_ERROR:OnceLock<RwLock<String>>=OnceLock::new();
fn le()->&'static RwLock<String>{LAST_ERROR.get_or_init(||RwLock::new(String::new()))}
fn set(m:impl Into<String>){if let Ok(mut s)=le().write(){*s=m.into();}} fn clear(){if let Ok(mut s)=le().write(){s.clear();}}
fn cs(p:*const c_char)->Result<String,String>{if p.is_null(){return Err("null pointer".into())}; let c=unsafe{CStr::from_ptr(p)}; Ok(c.to_string_lossy().into_owned())}
fn to(v:String)->*mut c_char{CString::new(v).map(|s|s.into_raw()).unwrap_or(std::ptr::null_mut())}
#[no_mangle] pub extern "C" fn harmonia_search_brave_version()->*const c_char{VERSION.as_ptr().cast()} #[no_mangle] pub extern "C" fn harmonia_search_brave_healthcheck()->i32{1}
#[no_mangle]
pub extern "C" fn harmonia_search_brave_query(query:*const c_char)->*mut c_char{
 let query=match cs(query){Ok(v)=>v,Err(e)=>{set(e);return std::ptr::null_mut();}};
 let key=match get_secret_for_symbol("brave_api_key"){Some(v)=>v,None=>{set("missing secret: brave_api_key");return std::ptr::null_mut();}};
 let endpoint=env::var("HARMONIA_BRAVE_API_URL").unwrap_or_else(|_|"https://api.search.brave.com/res/v1/web/search".into());
 let out=Command::new("curl").arg("-sS").arg("-G").arg("-H").arg(format!("X-Subscription-Token: {key}")).arg("--data-urlencode").arg(format!("q={query}")).arg(endpoint).output();
 match out{Ok(o) if o.status.success()=>{clear();to(String::from_utf8_lossy(&o.stdout).to_string())},Ok(o)=>{set(format!("brave query failed: {}",String::from_utf8_lossy(&o.stderr)));std::ptr::null_mut()},Err(e)=>{set(format!("curl exec failed: {e}"));std::ptr::null_mut()}}
}
#[no_mangle] pub extern "C" fn harmonia_search_brave_last_error()->*mut c_char{to(le().read().map(|v|v.clone()).unwrap_or_else(|_|"search-brave lock poisoned".into()))}
#[no_mangle] pub extern "C" fn harmonia_search_brave_free_string(ptr:*mut c_char){if ptr.is_null(){return;} unsafe{drop(CString::from_raw(ptr));}}
