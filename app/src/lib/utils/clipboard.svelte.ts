const RESET_DELAY_MS = 1500;

export function createClipboard() {
    let copiedKey = $state<string | null>(null);

    async function copy(value: string, key: string) {
        try {
            await navigator.clipboard.writeText(value);
            copiedKey = key;
            setTimeout(() => {
                if (copiedKey === key) copiedKey = null;
            }, RESET_DELAY_MS);
        } catch {
            copiedKey = null;
        }
    }

    return {
        get copiedKey() {
            return copiedKey;
        },
        copy,
    };
}
