use harmonia_vault::get_secret_for_symbol;
use std::env; use std::ffi::{CStr,CString}; use std::os::raw::c_char; use std::process::Command; use std::sync::{OnceLock,RwLock};
const VERSION:&[u8]=b"harmonia-search-exa/0.1.0\0"; static LAST_ERROR:OnceLock<RwLock<String>>=OnceLock::new();
fn le()->&'static RwLock<String>{LAST_ERROR.get_or_init(||RwLock::new(String::new()))}
fn set(m:impl Into<String>){if let Ok(mut s)=le().write(){*s=m.into();}} fn clear(){if let Ok(mut s)=le().write(){s.clear();}}
fn cs(p:*const c_char)->Result<String,String>{if p.is_null(){return Err("null pointer".into())}; let c=unsafe{CStr::from_ptr(p)}; Ok(c.to_string_lossy().into_owned())}
fn to(v:String)->*mut c_char{CString::new(v).map(|s|s.into_raw()).unwrap_or(std::ptr::null_mut())}
#[no_mangle] pub extern "C" fn harmonia_search_exa_version()->*const c_char{VERSION.as_ptr().cast()} #[no_mangle] pub extern "C" fn harmonia_search_exa_healthcheck()->i32{1}
#[no_mangle]
pub extern "C" fn harmonia_search_exa_query(query:*const c_char)->*mut c_char{
 let query=match cs(query){Ok(v)=>v,Err(e)=>{set(e);return std::ptr::null_mut();}};
 let key=match get_secret_for_symbol("exa_api_key"){Some(v)=>v,None=>{set("missing secret: exa_api_key");return std::ptr::null_mut();}};
 let endpoint=env::var("HARMONIA_EXA_API_URL").unwrap_or_else(|_|"https://api.exa.ai/search".into());
 let payload=format!("{{\"query\":\"{}\",\"numResults\":5}}",esc(&query));
 let out=Command::new("curl").arg("-sS").arg("-X").arg("POST").arg("-H").arg("Content-Type: application/json").arg("-H").arg(format!("x-api-key: {key}")).arg("-d").arg(payload).arg(endpoint).output();
 match out{Ok(o) if o.status.success()=>{clear();to(String::from_utf8_lossy(&o.stdout).to_string())},Ok(o)=>{set(format!("exa query failed: {}",String::from_utf8_lossy(&o.stderr)));std::ptr::null_mut()},Err(e)=>{set(format!("curl exec failed: {e}"));std::ptr::null_mut()}}
}
fn esc(s:&str)->String{s.replace('\\',"\\\\").replace('"',"\\\"").replace('\n',"\\n").replace('\r',"\\r")}
#[no_mangle] pub extern "C" fn harmonia_search_exa_last_error()->*mut c_char{to(le().read().map(|v|v.clone()).unwrap_or_else(|_|"search-exa lock poisoned".into()))}
#[no_mangle] pub extern "C" fn harmonia_search_exa_free_string(ptr:*mut c_char){if ptr.is_null(){return;} unsafe{drop(CString::from_raw(ptr));}}
