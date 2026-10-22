<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createGateway } from "$lib/api/ingress";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId, onCreated }: { rgId: string; onCreated: (gatewayId: string) => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let httpPort = $state(80);
    let httpsPort = $state(443);
    let creating = $state(false);
    let error = $state<string | null>(null);

    const valid = $derived(name.trim() !== "" && httpPort > 0 && httpsPort > 0 && httpPort !== httpsPort);

    export function open() {
        dialog?.open();
    }

    async function submit() {
        if (!auth.token || !valid) return;
        creating = true;
        error = null;
        try {
            const gateway = await createGateway(auth.token, rgId, { name: name.trim(), http_port: httpPort, https_port: httpsPort });
            dialog?.close();
            name = "";
            await onCreated(gateway.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create gateway";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Create API gateway" size="md" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            submit();
        }}
    >
        <p class="db-sub" style="margin: 0; font-size: 13px">
            The gateway runs as a service in this resource group. Connect your services to it afterwards and publish them under a local or public domain.
        </p>
        <div class="cx-field">
            <label class="cx-label" for="gw-name">Name</label>
            <input class="cx-input" id="gw-name" placeholder="edge" autocomplete="off" bind:value={name} />
        </div>
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
            <div class="cx-field">
                <label class="cx-label" for="gw-http">HTTP port</label>
                <input class="cx-input" id="gw-http" type="number" min="1" max="65535" bind:value={httpPort} />
            </div>
            <div class="cx-field">
                <label class="cx-label" for="gw-https">HTTPS port</label>
                <input class="cx-input" id="gw-https" type="number" min="1" max="65535" bind:value={httpsPort} />
            </div>
        </div>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={creating || !valid} aria-busy={creating}>
                {creating ? "Creating" : "Create"}
            </button>
        </div>
    </form>
</ModalDialog>
