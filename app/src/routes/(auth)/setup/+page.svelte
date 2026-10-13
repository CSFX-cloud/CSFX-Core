<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { Download, ShieldCheck } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { onMount } from "svelte";
    import { toast } from "svelte-sonner";
    import {
        completeSetup,
        getSetupStatus,
        SETUP_CA_URL,
        SetupError,
        type SetupStatus,
    } from "$lib/auth/api";
    import { auth } from "$lib/auth/store.svelte";

    type Step = "verify" | "account" | "console" | "done";

    const MIN_PASSWORD_LENGTH = 12;
    const MIN_PIN_LENGTH = 8;
    const STEPS: { id: Step; label: string }[] = [
        { id: "verify", label: "Verify" },
        { id: "account", label: "Administrator" },
        { id: "console", label: "Console PIN" },
    ];

    let step = $state<Step>("verify");
    let status = $state<SetupStatus | null>(null);
    let loadFailed = $state(false);
    let submitting = $state(false);
    let setupCode = $state("");
    let organizationName = $state("");
    let adminEmail = $state("");
    let password = $state("");
    let passwordRepeat = $state("");
    let consolePin = $state("");
    let consolePinRepeat = $state("");
    let recoveryKey = $state("");
    let keyStored = $state(false);

    const codeValid = $derived(setupCode.replace(/[^a-z0-9]/gi, "").length >= 8);
    const accountValid = $derived(
        organizationName.trim().length > 0 &&
            /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(adminEmail.trim()) &&
            password.length >= MIN_PASSWORD_LENGTH &&
            password === passwordRepeat,
    );
    const pinValid = $derived(
        consolePin.length >= MIN_PIN_LENGTH && consolePin === consolePinRepeat,
    );

    onMount(loadStatus);

    async function loadStatus() {
        loadFailed = false;
        try {
            status = await getSetupStatus();
            if (status.state === "ready") goto("/login");
        } catch {
            loadFailed = true;
            setTimeout(loadStatus, 5000);
        }
    }

    async function submit() {
        submitting = true;
        try {
            const result = await completeSetup({
                setupCode: setupCode.trim(),
                organizationName: organizationName.trim(),
                adminEmail: adminEmail.trim(),
                password,
                consolePin,
            });
            auth.setSession(result);
            recoveryKey = result.recovery_key;
            password = passwordRepeat = consolePin = consolePinRepeat = "";
            step = "done";
        } catch (err) {
            handleFailure(err);
        } finally {
            submitting = false;
        }
    }

    function handleFailure(err: unknown) {
        if (!(err instanceof SetupError)) {
            toast.error("Setup failed", { description: "The control plane did not respond" });
            return;
        }
        if (err.status === 410) {
            toast.info("Setup was already completed");
            goto("/login");
            return;
        }
        if (err.status === 401) step = "verify";
        toast.error("Setup failed", { description: err.message });
    }

    async function copyRecoveryKey() {
        await navigator.clipboard.writeText(recoveryKey);
        toast.success("Recovery key copied");
    }
</script>

<img
    src="/logo/logo-csfx.svg"
    alt="CSFX Logo"
    class="size-20 mb-6 rounded-md p-2 invert dark:invert-0"
/>

