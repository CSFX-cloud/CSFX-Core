let open = $state(false);

export const commandPalette = {
    get open() {
        return open;
    },
    show() {
        open = true;
    },
    hide() {
        open = false;
    },
    toggle() {
        open = !open;
    },
    set(value: boolean) {
        open = value;
    },
};
