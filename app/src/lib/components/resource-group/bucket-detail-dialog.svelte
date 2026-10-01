<script lang="ts">
    import { auth } from "$lib/auth/store.svelte";
    import {
        listBucketKeys,
        createBucketKey,
        type Bucket,
        type BucketAccessKey,
    } from "$lib/api/resource-groups";
    import { Button } from "$lib/components/ui/button/index.js";
    import { createClipboard } from "$lib/utils/clipboard.svelte";
    import ModalDialog from "./modal-dialog.svelte";

    let { rgId }: { rgId: string } = $props();

    const clipboard = createClipboard();

    let dialog = $state<ModalDialog | null>(null);
    let bucket = $state<Bucket | null>(null);
    let keys = $state<BucketAccessKey[]>([]);
    let error = $state<string | null>(null);
    let newKeyName = $state("");
    let creating = $state(false);
    let createdSecret = $state<string | null>(null);

    const endpoint = $derived(
        !bucket
            ? ""
            : bucket.exposure === "external"
              ? `${window.location.origin}/api/s3/${bucket.global_alias}`
              : `http://s3.svc.${rgId}.internal:3900`,
    );

    export async function open(target: Bucket) {
        if (!auth.token) return;
        bucket = target;
        error = null;
        createdSecret = null;
        newKeyName = "";
        dialog?.open();
        try {
            keys = await listBucketKeys(auth.token, target.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load access keys";
        }
    }

    async function createKey() {
        if (!auth.token || !bucket || !newKeyName) return;
        creating = true;
        error = null;
        try {
            const created = await createBucketKey(auth.token, bucket.id, { name: newKeyName });
            createdSecret = created.secret_access_key;
            newKeyName = "";
            keys = await listBucketKeys(auth.token, bucket.id);
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to create access key";
        } finally {
            creating = false;
        }
    }
</script>

<ModalDialog
    bind:this={dialog}
    title={bucket?.name ?? ""}
    subtitle={endpoint}
    width="max-w-lg"
    onclose={() => {
        bucket = null;
        createdSecret = null;
    }}
>
    <div class="flex flex-col gap-2">
        <p class="text-xs font-medium text-muted-foreground">Access Keys</p>
        {#if keys.length === 0}
            <p class="text-xs text-muted-foreground">no access keys yet</p>
        {:else}
            <div class="border rounded-lg divide-y">
                {#each keys as key (key.id)}
                    <div class="flex items-center justify-between px-3 py-2 text-xs">
                        <div>
                            <p class="font-medium">{key.name}</p>
                            <p class="text-muted-foreground font-mono">{key.garage_key_id}</p>
                        </div>
                        <span class="text-muted-foreground">{key.permissions}</span>
                    </div>
                {/each}
            </div>
        {/if}
    </div>

    {#if createdSecret}
        <div class="border rounded-lg p-3 bg-muted/30 flex flex-col gap-1.5">
            <p class="text-xs font-medium">Secret access key created</p>
            <p class="text-xs font-mono break-all">{createdSecret}</p>
            <p class="text-xs text-destructive">this will not be shown again, copy it now</p>
            <Button
                size="sm"
                variant="outline"
                class="self-start"
                onclick={() => clipboard.copy(createdSecret ?? "", "bucket-secret")}
            >
                {clipboard.copiedKey === "bucket-secret" ? "Copied" : "Copy"}
            </Button>
        </div>
    {/if}

    {#if error}
        <p class="text-xs text-destructive">{error}</p>
    {/if}

    <div class="flex gap-2">
        <input class="border rounded px-3 py-1.5 text-sm bg-background flex-1" placeholder="key name" bind:value={newKeyName} />
        <Button size="sm" onclick={createKey} disabled={creating || !newKeyName}>
            {creating ? "Creating..." : "Create Key"}
        </Button>
    </div>
</ModalDialog>
