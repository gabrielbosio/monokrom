use std::sync::Mutex;

extern "C" {
    fn monokrom_download_file(name: *const u8, name_len: u32, data: *const u8, data_len: u32);
    fn monokrom_request_upload();
}

static PENDING_UPLOAD: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Trigger a browser file download
pub fn download(filename: &str, content: &str) {
    unsafe {
        monokrom_download_file(
            filename.as_ptr(),
            filename.len() as u32,
            content.as_ptr(),
            content.len() as u32,
        );
    }
}

/// Open the browser file picker for uploading
pub fn request_upload() {
    unsafe {
        monokrom_request_upload();
    }
}

/// Poll for a pending upload result (called each frame)
pub fn take_pending_upload() -> Option<(String, String)> {
    PENDING_UPLOAD.lock().unwrap().take()
}

/// Callback from JS when a file has been read
#[no_mangle]
pub extern "C" fn monokrom_upload_complete(
    js_name: sapp_jsutils::JsObject,
    js_content: sapp_jsutils::JsObject,
) {
    let mut name = String::new();
    js_name.to_string(&mut name);
    let mut content = String::new();
    js_content.to_string(&mut content);
    *PENDING_UPLOAD.lock().unwrap() = Some((name, content));
}
