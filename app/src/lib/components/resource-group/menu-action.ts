import type { Component } from "svelte";

export interface MenuAction {
    label: string;
    icon: Component;
    onselect: () => void;
    danger?: boolean;
    disabled?: boolean;
}
