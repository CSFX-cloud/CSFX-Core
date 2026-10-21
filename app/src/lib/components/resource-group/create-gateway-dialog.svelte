<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import { createGateway } from "$lib/api/ingress";
    import { Button } from "$lib/components/ui/button/index.js";
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

<ModalDialog bind:this={dialog} title="Create API Gateway" onclose={() => (error = null)}>
    <p class="text-xs text-muted-foreground">
        The gateway runs as a service in this resource group. Connect your services to it afterwards and publish them under a local or public domain.
    </p>
    <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="gw-name">Name</label>
            <input id="gw-name" class="border rounded px-3 py-1.5 text-sm bg-background" placeholder="edge" bind:value={name} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="gw-http">HTTP port</label>
            <input id="gw-http" type="number" min="1" max="65535" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={httpPort} />
        </div>
        <div class="flex flex-col gap-1">
            <label class="text-xs text-muted-foreground" for="gw-https">HTTPS port</label>
            <input id="gw-https" type="number" min="1" max="65535" class="border rounded px-3 py-1.5 text-sm bg-background" bind:value={httpsPort} />
        </div>
    </div>
    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}
    <div class="flex gap-2 justify-end">
        <Button size="sm" variant="outline" onclick={() => dialog?.close()}>Cancel</Button>
        <Button size="sm" onclick={submit} disabled={creating || !valid}>
            {creating ? "Creating..." : "Create"}
        </Button>
    </div>
</ModalDialog>
