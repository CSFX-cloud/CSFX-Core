<script lang="ts">
    import { saveExportTarget, type ExportTargetInput } from "$lib/api/exports";
    import ModalDialog from "$lib/components/resource-group/modal-dialog.svelte";

    const MIN_PASSPHRASE_LENGTH = 12;

    let { onSaved }: { onSaved: () => Promise<void> } = $props();

    let dialog = $state<ModalDialog | null>(null);
    let name = $state("");
    let kind = $state<"local" | "s3">("local");
    let path = $state("");
    let endpoint = $state("");
    let region = $state("");
    let bucket = $state("");
    let prefix = $state("");
    let accessKeyId = $state("");
    let secretAccessKey = $state("");
    let encrypt = $state(false);
    let passphrase = $state("");
    let confirmation = $state("");
    let makeDefault = $state(false);
    let saving = $state(false);
    let error = $state<string | null>(null);

    const passphraseProblem = $derived.by(() => {
        if (!encrypt) return null;
        if (passphrase.length < MIN_PASSPHRASE_LENGTH) return `Passphrase needs at least ${MIN_PASSPHRASE_LENGTH} characters`;
        if (passphrase !== confirmation) return "Passphrases do not match";
        return null;
    });

    const complete = $derived(
        /^[a-z0-9_-]{1,63}$/.test(name) &&
            passphraseProblem === null &&
            (kind === "local" ? path.startsWith("/") : [endpoint, region, bucket, accessKeyId, secretAccessKey].every((value) => value.trim() !== "")),
    );

    export function open() {
        name = "";
        kind = "local";
        path = "";
        endpoint = "";
        region = "";
        bucket = "";
        prefix = "";
        accessKeyId = "";
        secretAccessKey = "";
        encrypt = false;
        passphrase = "";
        confirmation = "";
        makeDefault = false;
        error = null;
        dialog?.open();
    }

    function input(): ExportTargetInput {
        const shared = { passphrase: encrypt ? passphrase : null, default: makeDefault };
        if (kind === "local") return { kind, path: path.trim(), ...shared };
        return {
            kind,
            endpoint: endpoint.trim(),
            region: region.trim(),
            bucket: bucket.trim(),
            prefix: prefix.trim(),
            access_key_id: accessKeyId.trim(),
            secret_access_key: secretAccessKey,
            ...shared,
        };
    }

    async function save() {
        if (!complete) return;
        saving = true;
        error = null;
        try {
            await saveExportTarget(name, input());
            dialog?.close();
            await onSaved();
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to save export target";
        } finally {
            saving = false;
        }
    }
</script>

<ModalDialog bind:this={dialog} title="Add export target" subtitle="Where exports are written" size="lg" onclose={() => (error = null)}>
    <form
        class="md__fields"
        onsubmit={(event) => {
            event.preventDefault();
            save();
        }}
    >
        <div class="cx-field">
            <label class="cx-label" for="target-name">Name</label>
            <input class="cx-input" id="target-name" autocomplete="off" placeholder="offsite" bind:value={name} />
            <p class="cx-hint">Lowercase letters, digits, - and _</p>
        </div>
        <div class="cx-field">
            <span class="cx-label">Type</span>
            <div class="st-row">
                <label class="cx-check"><input type="radio" value="local" bind:group={kind} /> Path on the gateway host</label>
                <label class="cx-check"><input type="radio" value="s3" bind:group={kind} /> S3 compatible storage</label>
            </div>
        </div>
        {#if kind === "local"}
            <div class="cx-field">
                <label class="cx-label" for="target-path">Absolute path</label>
                <input class="cx-input" id="target-path" placeholder="/var/lib/csfx-cp/exports" bind:value={path} />
                <p class="cx-hint">NFS and SMB mounts on the gateway host work as well.</p>
            </div>
        {:else}
            <div class="cx-field">
                <label class="cx-label" for="target-endpoint">Endpoint</label>
                <input class="cx-input" id="target-endpoint" type="url" placeholder="https://s3.example.com" bind:value={endpoint} />
            </div>
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                <div class="cx-field">
                    <label class="cx-label" for="target-bucket">Bucket</label>
                    <input class="cx-input" id="target-bucket" bind:value={bucket} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="target-region">Region</label>
                    <input class="cx-input" id="target-region" placeholder="eu-central-1" bind:value={region} />
                </div>
            </div>
            <div class="cx-field">
                <label class="cx-label" for="target-prefix">Prefix</label>
                <input class="cx-input" id="target-prefix" placeholder="csfx/prod" bind:value={prefix} />
            </div>
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                <div class="cx-field">
                    <label class="cx-label" for="target-key">Access key id</label>
                    <input class="cx-input" id="target-key" autocomplete="off" bind:value={accessKeyId} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="target-secret">Secret access key</label>
                    <input class="cx-input" id="target-secret" type="password" autocomplete="new-password" bind:value={secretAccessKey} />
                </div>
            </div>
        {/if}
        <label class="cx-check"><input type="checkbox" bind:checked={encrypt} /> Encrypt exports with a passphrase</label>
        {#if encrypt}
            <div class="st-note">
                Exports cannot be read without this passphrase and there is no recovery. Store it somewhere safe.
            </div>
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px">
                <div class="cx-field">
                    <label class="cx-label" for="target-passphrase">Passphrase</label>
                    <input class="cx-input" id="target-passphrase" type="password" autocomplete="new-password" bind:value={passphrase} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="target-confirm">Repeat passphrase</label>
                    <input class="cx-input" id="target-confirm" type="password" autocomplete="new-password" bind:value={confirmation} />
                </div>
            </div>
            {#if passphrase !== "" && passphraseProblem}<p class="cx-hint cx-hint--error">{passphraseProblem}</p>{/if}
        {/if}
        <label class="cx-check"><input type="checkbox" bind:checked={makeDefault} /> Use as default target</label>
        {#if error}<p class="cx-hint cx-hint--error">{error}</p>{/if}
        <div class="md__foot">
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={() => dialog?.close()}>Cancel</button>
            <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={saving || !complete} aria-busy={saving}>
                {saving ? "Saving" : "Save target"}
            </button>
        </div>
    </form>
</ModalDialog>
