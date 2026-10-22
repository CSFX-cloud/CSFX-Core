let detail = $state<string | null>(null);

export const breadcrumb = {
    get detail() {
        return detail;
    },
    set(value: string | null) {
        detail = value;
    },
};
