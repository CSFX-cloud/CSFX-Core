interface PendingCredentials {
    username: string;
    password: string;
}

let pending: PendingCredentials | null = null;

export const pendingLogin = {
    set(username: string, password: string) {
        pending = { username, password };
    },
    get(): PendingCredentials | null {
        return pending;
    },
    clear() {
        pending = null;
    },
};
