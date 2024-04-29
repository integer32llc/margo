class Copy extends HTMLElement {
  connectedCallback() {
    let button = this.querySelector('[data-target = "copy"]');
    let state0 = this.querySelector('[data-target = "state0"]');
    let state1 = this.querySelector('[data-target = "state1"]');

    if (!(button && state0 && state1)) {
      return;
    }

    const swapState = () => {
      state0.classList.toggle("invisible");
      state1.classList.toggle("invisible");
    };

    button.addEventListener("click", (evt) => {
      evt.preventDefault();

      let content = this.querySelector('[data-target = "content"]');
      let text = content?.textContent;
      if (!text) {
        return;
      }

      navigator.clipboard.writeText(text);

      swapState();
      window.setTimeout(swapState, 1000);
    });

    button.classList.remove("hidden");
  }
}

window.customElements.define("mg-copy", Copy);
