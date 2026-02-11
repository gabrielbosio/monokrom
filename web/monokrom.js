// Monokrom — prevent browser from intercepting app hotkeys.
// gl.js is patched to map MetaLeft/MetaRight to LeftSuper/RightSuper,
// so Cmd key events flow to miniquad naturally. We just need to block
// the browser's default actions (Save Page, Open File, New Window, etc).
var _mkr_macos = /Mac|iPhone|iPad|iPod/.test(navigator.platform);

document.addEventListener("keydown", function (e) {
    var mod = _mkr_macos ? e.metaKey : e.ctrlKey;
    if (mod) {
        var k = e.key.toLowerCase();
        // Block browser defaults for Cmd/Ctrl app shortcuts
        if ("sozxyvcafreild".indexOf(k) !== -1) {
            e.preventDefault();
        }
        if (e.key === "ArrowUp" || e.key === "ArrowDown" ||
            e.key === "ArrowLeft" || e.key === "ArrowRight" ||
            e.key === "Delete" || e.key === "Backspace") {
            e.preventDefault();
        }
    }
    // Alt+N for new file — prevent browser/OS special character insertion
    if (e.altKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
    }
}, true);

// Monokrom miniquad plugin — platform detection + file export/import
miniquad_add_plugin({
    register_plugin: function (importObject) {
        importObject.env.monokrom_is_macos = function () {
            return _mkr_macos ? 1 : 0;
        };

        importObject.env.monokrom_download_file = function (
            name_ptr,
            name_len,
            data_ptr,
            data_len
        ) {
            var buf = new Uint8Array(wasm_memory.buffer, name_ptr, name_len);
            var name = new TextDecoder().decode(buf);
            var data_buf = new Uint8Array(wasm_memory.buffer, data_ptr, data_len);
            var content = new TextDecoder().decode(data_buf);

            var blob = new Blob([content], { type: "text/plain" });
            var a = document.createElement("a");
            a.href = URL.createObjectURL(blob);
            a.download = name;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(a.href);
        };

        importObject.env.monokrom_request_upload = function () {
            var input = document.createElement("input");
            input.type = "file";
            input.accept = ".txt,.mkr,text/plain";
            input.onchange = function (e) {
                var file = e.target.files[0];
                if (!file) return;
                var reader = new FileReader();
                reader.onload = function (ev) {
                    var name = js_object(file.name);
                    var content = js_object(ev.target.result);
                    wasm_exports.monokrom_upload_complete(name, content);
                };
                reader.readAsText(file);
            };
            input.click();
        };
    },
});
