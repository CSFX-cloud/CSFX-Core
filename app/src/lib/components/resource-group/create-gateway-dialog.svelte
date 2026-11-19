<script lang="ts">
    import CreateGatewayForm from "./create-gateway-form.svelte";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId, onCreated }: { rgId: string; onCreated: (gatewayId: string) => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);

    export function open() {
        dialog?.open();
    }

    async function created(gatewayId: string) {
        dialog?.close();
        await onCreated(gatewayId);
    }
</script>

<ModalDialog bind:this={dialog} title="Create API gateway" size="md">
    <CreateGatewayForm {rgId} onCreated={created} oncancel={() => dialog?.close()} />
</ModalDialog>
