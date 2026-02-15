// Monokrom — prevent browser from intercepting app hotkeys.
// gl.js is patched to map MetaLeft/MetaRight to LeftSuper/RightSuper,
// so Cmd key events flow to miniquad naturally. We just need to block
// the browser's default actions (Save Page, Open File, New Window, etc).
var _mkr_macos = /Mac|iPhone|iPad|iPod/.test(navigator.platform);

document.addEventListener("keydown", function (e) {
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
