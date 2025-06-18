(function () {
    const canvas = document.querySelector("canvas");
    /**
     *
     * @type {HTMLInputElement}
     */
    const ipt = document.createElement("input");
    ipt.id = "winit-input";
    ipt.style.cssText = "background-color: transparent; border: none; outline: none; width: 1px; height: 1px; caret-color: transparent; position: absolute;left:0;top:0;"
    ipt.addEventListener("keydown", (event) => {
        event.preventDefault();
    });
    ipt.addEventListener("keyup", (event) => {
        event.preventDefault();
    });
    ipt.addEventListener("compositionend", (event) => {
        const text = event.data;
        const callback = cwrap('winit_emscripten_send_input', null, ['string']);
        callback(text);
        ipt.value = "";
    });
    document.body.appendChild(ipt);
    const winit = {
        canvas,
        inputting: false,
        input: ipt,
        allowIme(value) {
            if (value) {
                this.inputting = true;
                this.input.focus();
            } else {
                this.inputting = false;
                this.canvas.focus();
            }
        },
        setImeCursor(x, y, width, height) {
            const ipt = this.input;
            ipt.style.left = x + "px";
            ipt.style.top = y + "px";
        }
    };
    canvas.addEventListener("click", () => {
        if (winit.inputting) {
            winit.input.focus();
        }
    });
    Module.winit = winit;
})();
