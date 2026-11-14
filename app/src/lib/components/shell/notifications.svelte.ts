import { listEvents } from "$lib/api/events";

let openCount = $state(0);

export const notifications = {
    get openCount() {
        return openCount;
    },
    set(value: number) {
        openCount = value;
    },
    async refresh() {
        try {
            openCount = (await listEvents({ status: "open" })).length;
        } catch {
            return;
        }
    },
};