{#if loadFailed}
    <h1 class="text-2xl font-light mb-4">Control plane is starting</h1>
    <p class="text-sm text-muted-foreground flex items-center gap-2">
        <Spinner /> This can take a few minutes after the first boot. Retrying automatically.
    </p>
{:else if !status}
    <Spinner />
{:else if step === "done"}
    <h1 class="text-2xl font-light mb-2">Your cluster is ready</h1>
    <p class="text-sm text-muted-foreground mb-6">
        Store the console recovery key offline. It replaces the console PIN if you forget it and is
        shown only once.
    </p>
    <div class="rounded-md border bg-muted px-4 py-3 font-mono text-sm tracking-wider text-center mb-3">
        {recoveryKey}
    </div>
    <Button variant="outline" class="w-full mb-6" onclick={copyRecoveryKey}>Copy recovery key</Button>
    <ul class="text-sm text-muted-foreground list-disc pl-5 mb-6 space-y-1">
        <li>Enable two-factor authentication in your account settings.</li>
        <li>SSH access is off. Enable it temporarily under Settings when needed.</li>
        <li>Use the CLI with <span class="font-mono">csfx login</span>.</li>
    </ul>
    <div class="flex items-center gap-2 mb-4">
        <Checkbox id="key-stored" bind:checked={keyStored} />
        <Label for="key-stored" class="text-sm">I stored the recovery key</Label>
    </div>
    <Button class="w-full" disabled={!keyStored} onclick={() => goto("/")}>Open dashboard</Button>
{:else}
    <h1 class="text-2xl font-light mb-2">Set up CSFX</h1>
    <ol class="flex items-center gap-2 text-xs text-muted-foreground mb-8">
        {#each STEPS as entry, index (entry.id)}
            <li class={entry.id === step ? "font-bold text-foreground" : ""}>
                {index + 1}. {entry.label}
            </li>
        {/each}
    </ol>

    <form
        class="flex flex-col gap-4"
        onsubmit={(event) => {
            event.preventDefault();
            if (step === "verify" && codeValid) step = "account";
            else if (step === "account" && accountValid) step = "console";
            else if (step === "console" && pinValid && !submitting) submit();
        }}
    >
        {#if step === "verify"}
            <p class="text-sm text-muted-foreground">
                Your browser warns about the certificate because this node signs it with its own
                authority. Compare this fingerprint with the one on the node console.
            </p>
            <div class="rounded-md border bg-muted px-3 py-2 font-mono text-xs break-all">
                {status.ca_fingerprint ?? "unavailable"}
            </div>
            <Button variant="outline" size="sm" class="gap-2" href={SETUP_CA_URL} download>
                <Download class="size-4" /> Download CA certificate
            </Button>
            <div class="flex flex-col gap-1">
                <Label for="setup-code" class="text-xs font-bold">Setup code from the node console</Label>
                <Input id="setup-code" placeholder="ABCD-EFGH" autocomplete="off" bind:value={setupCode} />
            </div>
            <Button type="submit" class="w-full" disabled={!codeValid}>Continue</Button>
        {:else if step === "account"}
            <div class="flex flex-col gap-1">
                <Label for="organization" class="text-xs font-bold">Organization name</Label>
                <Input id="organization" autocomplete="organization" bind:value={organizationName} />
            </div>
            <div class="flex flex-col gap-1">
                <Label for="email" class="text-xs font-bold">Administrator email</Label>
                <Input id="email" type="email" autocomplete="username" bind:value={adminEmail} />
            </div>
            <div class="flex flex-col gap-1">
                <Label for="password" class="text-xs font-bold">
                    Password (at least {MIN_PASSWORD_LENGTH} characters)
                </Label>
                <Input id="password" type="password" autocomplete="new-password" bind:value={password} />
            </div>
            <div class="flex flex-col gap-1">
                <Label for="password-repeat" class="text-xs font-bold">Repeat password</Label>
                <Input id="password-repeat" type="password" autocomplete="new-password" bind:value={passwordRepeat} />
            </div>
            <div class="flex gap-2">
                <Button type="button" variant="outline" onclick={() => (step = "verify")}>Back</Button>
                <Button type="submit" class="flex-1" disabled={!accountValid}>Continue</Button>
            </div>
        {:else}
            <p class="text-sm text-muted-foreground flex gap-2">
                <ShieldCheck class="size-4 shrink-0 mt-0.5" />
                The PIN protects the recovery menu on the node console. It works without the
                control plane, so keep it separate from the administrator password.
            </p>
            <div class="flex flex-col gap-1">
                <Label for="pin" class="text-xs font-bold">Console PIN (at least {MIN_PIN_LENGTH} characters)</Label>
                <Input id="pin" type="password" autocomplete="off" bind:value={consolePin} />
            </div>
            <div class="flex flex-col gap-1">
                <Label for="pin-repeat" class="text-xs font-bold">Repeat PIN</Label>
                <Input id="pin-repeat" type="password" autocomplete="off" bind:value={consolePinRepeat} />
            </div>
            <div class="flex gap-2">
                <Button type="button" variant="outline" onclick={() => (step = "account")}>Back</Button>
                <Button type="submit" class="flex-1" disabled={!pinValid || submitting}>
                    {#if submitting}<Spinner />{:else}Initialize cluster{/if}
                </Button>
            </div>
        {/if}
    </form>
{/if}
