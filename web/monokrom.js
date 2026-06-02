// Monokrom — prevent browser from intercepting app hotkeys.
// gl.js is patched to map MetaLeft/MetaRight to LeftSuper/RightSuper,
// so Cmd key events flow to miniquad naturally. We just need to block
// the browser's default actions (Save Page, Open File, New Window, etc).
var _mkr_macos = /Mac|iPhone|iPad|iPod/.test(navigator.platform);

// sapp keycode for Escape (matches gl.js into_sapp_keycode).
var _MKR_SAPP_ESCAPE = 256;

function _mkr_sapp_modifiers(e) {
    var m = 0;
    if (e.shiftKey) m |= 1;
    if (e.ctrlKey)  m |= 2;
    if (e.altKey)   m |= 4;
    if (e.metaKey)  m |= 8;
    return m;
}

// Forward Escape directly to miniquad from the document level. Firefox blurs
// the focused element on Esc (even with preventDefault), which would leave
// canvas.onkeydown deaf to subsequent presses. stopPropagation avoids double
// dispatch when the canvas is still focused.
document.addEventListener("keydown", function (e) {
    if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        if (typeof wasm_exports !== "undefined" && wasm_exports) {
            wasm_exports.key_down(_MKR_SAPP_ESCAPE, _mkr_sapp_modifiers(e), e.repeat);
        }
        return;
    }
    // Alt+key: block OS special char insertion for app shortcuts
    if (e.altKey) {
        var k = e.key.toLowerCase();
        if ("sonzyxcvafrdi".indexOf(k) !== -1) {
            e.preventDefault();
        }
        // Alt+arrows: block browser back/forward; Alt+Del/Bksp: block browser actions
        if (e.key === "ArrowUp" || e.key === "ArrowDown" ||
            e.key === "ArrowLeft" || e.key === "ArrowRight" ||
            e.key === "Delete" || e.key === "Backspace") {
            e.preventDefault();
        }
    }
    // Cmd/Ctrl+arrows: block browser scroll-to-top/bottom
    var mod = _mkr_macos ? e.metaKey : e.ctrlKey;
    if (mod) {
        if (e.key === "ArrowUp" || e.key === "ArrowDown" ||
            e.key === "ArrowLeft" || e.key === "ArrowRight") {
            e.preventDefault();
        }
    }
}, true);

document.addEventListener("keyup", function (e) {
    if (e.key === "Escape") {
        e.stopPropagation();
        if (typeof wasm_exports !== "undefined" && wasm_exports) {
            wasm_exports.key_up(_MKR_SAPP_ESCAPE, _mkr_sapp_modifiers(e));
        }
    }
}, true);

// When the browser steals focus (e.g. Cmd+S "Save Page" dialog), modifier
// key-up events are lost, leaving miniquad's is_key_down stuck. Dispatch
// synthetic keyups on the canvas so gl.js clears the key state.
window.addEventListener("blur", function () {
    var canvas = document.querySelector("canvas");
    if (!canvas) return;
    [["MetaLeft", "Meta"], ["MetaRight", "Meta"],
     ["ControlLeft", "Control"], ["ControlRight", "Control"]].forEach(function (pair) {
        canvas.dispatchEvent(new KeyboardEvent("keyup", {
            code: pair[0], key: pair[1], bubbles: false
        }));
    });
});

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

        importObject.env.monokrom_has_embedded_source = function () {
            return (typeof MONOKROM_GAME_SOURCE !== "undefined") ? 1 : 0;
        };

        importObject.env.monokrom_get_embedded_source = function () {
            return js_object(typeof MONOKROM_GAME_SOURCE !== "undefined" ? MONOKROM_GAME_SOURCE : "");
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
